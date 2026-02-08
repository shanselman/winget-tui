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
        let output = Command::new("winget")
            .args(args)
            .output()
            .await
            .context("Failed to run winget. Is it installed?")?;

        // winget may return non-zero for "no results" — still valid
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() && stdout.trim().is_empty() {
            bail!("winget failed: {}", stderr.trim());
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
            // Stop at footer lines like "123 upgrades available."
            // These are short lines with a digit followed by a word — distinct from data rows
            // which span the full table width
            .take_while(|l| l.len() > 20 || !l.trim_start().starts_with(|c: char| c.is_ascii_digit()))
            .filter_map(|line| self.parse_table_row(line, &col_positions))
            .collect()
    }

    fn detect_columns(header: &str) -> Vec<(&str, usize)> {
        let mut cols = Vec::new();
        let mut i = 0;
        let bytes = header.as_bytes();

        while i < bytes.len() {
            // Skip whitespace
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i >= bytes.len() {
                break;
            }
            let start = i;
            // Read until whitespace
            while i < bytes.len() && bytes[i] != b' ' {
                i += 1;
            }
            let name = &header[start..i];
            cols.push((name, start));
        }
        cols
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

        // Find column indices by name
        let name_idx = cols.iter().position(|(n, _)| *n == "Name");
        let id_idx = cols.iter().position(|(n, _)| *n == "Id");
        let ver_idx = cols.iter().position(|(n, _)| *n == "Version");
        let source_idx = cols.iter().position(|(n, _)| *n == "Source");
        let avail_idx = cols.iter().position(|(n, _)| *n == "Available");

        let id = id_idx.map(|i| get_field(i)).unwrap_or_default();
        if id.is_empty() {
            return None;
        }

        Some(Package {
            name: name_idx.map(|i| get_field(i)).unwrap_or_default(),
            id,
            version: ver_idx.map(|i| get_field(i)).unwrap_or_default(),
            source: source_idx.map(|i| get_field(i)).unwrap_or_default(),
            available_version: avail_idx.map(|i| get_field(i)).unwrap_or_default(),
        })
    }

    fn parse_show_output(&self, output: &str) -> PackageDetail {
        let mut detail = PackageDetail::default();

        let lines: Vec<&str> = output.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim();

            // Parse "Found Name [Id]" header line
            if trimmed.starts_with("Found ") {
                if let Some(bracket_start) = trimmed.rfind('[') {
                    if let Some(bracket_end) = trimmed.rfind(']') {
                        detail.name = trimmed[6..bracket_start].trim().to_string();
                        detail.id = trimmed[bracket_start + 1..bracket_end].to_string();
                    }
                }
                i += 1;
                continue;
            }

            // Parse "Key: Value" lines (only top-level, not indented)
            if !line.starts_with(' ') && !line.starts_with('\t') {
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim();
                    let value = value.trim().to_string();
                    match key {
                        "Version" | "PackageVersion" => detail.version = value,
                        "Publisher" => detail.publisher = value,
                        "Description" => {
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
                        "Homepage" => detail.homepage = value,
                        "Publisher Url" => {
                            if detail.homepage.is_empty() {
                                detail.homepage = value;
                            }
                        }
                        "License" => detail.license = value,
                        "Source" => detail.source = value,
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

                let name_idx = col_positions.iter().position(|(n, _)| *n == "Name");
                let arg_idx = col_positions.iter().position(|(n, _)| *n == "Argument");
                let type_idx = col_positions.iter().position(|(n, _)| *n == "Type");

                let name = name_idx.map(|i| get_field(i)).unwrap_or_default();
                if name.is_empty() {
                    return None;
                }

                Some(Source {
                    name,
                    url: arg_idx.map(|i| get_field(i)).unwrap_or_default(),
                    source_type: type_idx.map(|i| get_field(i)).unwrap_or_default(),
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

    async fn list_upgrades(&self) -> Result<Vec<Package>> {
        let args = vec!["upgrade", "--accept-source-agreements"];
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
        self.run_winget(&args).await
    }

    async fn uninstall(&self, id: &str) -> Result<String> {
        self.run_winget(&["uninstall", "--id", id, "--accept-source-agreements"])
            .await
    }

    async fn upgrade(&self, id: &str) -> Result<String> {
        self.run_winget(&["upgrade", "--id", id, "--accept-source-agreements", "--accept-package-agreements"])
            .await
    }

    async fn list_sources(&self) -> Result<Vec<Source>> {
        let output = self.run_winget(&["source", "list"]).await?;
        Ok(self.parse_sources_from_table(&output))
    }
}
