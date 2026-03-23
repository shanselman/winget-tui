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

    pub fn matches(&self, source: &str) -> bool {
        match self {
            Self::All => true,
            Self::Winget => source.eq_ignore_ascii_case("winget"),
            Self::MsStore => source.eq_ignore_ascii_case("msstore"),
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

    #[test]
    fn source_filter_cycle_wraps() {
        assert_eq!(SourceFilter::All.cycle(), SourceFilter::Winget);
        assert_eq!(SourceFilter::Winget.cycle(), SourceFilter::MsStore);
        assert_eq!(SourceFilter::MsStore.cycle(), SourceFilter::All);
    }

    #[test]
    fn source_filter_matches_case_insensitive() {
        assert!(SourceFilter::All.matches("winget"));
        assert!(SourceFilter::All.matches("msstore"));
        assert!(SourceFilter::All.matches("anything"));
        assert!(SourceFilter::Winget.matches("winget"));
        assert!(SourceFilter::Winget.matches("WINGET"));
        assert!(!SourceFilter::Winget.matches("msstore"));
        assert!(SourceFilter::MsStore.matches("msstore"));
        assert!(SourceFilter::MsStore.matches("MSSTORE"));
        assert!(!SourceFilter::MsStore.matches("winget"));
    }

    #[test]
    fn source_filter_display() {
        assert_eq!(SourceFilter::All.to_string(), "All");
        assert_eq!(SourceFilter::Winget.to_string(), "winget");
        assert_eq!(SourceFilter::MsStore.to_string(), "msstore");
    }

    #[test]
    fn package_is_truncated_detects_ellipsis() {
        let truncated = Package {
            id: "MSIX\\bsky.app-C52C8C38\u{2026}".to_string(),
            name: "Bluesky".to_string(),
            version: "1.0".to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
        };
        assert!(
            truncated.is_truncated(),
            "ID ending with '…' should be truncated"
        );

        let normal = Package {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "131.0".to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
        };
        assert!(!normal.is_truncated(), "Normal ID should not be truncated");
    }

    #[test]
    fn operation_display_install_with_version() {
        let op = Operation::Install {
            id: "Google.Chrome".to_string(),
            version: Some("131.0".to_string()),
        };
        assert_eq!(op.to_string(), "Installing Google.Chrome v131.0");
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
    fn operation_display_batch_upgrade_count() {
        let op = Operation::BatchUpgrade {
            ids: vec!["Google.Chrome".to_string(), "Mozilla.Firefox".to_string()],
        };
        assert_eq!(op.to_string(), "Batch upgrading 2 packages");

        let empty = Operation::BatchUpgrade { ids: vec![] };
        assert_eq!(empty.to_string(), "Batch upgrading 0 packages");
    }
}
