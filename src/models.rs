use std::fmt;

use serde::Deserialize;

/// Column to sort the package list by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    /// No explicit sort; winget's natural order is preserved.
    #[default]
    None,
    Name,
    Id,
    Version,
}

impl SortField {
    // No cycle helper needed; App::cycle_sort implements the full state machine.
}

impl fmt::Display for SortField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "—"),
            Self::Name => write!(f, "Name"),
            Self::Id => write!(f, "ID"),
            Self::Version => write!(f, "Version"),
        }
    }
}

/// Sort direction for the package list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

impl SortDir {
    pub fn indicator(self) -> &'static str {
        match self {
            Self::Asc => " ↑",
            Self::Desc => " ↓",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PinState {
    #[default]
    None,
    Pinned,
    Blocking,
    Gating(String),
}

impl PinState {
    pub fn is_pinned(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn short_marker(&self) -> &'static str {
        if self.is_pinned() {
            "📌 "
        } else {
            ""
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::None => "Not pinned".to_string(),
            Self::Pinned => "Pinned for upgrade-all".to_string(),
            Self::Blocking => "Blocked from upgrades".to_string(),
            Self::Gating(version) => format!("Pinned to {version}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinFilter {
    #[default]
    All,
    PinnedOnly,
    UnpinnedOnly,
}

impl PinFilter {
    pub fn cycle(&self) -> Self {
        match self {
            Self::All => Self::PinnedOnly,
            Self::PinnedOnly => Self::UnpinnedOnly,
            Self::UnpinnedOnly => Self::All,
        }
    }

    pub fn matches(&self, pin_state: &PinState) -> bool {
        match self {
            Self::All => true,
            Self::PinnedOnly => pin_state.is_pinned(),
            Self::UnpinnedOnly => !pin_state.is_pinned(),
        }
    }
}

impl fmt::Display for PinFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "Pins: all"),
            Self::PinnedOnly => write!(f, "Pins: only 📌"),
            Self::UnpinnedOnly => write!(f, "Pins: hide 📌"),
        }
    }
}

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
    #[serde(skip, default)]
    pub pin_state: PinState,
}

impl Package {
    /// Returns true if the package ID was truncated by winget.
    ///
    /// winget truncates long IDs with either a Unicode ellipsis (`…`) or three
    /// ASCII dots (`...`), depending on the terminal and locale. Either form
    /// must be treated as truncated; using such an ID with `winget show --exact`
    /// or any mutating command will always fail.
    pub fn is_truncated(&self) -> bool {
        self.id.ends_with('…') || self.id.ends_with("...")
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
    pub release_notes_url: String,
    pub pin_state: PinState,
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
            release_notes_url: pick(self.release_notes_url, &base.release_notes_url),
            pin_state: if self.pin_state.is_pinned() {
                self.pin_state
            } else {
                base.pin_state.clone()
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackagePin {
    pub id: String,
    pub pin_state: PinState,
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
    Pin { id: String },
    Unpin { id: String },
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
            Self::Pin { id } => write!(f, "Pinning {id}"),
            Self::Unpin { id } => write!(f, "Unpinning {id}"),
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
            pin_state: PinState::None,
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
    fn source_filter_as_arg_all_returns_none() {
        assert_eq!(SourceFilter::All.as_arg(), None);
    }

    #[test]
    fn source_filter_as_arg_winget_returns_some() {
        assert_eq!(SourceFilter::Winget.as_arg(), Some("winget"));
    }

    #[test]
    fn source_filter_as_arg_msstore_returns_some() {
        assert_eq!(SourceFilter::MsStore.as_arg(), Some("msstore"));
    }

    #[test]
    fn source_filter_display() {
        assert_eq!(SourceFilter::All.to_string(), "All");
        assert_eq!(SourceFilter::Winget.to_string(), "winget");
        assert_eq!(SourceFilter::MsStore.to_string(), "msstore");
    }

    #[test]
    fn pin_filter_cycle() {
        assert_eq!(PinFilter::All.cycle(), PinFilter::PinnedOnly);
        assert_eq!(PinFilter::PinnedOnly.cycle(), PinFilter::UnpinnedOnly);
        assert_eq!(PinFilter::UnpinnedOnly.cycle(), PinFilter::All);
    }

    #[test]
    fn pin_state_helpers() {
        assert!(!PinState::None.is_pinned());
        assert!(PinState::Pinned.is_pinned());
        assert_eq!(PinState::Blocking.label(), "Blocked from upgrades");
        assert_eq!(
            PinState::Gating("1.2.*".to_string()).label(),
            "Pinned to 1.2.*"
        );
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
    fn is_truncated_ascii_dots_suffix() {
        // winget produces ASCII "..." truncation on some terminals/locales
        assert!(pkg("Microsoft.Sysinternals.R...").is_truncated());
    }

    #[test]
    fn is_truncated_two_dots_not_truncated() {
        // Two dots at the end should not be considered truncated
        assert!(!pkg("Some.Package..").is_truncated());
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
            release_notes_url: String::new(),
            pin_state: PinState::None,
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
        assert_eq!(
            merged.name, "Google Chrome",
            "base name should be preserved"
        );
        assert_eq!(merged.version, "132.0", "base version should be preserved");
        assert_eq!(merged.source, "winget", "base source should be preserved");
        assert_eq!(merged.publisher, "Google LLC", "fresh publisher should win");
        assert_eq!(
            merged.description, "A fast browser",
            "fresh description should win"
        );
    }

    #[test]
    fn merge_over_release_notes_url_prefers_fresh() {
        let fresh = PackageDetail {
            release_notes_url: "https://example.com/releases/v2".to_string(),
            ..PackageDetail::default()
        };
        let base = PackageDetail {
            release_notes_url: "https://example.com/releases/v1".to_string(),
            ..PackageDetail::default()
        };
        let merged = fresh.merge_over(&base);
        assert_eq!(merged.release_notes_url, "https://example.com/releases/v2");
    }

    // ── SortField / SortDir ───────────────────────────────────────────────────

    #[test]
    fn sort_field_default_is_none() {
        assert_eq!(SortField::default(), SortField::None);
    }

    #[test]
    fn sort_dir_default_is_asc() {
        assert_eq!(SortDir::default(), SortDir::Asc);
    }

    #[test]
    fn sort_field_display() {
        assert_eq!(SortField::None.to_string(), "—");
        assert_eq!(SortField::Name.to_string(), "Name");
        assert_eq!(SortField::Id.to_string(), "ID");
        assert_eq!(SortField::Version.to_string(), "Version");
    }

    #[test]
    fn sort_dir_indicator() {
        assert_eq!(SortDir::Asc.indicator(), " ↑");
        assert_eq!(SortDir::Desc.indicator(), " ↓");
    }
}
