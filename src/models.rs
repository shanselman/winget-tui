use std::fmt;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFilter {
    All,
    Winget,
    MsStore,
}

impl SourceFilter {
    pub fn cycle(&self) -> Self {
        match self {
            Self::All => Self::Winget,
            Self::Winget => Self::MsStore,
            Self::MsStore => Self::All,
        }
    }

    /// Returns the winget `--source` argument value for this filter,
    /// or `None` when all sources should be included.
    pub fn as_arg(&self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Winget => Some("winget"),
            Self::MsStore => Some("msstore"),
        }
    }
}

impl fmt::Display for SourceFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Winget => write!(f, "winget"),
            Self::MsStore => write!(f, "msstore"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    #[serde(alias = "PackageIdentifier", alias = "Id")]
    pub id: String,
    #[serde(alias = "PackageName", alias = "Name")]
    pub name: String,
    #[serde(alias = "PackageVersion", alias = "Version", default)]
    pub version: String,
    #[serde(alias = "Source", default)]
    pub source: String,
    /// Only present in upgrade listings
    #[serde(alias = "AvailableVersion", default)]
    pub available_version: String,
}

impl Package {
    /// Returns true if the package ID was truncated by winget (ends with '…').
    /// Truncated IDs cannot be used with `winget show --exact`.
    pub fn is_truncated(&self) -> bool {
        self.id.ends_with('…')
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackageDetail {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub source: String,
}

impl PackageDetail {
    /// Merge `self` (freshly loaded) with `base` (pre-populated stub), returning a combined
    /// detail where non-empty fields from `self` take precedence over `base`.
    ///
    /// This pattern is used when `winget show` completes: the stub from the package list
    /// provides instant `id`, `name`, `version`, and `source` before the async call returns,
    /// while the full response fills in `publisher`, `description`, `homepage`, and `license`.
    /// If winget returns empty values for any field, the stub's values are preserved.
    pub fn merge_over(self, base: &PackageDetail) -> PackageDetail {
        let pick = |fresh: String, fallback: &String| -> String {
            if fresh.is_empty() {
                fallback.clone()
            } else {
                fresh
            }
        };
        PackageDetail {
            id: pick(self.id, &base.id),
            name: pick(self.name, &base.name),
            version: pick(self.version, &base.version),
            source: pick(self.source, &base.source),
            publisher: pick(self.publisher, &base.publisher),
            description: pick(self.description, &base.description),
            homepage: pick(self.homepage, &base.homepage),
            license: pick(self.license, &base.license),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Source {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Argument", alias = "Url", default)]
    pub url: String,
    #[serde(alias = "Type", default)]
    pub source_type: String,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Install { id: String, version: Option<String> },
    Uninstall { id: String },
    Upgrade { id: String },
    BatchUpgrade { ids: Vec<String> },
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Install { id, version } => {
                if let Some(v) = version {
                    write!(f, "Installing {id} v{v}")
                } else {
                    write!(f, "Installing {id}")
                }
            }
            Self::Uninstall { id } => write!(f, "Uninstalling {id}"),
            Self::Upgrade { id } => write!(f, "Upgrading {id}"),
            Self::BatchUpgrade { ids } => write!(f, "Batch upgrading {} packages", ids.len()),
        }
    }
}

/// Result of a completed operation
#[derive(Debug, Clone)]
pub struct OpResult {
    pub operation: Operation,
    pub success: bool,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(id: &str) -> Package {
        Package {
            id: id.to_string(),
            name: "Test".to_string(),
            version: "1.0".to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
        }
    }

    // ── SourceFilter ──────────────────────────────────────────────────────────

    #[test]
    fn source_filter_cycle() {
        assert_eq!(SourceFilter::All.cycle(), SourceFilter::Winget);
        assert_eq!(SourceFilter::Winget.cycle(), SourceFilter::MsStore);
        assert_eq!(SourceFilter::MsStore.cycle(), SourceFilter::All);
    }

    #[test]
    fn source_filter_display() {
        assert_eq!(SourceFilter::All.to_string(), "All");
        assert_eq!(SourceFilter::Winget.to_string(), "winget");
        assert_eq!(SourceFilter::MsStore.to_string(), "msstore");
    }

    // ── Package::is_truncated ─────────────────────────────────────────────────

    #[test]
    fn is_truncated_normal_id() {
        assert!(!pkg("Google.Chrome").is_truncated());
    }

    #[test]
    fn is_truncated_ellipsis_suffix() {
        assert!(pkg("MSIX\\bsky.app-C52C8C38_1.0.0.0_neutr\u{2026}").is_truncated());
    }

    #[test]
    fn is_truncated_name_ellipsis_not_id() {
        assert!(!pkg("Microsoft.DotNet.DesktopRuntime.10").is_truncated());
    }

    // ── Operation::Display ────────────────────────────────────────────────────

    #[test]
    fn operation_display_install_with_version() {
        let op = Operation::Install {
            id: "Google.Chrome".to_string(),
            version: Some("132.0".to_string()),
        };
        assert_eq!(op.to_string(), "Installing Google.Chrome v132.0");
    }

    #[test]
    fn operation_display_install_without_version() {
        let op = Operation::Install {
            id: "Google.Chrome".to_string(),
            version: None,
        };
        assert_eq!(op.to_string(), "Installing Google.Chrome");
    }

    #[test]
    fn operation_display_uninstall() {
        let op = Operation::Uninstall {
            id: "Google.Chrome".to_string(),
        };
        assert_eq!(op.to_string(), "Uninstalling Google.Chrome");
    }

    #[test]
    fn operation_display_upgrade() {
        let op = Operation::Upgrade {
            id: "Google.Chrome".to_string(),
        };
        assert_eq!(op.to_string(), "Upgrading Google.Chrome");
    }

    #[test]
    fn operation_display_batch_upgrade() {
        let op = Operation::BatchUpgrade {
            ids: vec![
                "Google.Chrome".to_string(),
                "Microsoft.VisualStudioCode".to_string(),
            ],
        };
        assert_eq!(op.to_string(), "Batch upgrading 2 packages");
    }

    #[test]
    fn operation_display_batch_upgrade_zero() {
        let op = Operation::BatchUpgrade { ids: vec![] };
        assert_eq!(op.to_string(), "Batch upgrading 0 packages");
    }

    // ── PackageDetail::merge_over ─────────────────────────────────────────────

    #[test]
    fn merge_over_prefers_non_empty_fresh_fields() {
        let fresh = PackageDetail {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "132.0".to_string(),
            publisher: "Google LLC".to_string(),
            description: "A fast browser".to_string(),
            homepage: "https://google.com".to_string(),
            license: "Proprietary".to_string(),
            source: "winget".to_string(),
        };
        let base = PackageDetail {
            id: "OLD.ID".to_string(),
            name: "Old Name".to_string(),
            version: "1.0".to_string(),
            source: "msstore".to_string(),
            ..PackageDetail::default()
        };
        let merged = fresh.merge_over(&base);
        assert_eq!(merged.id, "Google.Chrome");
        assert_eq!(merged.name, "Google Chrome");
        assert_eq!(merged.version, "132.0");
        assert_eq!(merged.publisher, "Google LLC");
        assert_eq!(merged.source, "winget");
    }

    #[test]
    fn merge_over_falls_back_to_base_for_empty_fields() {
        let fresh = PackageDetail {
            publisher: "Google LLC".to_string(),
            description: "A fast browser".to_string(),
            ..PackageDetail::default()
        };
        let base = PackageDetail {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "132.0".to_string(),
            source: "winget".to_string(),
            ..PackageDetail::default()
        };
        let merged = fresh.merge_over(&base);
        assert_eq!(merged.id, "Google.Chrome", "base id should be preserved");
        assert_eq!(merged.name, "Google Chrome", "base name should be preserved");
        assert_eq!(merged.version, "132.0", "base version should be preserved");
        assert_eq!(merged.source, "winget", "base source should be preserved");
        assert_eq!(merged.publisher, "Google LLC", "fresh publisher should win");
        assert_eq!(merged.description, "A fast browser", "fresh description should win");
    }
}
