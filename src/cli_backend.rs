use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use unicode_width::UnicodeWidthChar;

use tokio::process::Command;

use crate::backend::WingetBackend;
use crate::models::{Package, PackageDetail, Source};

pub struct CliBackend;

/// Returns `true` for winget footer lines like `"2 upgrades available."` or
/// `"3 Pakete verfügen über Pins…"`.
///
/// These lines start with one or more ASCII digits immediately followed by a
/// space.  A plain digit-prefixed package name such as `"7-Zip 25.01 (x64)"`
/// is **not** a footer because the digit sequence is followed by `'-'`, not `' '`.
fn is_winget_footer_line(line: &str) -> bool {
    let bytes = line.trim_start().as_bytes();
    let d = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    d > 0 && d < bytes.len() && bytes[d] == b' '
}

/// Strip ASCII control characters (0x00–0x1F, 0x7F) except tab and newline.
/// Prevents ANSI escape injection from malicious package metadata.
fn sanitize_text(s: &str) -> String {
    s.chars()
        .filter(|&c| c == '\t' || c == '\n' || (c >= ' ' && c != '\x7F'))
        .collect()
}

/// Pre-computed column indices for a package table.
#[derive(Copy, Clone)]
struct PackageCols {
    name: Option<usize>,
    id: Option<usize>,
    version: Option<usize>,
    source: Option<usize>,
    available: Option<usize>,
}

/// Pre-computed column indices for a source table.
#[derive(Copy, Clone)]
struct SourceCols {
    name: Option<usize>,
    arg: Option<usize>,
    source_type: Option<usize>,
}

impl CliBackend {
    pub fn new() -> Self {
        Self
    }

    async fn run_winget(&self, args: &[&str]) -> Result<String> {
        self.run_winget_inner(args, false).await
    }

    /// Run winget in strict mode: any non-zero exit is an error.
    /// Use for mutating operations (install, uninstall, upgrade).
    async fn run_winget_strict(&self, args: &[&str]) -> Result<String> {
        self.run_winget_inner(args, true).await
    }

    async fn run_winget_inner(&self, args: &[&str], strict: bool) -> Result<String> {
        let output = Command::new("winget")
            .args(args)
            .output()
            .await
            .context("Failed to run winget. Is it installed?")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && (strict || stdout.trim().is_empty()) {
            // In strict mode, always fail. In lenient mode, fail only if
            // there's no stdout (winget returns non-zero for "no results"
            // but still prints a table).
            let detail = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            bail!("winget failed: {}", detail);
        }

        Ok(Self::clean_output(&stdout))
    }

    /// Normalize winget stdout: resolve `\r\n` line endings and progress-spinner
    /// overwrites (`\r` mid-line, keeping only the segment after the last one).
    ///
    /// Uses a single pre-allocated `String` instead of the three-allocation
    /// pipeline (`replace` → `collect::<Vec<_>>` → `join`) that the naive
    /// implementation requires. `str::lines()` already strips `\r\n` for us,
    /// so we only need to handle embedded `\r` characters within a line.
    fn clean_output(stdout: &str) -> String {
        let mut cleaned = String::with_capacity(stdout.len());
        for line in stdout.lines() {
            if !cleaned.is_empty() {
                cleaned.push('\n');
            }
            let content = if let Some(pos) = line.rfind('\r') {
                &line[pos + 1..]
            } else {
                line
            };
            cleaned.push_str(content);
        }
        cleaned
    }

    fn parse_packages_from_table(&self, output: &str) -> Vec<Package> {
        // winget table output has a header line followed by a separator (all dashes)
        // then data rows. Column positions are determined by the header.
        // winget also emits short progress lines like "-", "\", "|" before the table.
        let lines: Vec<&str> = output.lines().collect();

        // Find the real separator line: must be mostly dashes and long enough to be a table separator
        let sep_idx = lines.iter().position(|l| {
            let trimmed = l.trim();
            trimmed.len() > 10
                && trimmed.chars().all(|c| c == '-' || c == ' ')
                && trimmed.contains('-')
        });
        let sep_idx = match sep_idx {
            Some(i) if i > 0 => i,
            _ => return Vec::new(),
        };

        let header = lines[sep_idx - 1];
        let col_positions = Self::detect_columns(header);
        let col_map = Self::package_column_map(&col_positions);

        lines[sep_idx + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            // Skip footer lines like "2 upgrades available." (digit(s) + space).
            // Uses filter (not take_while) so a false positive only skips one line
            // instead of silently dropping all remaining packages.
            .filter(|l| !is_winget_footer_line(l))
            .filter_map(|line| self.parse_table_row(line, &col_positions, col_map))
            .collect()
    }

    fn detect_columns(header: &str) -> Vec<(&str, usize)> {
        let mut cols = Vec::new();
        let mut display_pos = 0usize;
        let mut iter = header.char_indices().peekable();

        loop {
            // Skip whitespace
            while let Some(&(_, ' ')) = iter.peek() {
                iter.next();
                display_pos += 1;
            }
            let Some(&(start_byte, _)) = iter.peek() else {
                break;
            };
            let start_display = display_pos;
            // Read until whitespace
            let mut end_byte = start_byte;
            while let Some(&(byte_off, ch)) = iter.peek() {
                if ch == ' ' {
                    break;
                }
                end_byte = byte_off + ch.len_utf8();
                display_pos += ch.width().unwrap_or(0);
                iter.next();
            }
            let name = &header[start_byte..end_byte];
            cols.push((name, start_display));
        }
        cols
    }

    /// Find a column index matching any of the given names (case-insensitive).
    fn find_column_ci(cols: &[(&str, usize)], names: &[&str]) -> Option<usize> {
        cols.iter().position(|(col_name, _)| {
            let lower = col_name.to_lowercase();
            names.iter().any(|n| lower == *n)
        })
    }

    /// Extract the field at column `idx` from `line`, using display-width column boundaries.
    /// Returns an empty string if the index is out of range.
    fn extract_field(line: &str, cols: &[(&str, usize)], idx: usize) -> String {
        if idx >= cols.len() {
            return String::new();
        }
        let col_start = cols[idx].1;
        let col_end = if idx + 1 < cols.len() {
            cols[idx + 1].1
        } else {
            usize::MAX
        };

        let mut result = String::new();
        let mut width = 0usize;
        for ch in line.chars() {
            let cw = ch.width().unwrap_or(0);
            if width + cw > col_start && width < col_end {
                result.push(ch);
            }
            width += cw;
            if width >= col_end {
                break;
            }
        }
        result.trim().to_string()
    }

    /// Normalize a `winget show` key to a canonical English name (case-insensitive,
    /// with known translations for common locales).
    fn normalize_show_key(key: &str) -> &'static str {
        match key.to_lowercase().as_str() {
            "version" | "packageversion" => "version",
            "publisher" | "herausgeber" | "éditeur" | "editore" | "editor" => "publisher",
            "description" | "beschreibung" | "descripción" | "descrição" | "descrizione" => {
                "description"
            }
            "homepage" | "startseite" => "homepage",
            "publisher url" | "herausgeber-url" => "publisher_url",
            "license" | "lizenz" | "licence" | "licencia" | "licença" | "licenza" => "license",
            "source" | "quelle" | "origen" | "fonte" | "origine" => "source",
            _ => "",
        }
    }

    /// Pre-compute package column indices once for a table, to avoid repeated
    /// `to_lowercase()` allocations for every row.
    fn package_column_map(cols: &[(&str, usize)]) -> PackageCols {
        let mut map = PackageCols {
            name: Self::find_column_ci(cols, &["name", "nom", "nombre", "nome"]),
            id: Self::find_column_ci(cols, &["id", "id."]),
            version: Self::find_column_ci(cols, &["version", "versión", "versão", "versione"]),
            source: Self::find_column_ci(cols, &["source", "quelle", "origen", "fonte", "origine"]),
            available: Self::find_column_ci(
                cols,
                &[
                    "available",
                    "verfügbar",
                    "disponible",
                    "disponível",
                    "disponibile",
                ],
            ),
        };
        // Positional fallback for unrecognized locales (e.g. CJK)
        if map.id.is_none() && cols.len() >= 4 {
            map.name = map.name.or(Some(0));
            map.id = Some(1);
            map.version = map.version.or(Some(2));
            if cols.len() >= 5 {
                map.available = map.available.or(Some(3));
                map.source = map.source.or(Some(4));
            } else {
                map.source = map.source.or(Some(3));
            }
        }
        map
    }

    fn parse_table_row(
        &self,
        line: &str,
        cols: &[(&str, usize)],
        pcols: PackageCols,
    ) -> Option<Package> {
        // Extract fields using display-width columns (not byte offsets).
        // The header column positions are in display-width units (ASCII, so bytes == display width).
        // Data rows may contain multi-byte UTF-8 chars (e.g. '…') that are 1 display column
        // but 3 bytes, so we walk chars counting display width to find correct slice points.
        let field = |idx| Self::extract_field(line, cols, idx);

        let id = pcols.id.map(&field).unwrap_or_default();
        if id.is_empty() {
            return None;
        }
        // Valid package IDs contain '.' (e.g. "Google.Chrome") or '\' (e.g.
        // "ARP\Machine\X64\Git_is1"). This filters out text from footer lines
        // that happen to land in the ID column (e.g. long localized messages).
        if !id.contains('.') && !id.contains('\\') {
            return None;
        }

        Some(Package {
            name: sanitize_text(&pcols.name.map(&field).unwrap_or_default()),
            id: sanitize_text(&id),
            version: sanitize_text(&pcols.version.map(&field).unwrap_or_default()),
            source: sanitize_text(&pcols.source.map(&field).unwrap_or_default()),
            available_version: sanitize_text(&pcols.available.map(&field).unwrap_or_default()),
        })
    }

    fn parse_show_output(&self, output: &str) -> PackageDetail {
        let mut detail = PackageDetail::default();

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Parse "Found Name [Id]" header line (locale-independent).
            // Matches any "Prefix Name [Id]" pattern, e.g. "Gefunden Chrome [Google.Chrome]"
            if let (Some(bracket_start), Some(bracket_end)) =
                (trimmed.rfind('['), trimmed.rfind(']'))
            {
                if bracket_end > bracket_start && !trimmed.contains(':') {
                    let before_bracket = trimmed[..bracket_start].trim();
                    // Skip the prefix word ("Found", "Gefunden", etc.)
                    detail.name = sanitize_text(
                        &before_bracket
                            .split_once(' ')
                            .map(|(_, name)| name.trim().to_string())
                            .unwrap_or_default(),
                    );
                    detail.id = sanitize_text(&trimmed[bracket_start + 1..bracket_end]);
                    i += 1;
                    continue;
                }
            }

            // Parse "Key: Value" lines (only top-level, not indented)
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim();
                    let value = value.trim().to_string();
                    match Self::normalize_show_key(key) {
                        "version" => detail.version = sanitize_text(&value),
                        "publisher" => detail.publisher = sanitize_text(&value),
                        "description" => {
                            // Description value may be on this line or on indented continuation lines
                            let mut desc = value;
                            while i + 1 < lines.len() && lines[i + 1].starts_with("  ") {
                                i += 1;
                                if !desc.is_empty() {
                                    desc.push(' ');
                                }
                                desc.push_str(lines[i].trim());
                            }
                            detail.description = sanitize_text(&desc);
                        }
                        "homepage" => detail.homepage = sanitize_text(&value),
                        "publisher_url" => {
                            if detail.homepage.is_empty() {
                                detail.homepage = sanitize_text(&value);
                            }
                        }
                        "license" => detail.license = sanitize_text(&value),
                        "source" => detail.source = sanitize_text(&value),
                        _ => {}
                    }
                }
            }
            i += 1;
        }

        detail
    }

    /// Pre-compute source column indices once for a table.
    fn source_column_map(cols: &[(&str, usize)]) -> SourceCols {
        let mut map = SourceCols {
            name: Self::find_column_ci(cols, &["name", "nom", "nombre", "nome"]),
            arg: Self::find_column_ci(cols, &["argument"]),
            source_type: Self::find_column_ci(cols, &["type", "typ", "tipo"]),
        };
        // Positional fallback for unrecognized locales
        if map.name.is_none() && cols.len() >= 3 {
            map.name = Some(0);
            map.arg = Some(1);
            map.source_type = Some(2);
        }
        map
    }

    #[allow(dead_code)]
    fn parse_sources_from_table(&self, output: &str) -> Vec<Source> {
        let lines: Vec<&str> = output.lines().collect();
        let sep_idx = lines.iter().position(|l| {
            let trimmed = l.trim();
            trimmed.len() > 10
                && trimmed.chars().all(|c| c == '-' || c == ' ')
                && trimmed.contains('-')
        });
        let sep_idx = match sep_idx {
            Some(i) if i > 0 => i,
            _ => return Vec::new(),
        };

        let header = lines[sep_idx - 1];
        let col_positions = Self::detect_columns(header);
        let col_map = Self::source_column_map(&col_positions);

        lines[sep_idx + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let field = |idx| Self::extract_field(line, &col_positions, idx);

                let name = col_map.name.map(&field).unwrap_or_default();
                if name.is_empty() {
                    return None;
                }

                Some(Source {
                    name,
                    url: col_map.arg.map(&field).unwrap_or_default(),
                    source_type: col_map.source_type.map(&field).unwrap_or_default(),
                })
            })
            .collect()
    }
}

#[async_trait]
impl WingetBackend for CliBackend {
    async fn search(&self, query: &str, source: Option<&str>) -> Result<Vec<Package>> {
        let mut args = vec!["search", query, "--accept-source-agreements"];
        if let Some(src) = source {
            args.push("--source");
            args.push(src);
        }
        let output = self.run_winget(&args).await?;
        Ok(self.parse_packages_from_table(&output))
    }

    async fn list_installed(&self, source: Option<&str>) -> Result<Vec<Package>> {
        let mut args = vec!["list", "--accept-source-agreements"];
        if let Some(src) = source {
            args.push("--source");
            args.push(src);
        }
        let output = self.run_winget(&args).await?;
        Ok(self.parse_packages_from_table(&output))
    }

    async fn list_upgrades(&self, source: Option<&str>) -> Result<Vec<Package>> {
        let mut args = vec!["upgrade", "--accept-source-agreements"];
        if let Some(src) = source {
            args.push("--source");
            args.push(src);
        }
        let output = self.run_winget(&args).await?;
        Ok(self.parse_packages_from_table(&output))
    }

    async fn show(&self, id: &str) -> Result<PackageDetail> {
        let output = self
            .run_winget(&["show", "--id", id, "--exact", "--accept-source-agreements"])
            .await?;
        Ok(self.parse_show_output(&output))
    }

    async fn install(&self, id: &str, version: Option<&str>) -> Result<String> {
        let mut args = vec![
            "install",
            "--id",
            id,
            "--accept-source-agreements",
            "--accept-package-agreements",
        ];
        if let Some(v) = version {
            args.push("--version");
            args.push(v);
        }
        self.run_winget_strict(&args).await
    }

    async fn uninstall(&self, id: &str) -> Result<String> {
        self.run_winget_strict(&["uninstall", "--id", id, "--accept-source-agreements"])
            .await
    }

    async fn upgrade(&self, id: &str) -> Result<String> {
        self.run_winget_strict(&[
            "upgrade",
            "--id",
            id,
            "--accept-source-agreements",
            "--accept-package-agreements",
        ])
        .await
    }

    async fn list_sources(&self) -> Result<Vec<Source>> {
        let output = self.run_winget(&["source", "list"]).await?;
        Ok(self.parse_sources_from_table(&output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_english_upgrade_table() {
        let backend = CliBackend::new();
        let output = "\
Name                           Id                          Version     Available   Source
-----------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
Microsoft Visual Studio Code   Microsoft.VisualStudioCode  1.95.3      1.96.0      winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[0].version, "131.0.6778");
        assert_eq!(packages[0].available_version, "132.0.6834");
        assert_eq!(packages[0].source, "winget");
        assert_eq!(packages[1].id, "Microsoft.VisualStudioCode");
    }

    #[test]
    fn parse_german_upgrade_table() {
        let backend = CliBackend::new();
        let output = "\
Name                           ID                          Version     Verfügbar   Quelle
-----------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
Microsoft Visual Studio Code   Microsoft.VisualStudioCode  1.95.3      1.96.0      winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 2, "should parse German table headers");
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[0].available_version, "132.0.6834");
        assert_eq!(packages[0].source, "winget");
        assert_eq!(packages[1].id, "Microsoft.VisualStudioCode");
    }

    #[test]
    fn parse_unknown_locale_positional_fallback() {
        let backend = CliBackend::new();
        // Unrecognized column headers trigger positional fallback
        let output = "\
Foo                            Bar                         Baz         Qux         Quux
-----------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 1, "should parse via positional fallback");
        assert_eq!(packages[0].name, "Google Chrome");
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[0].version, "131.0.6778");
        assert_eq!(packages[0].available_version, "132.0.6834");
        assert_eq!(packages[0].source, "winget");
    }

    #[test]
    fn parse_english_show_output() {
        let backend = CliBackend::new();
        let output = "\
Found Google Chrome [Google.Chrome]
Version: 132.0.6834
Publisher: Google LLC
Description: A fast, secure, and free web browser
Homepage: https://www.google.com/chrome
License: Proprietary
Source: winget
";
        let detail = backend.parse_show_output(output);
        assert_eq!(detail.id, "Google.Chrome");
        assert_eq!(detail.name, "Google Chrome");
        assert_eq!(detail.version, "132.0.6834");
        assert_eq!(detail.publisher, "Google LLC");
        assert_eq!(detail.description, "A fast, secure, and free web browser");
        assert_eq!(detail.homepage, "https://www.google.com/chrome");
        assert_eq!(detail.license, "Proprietary");
    }

    #[test]
    fn parse_german_show_output() {
        let backend = CliBackend::new();
        let output = "\
Gefunden Google Chrome [Google.Chrome]
Version: 132.0.6834
Herausgeber: Google LLC
Beschreibung: Ein schneller, sicherer und kostenloser Webbrowser
Startseite: https://www.google.com/chrome
Lizenz: Proprietary
Quelle: winget
";
        let detail = backend.parse_show_output(output);
        assert_eq!(detail.id, "Google.Chrome");
        assert_eq!(detail.name, "Google Chrome");
        assert_eq!(detail.version, "132.0.6834");
        assert_eq!(detail.publisher, "Google LLC");
        assert_eq!(
            detail.description,
            "Ein schneller, sicherer und kostenloser Webbrowser"
        );
        assert_eq!(detail.homepage, "https://www.google.com/chrome");
        assert_eq!(detail.license, "Proprietary");
    }

    #[test]
    fn parse_german_list_table_without_available() {
        let backend = CliBackend::new();
        let output = "\
Name                           ID                          Version  Quelle
---------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6  winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[0].source, "winget");
        assert!(packages[0].available_version.is_empty());
    }

    #[test]
    fn parse_upgrade_table_with_german_pin_message() {
        let backend = CliBackend::new();
        // Real output from winget upgrade with pinned packages (German locale)
        let output = "\
Name                           ID                          Version     Verfügbar   Quelle
-------------------------------------------------------------------------------------------------
RamMap                         Microsoft.Sysinternals.R... 1.61        1.62        winget
vc_clip                        vc_clip.vc_dir              2026.01.29              winget
2 Pakete verfügen über Pins, die ein Upgrade verhindern. Verwenden Sie den Befehl \"winget pin\", um Pins anzuzeigen und zu bearbeiten. Wenn Sie das --include-pinned-Argument verwenden, werden möglicherweise weitere Ergebnisse angezeigt.
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(
            packages.len(),
            2,
            "should parse only the package rows, not the pin message"
        );
        assert_eq!(packages[0].id, "Microsoft.Sysinternals.R...");
        assert_eq!(packages[0].version, "1.61");
        assert_eq!(packages[0].available_version, "1.62");
        assert_eq!(packages[1].id, "vc_clip.vc_dir");
    }

    #[test]
    fn parse_upgrade_table_with_english_pin_message() {
        let backend = CliBackend::new();
        // English version of pin message
        let output = "\
Name                           Id                          Version     Available   Source
-------------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
2 packages have pins that prevent upgrade. Use the \"winget pin\" command to view and edit pins. If you use the --include-pinned argument, additional results may be displayed.
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(
            packages.len(),
            1,
            "should parse only the package rows, not the pin message"
        );
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[0].available_version, "132.0.6834");
    }

    #[test]
    fn parse_upgrade_table_with_upgrades_available_footer() {
        let backend = CliBackend::new();
        // Footer message indicating number of upgrades
        let output = "\
Name                           Id                          Version     Available   Source
-------------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
Microsoft Visual Studio Code   Microsoft.VisualStudioCode  1.95.3      1.96.0      winget
2 upgrades available.
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(
            packages.len(),
            2,
            "should parse package rows, stopping at footer"
        );
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[1].id, "Microsoft.VisualStudioCode");
    }

    #[test]
    fn parse_upgrade_table_with_multiple_footer_lines() {
        let backend = CliBackend::new();
        // Multiple footer messages
        let output = "\
Name                           Id                          Version     Available   Source
-------------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
1 upgrade available.
2 packages have pins that prevent upgrade. Use the \"winget pin\" command to view and edit pins.
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 1, "should stop at first footer line");
        assert_eq!(packages[0].id, "Google.Chrome");
    }

    #[test]
    fn parse_table_with_digit_starting_package_name() {
        let backend = CliBackend::new();
        // 7-Zip starts with a digit — must NOT be treated as a footer line
        let output = "\
Name                                               Id                                                  Version          Available  Source
-----------------------------------------------------------------------------------------------------------------------------------------
7-Zip 25.01 (x64)                                  7zip.7zip                                           25.01                       winget
CPUID CPU-Z MSI 2.15                               CPUID.CPU-Z.MSI                                     2.15             2.18       winget
Docker Desktop                                     Docker.DockerDesktop                                4.56.0           4.59.0     winget
2 upgrades available.
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 3, "7-Zip must not be treated as footer");
        assert_eq!(packages[0].id, "7zip.7zip");
        assert_eq!(packages[0].name, "7-Zip 25.01 (x64)");
        assert_eq!(packages[1].id, "CPUID.CPU-Z.MSI");
        assert_eq!(packages[2].id, "Docker.DockerDesktop");
    }

    #[test]
    fn parse_table_with_truncated_ids() {
        let backend = CliBackend::new();
        // MSIX packages with truncated IDs (ending with …)
        let output = "\
Name                                  Id                                    Version
---------------------------------------------------------------------------------------
Bluesky                               MSIX\\bsky.app-C52C8C38_1.0.0.0_neutr\u{2026} 1.0.0.0
Slack                                 SlackTechnologies.Slack               4.48.92.0
Microsoft Windows Desktop Runtime 10.\u{2026} Microsoft.DotNet.DesktopRuntime.10   10.0.4
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 3);
        assert!(
            packages[0].is_truncated(),
            "truncated MSIX ID should be detected"
        );
        assert!(
            !packages[1].is_truncated(),
            "normal ID should not be truncated"
        );
        assert!(
            !packages[2].is_truncated(),
            "normal ID with truncated name should not be truncated"
        );
    }

    #[test]
    fn parse_table_long_footer_not_treated_as_package() {
        let backend = CliBackend::new();
        // A long localized footer whose text extends into the ID column area.
        // With filter (not take_while) + ID validation, this must not produce a package,
        // AND Chrome after it must still be parsed.
        let output = "\
Name                           Id                          Version     Available   Source
-------------------------------------------------------------------------------------------------
Google Chrome                  Google.Chrome               131.0.6778  132.0.6834  winget
2 Pakete verfuegen ueber Pins die ein Upgrade verhindern, ein Upgrade kann ueber winget durchgefuehrt
Microsoft Visual Studio Code   Microsoft.VisualStudioCode  1.95.3      1.96.0      winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(
            packages.len(),
            2,
            "footer must be skipped, but VS Code after it must be kept"
        );
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[1].id, "Microsoft.VisualStudioCode");
    }

    #[test]
    fn sanitize_strips_ansi_escape_from_package_name() {
        // Direct test of sanitize_text helper
        let dirty = "Evil\x1b]52;c;payload\x07App";
        let clean = super::sanitize_text(dirty);
        assert!(!clean.contains('\x1b'), "ESC must be stripped");
        assert!(!clean.contains('\x07'), "BEL must be stripped");
        assert_eq!(clean, "Evil]52;c;payloadApp");

        // Verify tab and newline are preserved
        assert_eq!(super::sanitize_text("a\tb\nc"), "a\tb\nc");

        // End-to-end: package table with embedded escape in name
        let backend = CliBackend::new();
        let output = "\
Name                           Id                          Version   Source
----------------------------------------------------------------------------------
Google\x1b[2JChrome            Google.Chrome               131.0     winget
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 1);
        assert!(
            !packages[0].name.contains('\x1b'),
            "ANSI escape must be stripped from parsed package name"
        );
    }

    // ── parse_sources_from_table ──────────────────────────────────────────────

    #[test]
    fn parse_sources_english() {
        let backend = CliBackend::new();
        let output = "\
Name    Argument                             Type
----------------------------------------------------
winget  https://winget.azureedge.net/cache   Microsoft.PreIndexed.Package
msstore https://storeedgefd.dsx.mp.microsoft.com/v9.0  Microsoft.Rest
";
        let sources = backend.parse_sources_from_table(output);
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].name, "winget");
        assert_eq!(sources[0].url, "https://winget.azureedge.net/cache");
        assert_eq!(sources[0].source_type, "Microsoft.PreIndexed.Package");
        assert_eq!(sources[1].name, "msstore");
    }

    #[test]
    fn parse_sources_empty_output_returns_empty() {
        let backend = CliBackend::new();
        let sources = backend.parse_sources_from_table("");
        assert!(sources.is_empty(), "empty output should yield no sources");
    }

    #[test]
    fn parse_sources_no_separator_returns_empty() {
        let backend = CliBackend::new();
        let output = "winget  https://example.com  SomeType\n";
        let sources = backend.parse_sources_from_table(output);
        assert!(
            sources.is_empty(),
            "missing separator should yield no sources"
        );
    }

    #[test]
    fn parse_sources_positional_fallback_for_unknown_locale() {
        let backend = CliBackend::new();
        // Headers in an unrecognized language trigger positional fallback
        let output = "\
Nom     Argument                    Type
--------------------------------------------
winget  https://winget.example.com  SomeType
";
        // "Nom" is French for "Name" — not in the known list, triggers positional
        let sources = backend.parse_sources_from_table(output);
        // The positional fallback assigns col 0=name, col 1=arg, col 2=type
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "winget");
        assert_eq!(sources[0].url, "https://winget.example.com");
        assert_eq!(sources[0].source_type, "SomeType");
    }

    // ── parse_show_output edge cases ──────────────────────────────────────────

    #[test]
    fn parse_show_output_multiline_description() {
        let backend = CliBackend::new();
        // Description spans multiple indented continuation lines
        let output = "\
Found Visual Studio Code [Microsoft.VisualStudioCode]
Version: 1.96.0
Publisher: Microsoft Corporation
Description: Visual Studio Code is a code editor redefined and optimized
  for building and debugging modern web and cloud applications.
  Visual Studio Code is free and available on your favorite platform.
License: Freeware
";
        let detail = backend.parse_show_output(output);
        assert_eq!(detail.id, "Microsoft.VisualStudioCode");
        assert_eq!(detail.version, "1.96.0");
        assert!(
            detail.description.contains("redefined and optimized"),
            "first description line should be present"
        );
        assert!(
            detail.description.contains("favorite platform"),
            "continuation lines should be appended to description"
        );
    }

    #[test]
    fn parse_show_output_publisher_url_fallback_for_homepage() {
        let backend = CliBackend::new();
        // When Homepage is absent, Publisher Url should fill it
        let output = "\
Found Some App [SomePublisher.SomeApp]
Version: 2.0.0
Publisher: Some Publisher
Publisher Url: https://somepublisher.example.com
License: MIT
";
        let detail = backend.parse_show_output(output);
        assert_eq!(detail.id, "SomePublisher.SomeApp");
        assert_eq!(
            detail.homepage, "https://somepublisher.example.com",
            "Publisher Url should be used as homepage when Homepage is absent"
        );
    }

    #[test]
    fn parse_show_output_homepage_not_overwritten_by_publisher_url() {
        let backend = CliBackend::new();
        // When Homepage is already set, Publisher Url must NOT overwrite it
        let output = "\
Found Some App [SomePublisher.SomeApp]
Version: 2.0.0
Publisher: Some Publisher
Homepage: https://explicit-homepage.example.com
Publisher Url: https://somepublisher.example.com
License: MIT
";
        let detail = backend.parse_show_output(output);
        assert_eq!(
            detail.homepage, "https://explicit-homepage.example.com",
            "explicit Homepage must take precedence over Publisher Url"
        );
    }

    #[test]
    fn parse_table_no_separator_returns_empty() {
        let backend = CliBackend::new();
        // No separator line → should return empty Vec, not panic
        let output = "\
Name         Id             Version
Google Chrome  Google.Chrome  131.0
";
        let packages = backend.parse_packages_from_table(output);
        assert!(
            packages.is_empty(),
            "missing separator should return empty Vec"
        );
    }

    // ── clean_output ─────────────────────────────────────────────────────────

    #[test]
    fn clean_output_strips_crlf_and_progress_overwrites() {
        // \r\n line endings are normalized to \n
        let input = "line1\r\nline2\r\nline3\r\n";
        assert_eq!(
            super::CliBackend::clean_output(input),
            "line1\nline2\nline3"
        );

        // Embedded \r progress overwrites — keep final segment after last \r
        let input = "-\rloading\r\\ \rpackages table";
        assert_eq!(super::CliBackend::clean_output(input), "packages table");

        // Mixed: \r\n lines mixed with embedded-\r progress spinner lines
        let input = "spinning\rHeader\r\nName    Id\r\n-\r\\  \r|  \r-\r   data row\r\n";
        let result = super::CliBackend::clean_output(input);
        // "spinning\rHeader" → "Header"
        // "Name    Id" → unchanged
        // "-\r\\  \r|  \r-\r   data row" → "   data row"
        assert_eq!(result, "Header\nName    Id\n   data row");

        // No \r at all — output unchanged
        let plain = "line1\nline2\nline3\n";
        assert_eq!(
            super::CliBackend::clean_output(plain),
            "line1\nline2\nline3"
        );

        // Empty input
        assert_eq!(super::CliBackend::clean_output(""), "");
    }

    // ── is_winget_footer_line ─────────────────────────────────────────────────

    #[test]
    fn is_footer_line_detects_count_lines() {
        // Standard English footer
        assert!(super::is_winget_footer_line("2 upgrades available."));
        // German pin-message footer
        assert!(super::is_winget_footer_line(
            "2 Pakete verfügen über Pins, die ein Upgrade verhindern."
        ));
        // Single-package footer
        assert!(super::is_winget_footer_line("1 upgrade available."));
        // Large count
        assert!(super::is_winget_footer_line("123 packages found."));
    }

    #[test]
    fn is_footer_line_does_not_match_digit_prefixed_package_names() {
        // "7-Zip" starts with digit but next char is '-', not ' '
        assert!(!super::is_winget_footer_line("7-Zip 25.01 (x64)"));
        // "3DMark" — digit followed by a letter
        assert!(!super::is_winget_footer_line(
            "3DMark                    Futuremark.3DMark"
        ));
        // Ordinary package name
        assert!(!super::is_winget_footer_line("Google Chrome"));
        // Empty line
        assert!(!super::is_winget_footer_line(""));
        // Leading whitespace still checks trimmed content
        assert!(super::is_winget_footer_line("  2 upgrades available."));
        assert!(!super::is_winget_footer_line("  7-Zip 25.01 (x64)"));
    }
}
