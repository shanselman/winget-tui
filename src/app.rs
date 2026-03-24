use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ratatui::layout::Rect;

use crate::backend::WingetBackend;
use crate::models::{OpResult, Operation, Package, PackageDetail, SourceFilter};

/// Stores UI layout regions for mouse hit-testing
#[derive(Debug, Default, Clone)]
pub struct LayoutRegions {
    pub tab_bar: Rect,
    pub filter_bar: Rect,
    pub search_bar: Rect,
    pub package_list: Rect,
    pub detail_panel: Rect,
    /// Y offset where the first data row starts in the package list (after header + border)
    pub list_content_y: u16,
    /// Tab click regions: (start_x, end_x, mode)
    pub tab_regions: Vec<(u16, u16, AppMode)>,
}

/// Messages sent from background tasks back to the UI
#[derive(Debug)]
pub enum AppMessage {
    PackagesLoaded {
        generation: u64,
        packages: Vec<Package>,
    },
    DetailLoaded {
        generation: u64,
        detail: PackageDetail,
    },
    OperationComplete(OpResult),
    StatusUpdate(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Search,
    Installed,
    Upgrades,
}

impl AppMode {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Search => Self::Installed,
            Self::Installed => Self::Upgrades,
            Self::Upgrades => Self::Search,
        }
    }

    pub fn cycle_back(&self) -> Self {
        match self {
            Self::Search => Self::Upgrades,
            Self::Installed => Self::Search,
            Self::Upgrades => Self::Installed,
        }
    }

    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Search => "Search",
            Self::Installed => "Installed",
            Self::Upgrades => "Upgrades",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
}

/// Confirmation dialog state
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    pub message: String,
    pub operation: Operation,
}

pub struct App {
    pub mode: AppMode,
    pub input_mode: InputMode,
    pub source_filter: SourceFilter,
    pub search_query: String,
    pub packages: Vec<Package>,
    pub filtered_packages: Vec<Package>,
    pub selected: usize,
    pub detail: Option<PackageDetail>,
    pub detail_loading: bool,
    pub status_message: String,
    pub loading: bool,
    pub confirm: Option<ConfirmDialog>,
    pub show_help: bool,
    pub should_quit: bool,
    pub layout: LayoutRegions,
    /// Scroll offset of the package list table (set during rendering)
    pub table_scroll_offset: usize,
    /// Tick counter for animations (spinner, etc.)
    pub tick: usize,
    /// Incremented on each view refresh; stale results are discarded
    pub view_generation: u64,
    /// Incremented on each detail load; stale results are discarded
    pub detail_generation: u64,
    /// Cache of package details to avoid repeated winget show calls
    pub detail_cache: HashMap<String, PackageDetail>,
    /// Indices into filtered_packages that are selected for batch operations
    pub selected_packages: HashSet<usize>,
    pub backend: Arc<dyn WingetBackend>,
    pub message_tx: tokio::sync::mpsc::UnboundedSender<AppMessage>,
    pub message_rx: tokio::sync::mpsc::UnboundedReceiver<AppMessage>,
}

impl App {
    pub fn new(backend: Arc<dyn WingetBackend>) -> Self {
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            mode: AppMode::Installed,
            input_mode: InputMode::Normal,
            source_filter: SourceFilter::All,
            search_query: String::new(),
            packages: Vec::new(),
            filtered_packages: Vec::new(),
            selected: 0,
            detail: None,
            detail_loading: false,
            status_message: "Loading...".to_string(),
            loading: false,
            confirm: None,
            show_help: false,
            should_quit: false,
            layout: LayoutRegions::default(),
            table_scroll_offset: 0,
            tick: 0,
            view_generation: 0,
            detail_generation: 0,
            detail_cache: HashMap::new(),
            selected_packages: HashSet::new(),
            backend,
            message_tx,
            message_rx,
        }
    }

    pub fn apply_filter(&mut self) {
        // Client-side text filter is active when in Installed/Upgrades mode with a non-empty query.
        let text_query = self.search_query.to_ascii_lowercase();
        let local_text_filter = matches!(self.mode, AppMode::Installed | AppMode::Upgrades)
            && !text_query.is_empty();

        // When a source filter is active, winget already filters server-side
        // (and omits the Source column), so accept all returned packages.
        let source_filtered: Vec<Package> = if self.source_filter == SourceFilter::All {
            self.packages
                .iter()
                .filter(|p| self.source_filter.matches(&p.source))
                .cloned()
                .collect()
        } else {
            self.packages.clone()
        };

        self.filtered_packages = if local_text_filter {
            source_filtered
                .into_iter()
                .filter(|p| {
                    p.name.to_ascii_lowercase().contains(&text_query)
                        || p.id.to_ascii_lowercase().contains(&text_query)
                })
                .collect()
        } else {
            source_filtered
        };

        // Keep selection in bounds
        if self.selected >= self.filtered_packages.len() {
            self.selected = self.filtered_packages.len().saturating_sub(1);
        }
        // Clear multi-select since indices are now stale
        self.selected_packages.clear();
    }

    pub fn selected_package(&self) -> Option<&Package> {
        self.filtered_packages.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.filtered_packages.is_empty() {
            return;
        }
        let len = self.filtered_packages.len() as isize;
        let new = (self.selected as isize + delta).rem_euclid(len);
        self.selected = new as usize;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    pub fn spinner(&self) -> char {
        const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[self.tick % FRAMES.len()]
    }

    pub fn refresh_view(&mut self) {
        self.view_generation += 1;
        let generation = self.view_generation;
        let backend = self.backend.clone();
        let tx = self.message_tx.clone();
        let mode = self.mode;
        let query = self.search_query.clone();
        let source_arg = match &self.source_filter {
            SourceFilter::Winget => Some("winget".to_string()),
            SourceFilter::MsStore => Some("msstore".to_string()),
            SourceFilter::All => None,
        };

        tokio::spawn(async move {
            let result = match mode {
                AppMode::Search => {
                    if query.is_empty() {
                        Ok(Vec::new())
                    } else {
                        backend.search(&query, source_arg.as_deref()).await
                    }
                }
                AppMode::Installed => backend.list_installed(source_arg.as_deref()).await,
                AppMode::Upgrades => backend.list_upgrades(source_arg.as_deref()).await,
            };

            match result {
                Ok(packages) => {
                    let _ = tx.send(AppMessage::PackagesLoaded {
                        generation,
                        packages,
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                }
            }
        });
    }

    pub fn load_detail(&mut self, id: &str) {
        // Always increment generation to invalidate any in-flight detail requests.
        // Without this, returning from cache leaves the old generation active,
        // and a stale async response can overwrite the correct cached detail.
        self.detail_generation += 1;

        // Return cached detail immediately if available
        if let Some(cached) = self.detail_cache.get(id) {
            self.detail = Some(cached.clone());
            self.detail_loading = false;
            return;
        }

        // Pre-populate from Package list data for instant feedback
        if let Some(pkg) = self.filtered_packages.iter().find(|p| p.id == id) {
            self.detail = Some(PackageDetail {
                id: pkg.id.clone(),
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                source: pkg.source.clone(),
                ..PackageDetail::default()
            });
        }

        self.detail_loading = true;
        let generation = self.detail_generation;
        let backend = self.backend.clone();
        let tx = self.message_tx.clone();
        let id = id.to_string();

        tokio::spawn(async move {
            match backend.show(&id).await {
                Ok(detail) => {
                    let _ = tx.send(AppMessage::DetailLoaded { generation, detail });
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::Error(e.to_string()));
                }
            }
        });
    }

    pub fn execute_operation(&self, op: Operation) {
        let backend = self.backend.clone();
        let tx = self.message_tx.clone();

        tokio::spawn(async move {
            let result = match &op {
                Operation::Install { id, version } => backend.install(id, version.as_deref()).await,
                Operation::Uninstall { id } => backend.uninstall(id).await,
                Operation::Upgrade { id } => backend.upgrade(id).await,
                Operation::BatchUpgrade { ids } => {
                    // Execute sequentially to avoid Windows Installer conflicts
                    let total = ids.len();
                    let mut failures: Vec<String> = Vec::new();
                    for (i, id) in ids.iter().enumerate() {
                        let _ = tx.send(AppMessage::StatusUpdate(format!(
                            "Upgrading {}/{}: {}...",
                            i + 1,
                            total,
                            id
                        )));
                        if let Err(e) = backend.upgrade(id).await {
                            failures.push(format!("{}: {}", id, e));
                        }
                    }
                    if failures.is_empty() {
                        Ok(format!("All {} packages upgraded successfully", total))
                    } else {
                        Err(anyhow::anyhow!(
                            "{}/{} succeeded, {} failed: {}",
                            total - failures.len(),
                            total,
                            failures.len(),
                            failures.join("; ")
                        ))
                    }
                }
            };

            let op_result = match result {
                Ok(msg) => OpResult {
                    operation: op,
                    success: true,
                    message: msg,
                },
                Err(e) => OpResult {
                    operation: op,
                    success: false,
                    message: e.to_string(),
                },
            };

            let _ = tx.send(AppMessage::OperationComplete(op_result));
        });
    }

    pub fn process_messages(&mut self) {
        while let Ok(msg) = self.message_rx.try_recv() {
            match msg {
                AppMessage::PackagesLoaded {
                    generation,
                    packages,
                } => {
                    // Discard stale results from a previous view/search
                    if generation < self.view_generation {
                        continue;
                    }
                    self.packages = packages;
                    self.apply_filter();
                    self.loading = false;
                    let count = self.filtered_packages.len();
                    self.set_status(format!(
                        "{count} package{} found",
                        if count == 1 { "" } else { "s" }
                    ));
                    // Auto-load detail for first selected package
                    if let Some(pkg) = self.selected_package() {
                        let id = pkg.id.clone();
                        self.load_detail(&id);
                    }
                }
                AppMessage::DetailLoaded { generation, detail } => {
                    // Discard stale detail from a previous selection
                    if generation < self.detail_generation {
                        continue;
                    }
                    // Merge: if winget show returned empty fields, keep pre-populated data
                    let merged = if let Some(existing) = &self.detail {
                        PackageDetail {
                            id: if detail.id.is_empty() {
                                existing.id.clone()
                            } else {
                                detail.id.clone()
                            },
                            name: if detail.name.is_empty() {
                                existing.name.clone()
                            } else {
                                detail.name.clone()
                            },
                            version: if detail.version.is_empty() {
                                existing.version.clone()
                            } else {
                                detail.version.clone()
                            },
                            source: if detail.source.is_empty() {
                                existing.source.clone()
                            } else {
                                detail.source.clone()
                            },
                            publisher: detail.publisher.clone(),
                            description: detail.description.clone(),
                            homepage: detail.homepage.clone(),
                            license: detail.license.clone(),
                        }
                    } else {
                        detail
                    };
                    // Cache for instant retrieval on revisit
                    if !merged.id.is_empty() {
                        self.detail_cache.insert(merged.id.clone(), merged.clone());
                    }
                    self.detail = Some(merged);
                    self.detail_loading = false;
                }
                AppMessage::OperationComplete(result) => {
                    // Invalidate cache for the affected package(s)
                    match &result.operation {
                        Operation::Install { id, .. }
                        | Operation::Uninstall { id }
                        | Operation::Upgrade { id } => {
                            self.detail_cache.remove(id);
                        }
                        Operation::BatchUpgrade { ids } => {
                            for id in ids {
                                self.detail_cache.remove(id);
                            }
                            self.selected_packages.clear();
                        }
                    }
                    if result.success {
                        self.set_status(format!("{} — done", result.operation));
                    } else {
                        self.set_status(format!(
                            "{} — failed: {}",
                            result.operation, result.message
                        ));
                    }
                    self.loading = false;
                    // Refresh the view after operation completes
                    self.refresh_view();
                }
                AppMessage::Error(msg) => {
                    self.set_status(format!("Error: {msg}"));
                    self.loading = false;
                }
                AppMessage::StatusUpdate(msg) => {
                    self.set_status(msg);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;

    use crate::backend::WingetBackend;
    use crate::models::{Package, PackageDetail, Source, SourceFilter};

    use super::{App, AppMode};

    struct StubBackend;

    #[async_trait]
    impl WingetBackend for StubBackend {
        async fn search(&self, _: &str, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn list_installed(&self, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn list_upgrades(&self, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn show(&self, _: &str) -> Result<PackageDetail> {
            Ok(PackageDetail::default())
        }
        async fn install(&self, _: &str, _: Option<&str>) -> Result<String> {
            Ok(String::new())
        }
        async fn uninstall(&self, _: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn upgrade(&self, _: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn list_sources(&self) -> Result<Vec<Source>> {
            Ok(vec![])
        }
    }

    fn make_pkg(id: &str, name: &str) -> Package {
        Package {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0".to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
        }
    }

    fn app_with_packages(mode: AppMode, pkgs: Vec<Package>) -> App {
        let mut app = App::new(Arc::new(StubBackend));
        app.mode = mode;
        app.packages = pkgs;
        app
    }

    #[test]
    fn local_text_filter_matches_by_name() {
        let mut app = app_with_packages(
            AppMode::Installed,
            vec![
                make_pkg("Google.Chrome", "Google Chrome"),
                make_pkg("Microsoft.Edge", "Microsoft Edge"),
            ],
        );
        app.search_query = "chrome".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 1);
        assert_eq!(app.filtered_packages[0].id, "Google.Chrome");
    }

    #[test]
    fn local_text_filter_matches_by_id() {
        let mut app = app_with_packages(
            AppMode::Upgrades,
            vec![
                make_pkg("Google.Chrome", "Google Chrome"),
                make_pkg("Microsoft.Edge", "Microsoft Edge"),
            ],
        );
        app.search_query = "microsoft".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 1);
        assert_eq!(app.filtered_packages[0].id, "Microsoft.Edge");
    }

    #[test]
    fn local_text_filter_is_case_insensitive() {
        let mut app = app_with_packages(
            AppMode::Installed,
            vec![make_pkg("Google.Chrome", "Google Chrome")],
        );
        app.search_query = "CHROME".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 1);
    }

    #[test]
    fn local_text_filter_empty_query_shows_all() {
        let mut app = app_with_packages(
            AppMode::Installed,
            vec![
                make_pkg("Google.Chrome", "Google Chrome"),
                make_pkg("Microsoft.Edge", "Microsoft Edge"),
            ],
        );
        app.search_query = String::new();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 2);
    }

    #[test]
    fn local_text_filter_not_applied_in_search_mode() {
        // In Search mode, the query is for winget search — apply_filter should
        // not do client-side filtering.
        let mut app = app_with_packages(
            AppMode::Search,
            vec![
                make_pkg("Google.Chrome", "Google Chrome"),
                make_pkg("Microsoft.Edge", "Microsoft Edge"),
            ],
        );
        app.source_filter = SourceFilter::All;
        app.search_query = "chrome".to_string();
        app.apply_filter();
        // Both packages should remain; text filtering is not applied in Search mode.
        assert_eq!(app.filtered_packages.len(), 2);
    }

    #[test]
    fn local_text_filter_no_match_returns_empty() {
        let mut app = app_with_packages(
            AppMode::Installed,
            vec![make_pkg("Google.Chrome", "Google Chrome")],
        );
        app.search_query = "firefox".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 0);
    }

    #[test]
    fn selection_clamped_when_filter_reduces_list() {
        let mut app = app_with_packages(
            AppMode::Installed,
            vec![
                make_pkg("A", "Apple"),
                make_pkg("B", "Banana"),
                make_pkg("C", "Cherry"),
            ],
        );
        app.selected = 2; // pointing at "Cherry"
        app.search_query = "apple".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 1);
        assert_eq!(app.selected, 0); // clamped to last valid index
    }
}
