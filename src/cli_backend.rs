use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use unicode_width::UnicodeWidthChar;

use tokio::process::Command;

use crate::backend::WingetBackend;
use crate::models::{Package, PackageDetail, Source};

pub struct CliBackend;

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

        if !output.status.success() {
            if strict || stdout.trim().is_empty() {
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
        }

        // winget uses \r to overwrite progress spinners in-place, and outputs
        // \r\n line endings on Windows. Resolve carriage returns: first normalize
        // line endings, then for lines with embedded \r (progress overwrites),
        // keep only the last segment.
        let cleaned: String = stdout
            .replace("\r\n", "\n")
            .split('\n')
            .map(|line| {
                if line.contains('\r') {
                    // Progress overwrite: keep final segment after last \r
                    line.rsplit('\r').next().unwrap_or(line)
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(cleaned)
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

        lines[sep_idx + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            // Skip footer lines like "2 upgrades available." (digit(s) + space).
            // Uses filter (not take_while) so a false positive only skips one line
            // instead of silently dropping all remaining packages.
            .filter(|l| {
                let bytes = l.trim_start().as_bytes();
                let d = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
                !(d > 0 && d < bytes.len() && bytes[d] == b' ')
            })
            .filter_map(|line| self.parse_table_row(line, &col_positions))
            .collect()
    }

    fn detect_columns(header: &str) -> Vec<(&str, usize)> {
        let mut cols = Vec::new();
        let mut display_pos = 0usize;
        let mut byte_pos = 0usize;

        let chars: Vec<char> = header.chars().collect();
        let mut ci = 0;

        while ci < chars.len() {
            // Skip whitespace
            while ci < chars.len() && chars[ci] == ' ' {
                display_pos += 1;
                byte_pos += 1;
                ci += 1;
            }
            if ci >= chars.len() {
                break;
            }
            let start_display = display_pos;
            let start_byte = byte_pos;
            // Read until whitespace
            while ci < chars.len() && chars[ci] != ' ' {
                display_pos += chars[ci].width().unwrap_or(0);
                byte_pos += chars[ci].len_utf8();
                ci += 1;
            }
            let name = &header[start_byte..byte_pos];
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

    /// Normalize a `winget show` key to a canonical English name (case-insensitive,
    /// with known translations for common locales).
    fn normalize_show_key(key: &str) -> &'static str {
        match key.to_lowercase().as_str() {
            "version" | "packageversion" => "version",
            "publisher" | "herausgeber" | "éditeur" | "editore" | "editor" => "publisher",
            "description" | "beschreibung" | "descripción" | "descrição" | "descrizione"
                => "description",
            "homepage" | "startseite" => "homepage",
            "publisher url" | "herausgeber-url" => "publisher_url",
            "license" | "lizenz" | "licence" | "licencia" | "licença" | "licenza" => "license",
            "source" | "quelle" | "origen" | "fonte" | "origine" => "source",
            _ => "",
        }
    }

    fn parse_table_row(&self, line: &str, cols: &[(&str, usize)]) -> Option<Package> {
        // Extract fields using display-width columns (not byte offsets).
        // The header column positions are in display-width units (ASCII, so bytes == display width).
        // Data rows may contain multi-byte UTF-8 chars (e.g. '…') that are 1 display column
        // but 3 bytes, so we walk chars counting display width to find correct slice points.
        let get_field = |idx: usize| -> String {
            if idx >= cols.len() {
                return String::new();
            }
            let col_start = cols[idx].1; // display-width offset
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
        };

        // Find column indices by name — case-insensitive with known translations
        // to support non-English locales (e.g. German: ID, Verfügbar, Quelle)
        let mut name_idx = Self::find_column_ci(cols, &["name", "nom", "nombre", "nome"]);
        let mut id_idx = Self::find_column_ci(cols, &["id", "id."]);
        let mut ver_idx = Self::find_column_ci(cols, &[
            "version", "versión", "versão", "versione",
        ]);
        let mut source_idx = Self::find_column_ci(cols, &[
            "source", "quelle", "origen", "fonte", "origine",
        ]);
        let mut avail_idx = Self::find_column_ci(cols, &[
            "available", "verfügbar", "disponible", "disponível", "disponibile",
        ]);

        // Positional fallback for unrecognized locales (e.g. CJK)
        if id_idx.is_none() && cols.len() >= 4 {
            name_idx = name_idx.or(Some(0));
            id_idx = Some(1);
            ver_idx = ver_idx.or(Some(2));
            if cols.len() >= 5 {
                avail_idx = avail_idx.or(Some(3));
                source_idx = source_idx.or(Some(4));
            } else {
                source_idx = source_idx.or(Some(3));
            }
        }

        let id = id_idx.map(&get_field).unwrap_or_default();
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
            name: name_idx.map(&get_field).unwrap_or_default(),
            id,
            version: ver_idx.map(&get_field).unwrap_or_default(),
            source: source_idx.map(&get_field).unwrap_or_default(),
            available_version: avail_idx.map(&get_field).unwrap_or_default(),
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
                    detail.name = before_bracket
                        .split_once(' ')
                        .map(|(_, name)| name.trim().to_string())
                        .unwrap_or_default();
                    detail.id = trimmed[bracket_start + 1..bracket_end].to_string();
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
                        "version" => detail.version = value,
                        "publisher" => detail.publisher = value,
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
                            detail.description = desc;
                        }
                        "homepage" => detail.homepage = value,
                        "publisher_url" => {
                            if detail.homepage.is_empty() {
                                detail.homepage = value;
                            }
                        }
                        "license" => detail.license = value,
                        "source" => detail.source = value,
                        _ => {}
                    }
                }
            }
            i += 1;
        }

        detail
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

        lines[sep_idx + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| {
                let get_field = |idx: usize| -> String {
                    if idx >= col_positions.len() {
                        return String::new();
                    }
                    let col_start = col_positions[idx].1;
                    let col_end = if idx + 1 < col_positions.len() {
                        col_positions[idx + 1].1
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
                };

                let mut name_idx = Self::find_column_ci(&col_positions, &["name", "nom", "nombre", "nome"]);
                let mut arg_idx = Self::find_column_ci(&col_positions, &["argument"]);
                let mut type_idx = Self::find_column_ci(&col_positions, &["type", "typ", "tipo"]);

                // Positional fallback for unrecognized locales
                if name_idx.is_none() && col_positions.len() >= 3 {
                    name_idx = Some(0);
                    arg_idx = Some(1);
                    type_idx = Some(2);
                }

                let name = name_idx.map(&get_field).unwrap_or_default();
                if name.is_empty() {
                    return None;
                }

                Some(Source {
                    name,
                    url: arg_idx.map(&get_field).unwrap_or_default(),
                    source_type: type_idx.map(&get_field).unwrap_or_default(),
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
        let mut args = vec!["install", "--id", id, "--accept-source-agreements", "--accept-package-agreements"];
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
        self.run_winget_strict(&["upgrade", "--id", id, "--accept-source-agreements", "--accept-package-agreements"])
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
        assert_eq!(detail.description, "Ein schneller, sicherer und kostenloser Webbrowser");
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
        assert_eq!(packages.len(), 2, "should parse only the package rows, not the pin message");
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
        assert_eq!(packages.len(), 1, "should parse only the package rows, not the pin message");
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
        assert_eq!(packages.len(), 2, "should parse package rows, stopping at footer");
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
";
        let packages = backend.parse_packages_from_table(output);
        assert_eq!(packages.len(), 2);
        assert!(packages[0].is_truncated(), "truncated MSIX ID should be detected");
        assert!(!packages[1].is_truncated(), "normal ID should not be truncated");
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
        assert_eq!(packages.len(), 2, "footer must be skipped, but VS Code after it must be kept");
        assert_eq!(packages[0].id, "Google.Chrome");
        assert_eq!(packages[1].id, "Microsoft.VisualStudioCode");
    }
}
