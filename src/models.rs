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
    #[serde(
        alias = "PackageVersion",
        alias = "Version",
        default
    )]
    pub version: String,
    #[serde(alias = "Source", default)]
    pub source: String,
    /// Only present in upgrade listings
    #[serde(alias = "AvailableVersion", default)]
    pub available_version: String,
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
    /// Available version when viewing an upgrade (empty otherwise)
    pub available_version: String,
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
