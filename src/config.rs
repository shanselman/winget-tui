/// Startup configuration loaded from an optional config file.
///
/// On Windows the file lives at `%APPDATA%\winget-tui\config.toml`.
/// On other platforms (useful for testing) it falls back to
/// `$HOME/.config/winget-tui/config.toml`.
///
/// Supported keys (all optional):
/// ```toml
/// default_view   = "installed"   # "installed" | "search" | "upgrades"
/// default_source = "all"         # "all" | "winget" | "msstore"
/// ```
use crate::app::AppMode;
use crate::models::SourceFilter;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub default_view: AppMode,
    pub default_source: SourceFilter,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_view: AppMode::Installed,
            default_source: SourceFilter::All,
        }
    }
}

impl Config {
    /// Load config from the platform config path, falling back to defaults
    /// for any missing or unrecognised keys.  Never returns an error — a
    /// missing or malformed file is silently ignored.
    pub fn load() -> Self {
        let path = Self::config_path();
        let text = match path.and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(t) => t,
            None => return Self::default(),
        };
        Self::parse(&text)
    }

    /// Returns the platform-specific config file path, or `None` if the
    /// required environment variable is not set.
    fn config_path() -> Option<std::path::PathBuf> {
        // Windows: %APPDATA%\winget-tui\config.toml
        if let Ok(appdata) = std::env::var("APPDATA") {
            return Some(
                std::path::PathBuf::from(appdata)
                    .join("winget-tui")
                    .join("config.toml"),
            );
        }
        // Fallback for non-Windows (dev / CI)
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                std::path::PathBuf::from(home)
                    .join(".config")
                    .join("winget-tui")
                    .join("config.toml"),
            );
        }
        None
    }

    /// Parse a minimal subset of TOML: bare `key = "value"` lines only.
    /// Comments (`#`), blank lines, and unrecognised keys are skipped.
    fn parse(text: &str) -> Self {
        let mut cfg = Self::default();
        for line in text.lines() {
            let line = line.trim();
            // Skip comments and blank lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim();
            match key {
                "default_view" => {
                    cfg.default_view = match value {
                        "search" => AppMode::Search,
                        "upgrades" => AppMode::Upgrades,
                        _ => AppMode::Installed,
                    };
                }
                "default_source" => {
                    cfg.default_source = match value {
                        "winget" => SourceFilter::Winget,
                        "msstore" => SourceFilter::MsStore,
                        _ => SourceFilter::All,
                    };
                }
                _ => {}
            }
        }
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_installed_all() {
        let cfg = Config::default();
        assert_eq!(cfg.default_view, AppMode::Installed);
        assert_eq!(cfg.default_source, SourceFilter::All);
    }

    #[test]
    fn parse_empty_string_returns_defaults() {
        let cfg = Config::parse("");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn parse_default_view_search() {
        let cfg = Config::parse(r#"default_view = "search""#);
        assert_eq!(cfg.default_view, AppMode::Search);
        assert_eq!(cfg.default_source, SourceFilter::All);
    }

    #[test]
    fn parse_default_view_upgrades() {
        let cfg = Config::parse(r#"default_view = "upgrades""#);
        assert_eq!(cfg.default_view, AppMode::Upgrades);
    }

    #[test]
    fn parse_default_source_winget() {
        let cfg = Config::parse(r#"default_source = "winget""#);
        assert_eq!(cfg.default_source, SourceFilter::Winget);
    }

    #[test]
    fn parse_default_source_msstore() {
        let cfg = Config::parse(r#"default_source = "msstore""#);
        assert_eq!(cfg.default_source, SourceFilter::MsStore);
    }

    #[test]
    fn parse_both_keys() {
        let input = "default_view = \"upgrades\"\ndefault_source = \"winget\"\n";
        let cfg = Config::parse(input);
        assert_eq!(cfg.default_view, AppMode::Upgrades);
        assert_eq!(cfg.default_source, SourceFilter::Winget);
    }

    #[test]
    fn parse_ignores_comments_and_blank_lines() {
        let input = "\
# This is a comment
default_view = \"search\"

# another comment
default_source = \"msstore\"
";
        let cfg = Config::parse(input);
        assert_eq!(cfg.default_view, AppMode::Search);
        assert_eq!(cfg.default_source, SourceFilter::MsStore);
    }

    #[test]
    fn parse_unknown_value_falls_back_to_default() {
        let cfg = Config::parse("default_view = \"unknown_value\"");
        assert_eq!(cfg.default_view, AppMode::Installed);
    }

    #[test]
    fn parse_unknown_key_is_ignored() {
        let cfg = Config::parse("unknown_key = \"foo\"");
        assert_eq!(cfg, Config::default());
    }
}
