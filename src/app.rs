use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ratatui::layout::Rect;
use ratatui::widgets::TableState;

use crate::backend::WingetBackend;
use crate::config::Config;
use crate::models::{
    OpResult, Operation, Package, PackageDetail, PackagePin, PinFilter, SortDir, SortField,
    SourceFilter,
};

/// Stores UI layout regions for mouse hit-testing
#[derive(Debug, Default, Clone)]
pub struct LayoutRegions {
    pub tab_bar: Rect,
    pub search_bar: Rect,
    pub package_list: Rect,
    pub detail_panel: Rect,
    /// Y offset where the first data row starts in the package list (after header + border)
    pub list_content_y: u16,
    /// Tab click regions: (start_x, end_x, mode)
    pub tab_regions: Vec<(u16, u16, AppMode)>,
}

/// Which panel currently has keyboard focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusZone {
    PackageList,
    DetailPanel,
}

impl FocusZone {
    pub fn toggle(self) -> Self {
        match self {
            Self::PackageList => Self::DetailPanel,
            Self::DetailPanel => Self::PackageList,
        }
    }
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
    /// Inline prompt for typing a specific version before installing
    VersionInput,
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
    pub focus: FocusZone,
    pub source_filter: SourceFilter,
    pub pin_filter: PinFilter,
    pub search_query: String,
    pub packages: Vec<Package>,
    pub filtered_packages: Vec<Package>,
    pub selected: usize,
    pub detail: Option<PackageDetail>,
    pub detail_loading: bool,
    pub status_message: String,
    pub loading: bool,
    pub confirm: Option<ConfirmDialog>,
    /// Version string being edited in the VersionInput prompt
    pub version_input: String,
    pub show_help: bool,
    pub should_quit: bool,
    pub layout: LayoutRegions,
    /// Sort field for the package list table.
    pub sort_field: SortField,
    /// Sort direction for the package list table.
    pub sort_dir: SortDir,
    /// Persistent table widget state (preserves viewport offset across frames)
    pub table_state: TableState,
    /// Scroll offset of the detail panel (in rendered lines)
    pub detail_scroll: usize,
    /// Total rendered line count of the detail panel (set during rendering)
    pub detail_content_lines: usize,
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

/// Compare two package version strings numerically, component by component.
///
/// Version strings are split on `.`, `-`, and `+`. Each component is compared
/// numerically when both sides parse as `u64`; otherwise lexicographically.
/// This avoids the lexicographic pitfall where `"10.0"` sorts before `"2.0"`.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(v: &str) -> Vec<&str> {
        v.split(['.', '-', '+']).collect()
    }
    let parts_a = split(a);
    let parts_b = split(b);
    let len = parts_a.len().max(parts_b.len());
    for i in 0..len {
        let pa = parts_a.get(i).copied().unwrap_or("");
        let pb = parts_b.get(i).copied().unwrap_or("");
        let ord = match (pa.parse::<u64>(), pb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => pa.cmp(pb),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

impl App {
    fn annotate_pins(packages: &mut [Package], pins: Vec<PackagePin>) {
        let pin_map: HashMap<String, _> = pins
            .into_iter()
            .map(|pin| (pin.id, pin.pin_state))
            .collect();
        for pkg in packages {
            if let Some(pin_state) = pin_map.get(&pkg.id) {
                pkg.pin_state = pin_state.clone();
            }
        }
    }

    pub fn new(backend: Arc<dyn WingetBackend>, cfg: Config) -> Self {
        let (message_tx, message_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            mode: cfg.default_view,
            input_mode: InputMode::Normal,
            focus: FocusZone::PackageList,
            source_filter: cfg.default_source,
            pin_filter: PinFilter::All,
            search_query: String::new(),
            packages: Vec::new(),
            filtered_packages: Vec::new(),
            selected: 0,
            detail: None,
            detail_loading: false,
            status_message: "Loading...".to_string(),
            loading: false,
            confirm: None,
            version_input: String::new(),
            show_help: false,
            should_quit: false,
            layout: LayoutRegions::default(),
            sort_field: SortField::None,
            sort_dir: SortDir::Asc,
            table_state: TableState::default(),
            detail_scroll: 0,
            detail_content_lines: 0,
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
        // When a source filter is active, winget already filters server-side
        // (and omits the Source column), so accept all returned packages.
        // Backfill the source field when winget omitted it (single-source query).
        self.filtered_packages = self.packages.clone();
        if let Some(src) = self.source_filter.as_arg() {
            for pkg in &mut self.filtered_packages {
                if pkg.source.is_empty() {
                    pkg.source = src.to_string();
                }
            }
        }
        if self.mode != AppMode::Search {
            self.filtered_packages
                .retain(|pkg| self.pin_filter.matches(&pkg.pin_state));
        }
        // Apply sort if a field is selected.
        // sort_by_cached_key computes the key exactly once per element (O(N))
        // rather than on every comparison (O(N log N)), avoiding repeated heap
        // allocations from to_lowercase() for Name and Id sorts.
        match self.sort_field {
            SortField::None => {}
            SortField::Name => {
                self.filtered_packages
                    .sort_by_cached_key(|p| p.name.to_lowercase());
                if self.sort_dir == SortDir::Desc {
                    self.filtered_packages.reverse();
                }
            }
            SortField::Id => {
                self.filtered_packages
                    .sort_by_cached_key(|p| p.id.to_lowercase());
                if self.sort_dir == SortDir::Desc {
                    self.filtered_packages.reverse();
                }
            }
            SortField::Version => {
                self.filtered_packages
                    .sort_by(|a, b| compare_versions(&a.version, &b.version));
                if self.sort_dir == SortDir::Desc {
                    self.filtered_packages.reverse();
                }
            }
        }
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
        self.ensure_selection_visible();
    }

    /// Adjust the table viewport offset so the selected row is visible.
    pub fn ensure_selection_visible(&mut self) {
        let viewport_rows = self.layout.package_list.height.saturating_sub(3) as usize;
        if viewport_rows == 0 {
            return;
        }
        let offset = self.table_state.offset();
        if self.selected < offset {
            *self.table_state.offset_mut() = self.selected;
        } else if self.selected >= offset + viewport_rows {
            *self.table_state.offset_mut() = self.selected - viewport_rows + 1;
        }
    }

    /// Scroll the detail panel by `delta` lines, clamped to valid range.
    pub fn scroll_detail(&mut self, delta: isize) {
        let viewport = self.layout.detail_panel.height.saturating_sub(3) as usize;
        let max = self.detail_content_lines.saturating_sub(viewport);
        self.detail_scroll = (self.detail_scroll as isize + delta).clamp(0, max as isize) as usize;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
    }

    /// Advance through sort states: None → Name↑ → Name↓ → ID↑ → ID↓ → Version↑ → Version↓ → None → …
    pub fn cycle_sort(&mut self) {
        use crate::models::{SortDir, SortField};
        let (next_field, next_dir) = match (self.sort_field, self.sort_dir) {
            (SortField::None, _) => (SortField::Name, SortDir::Asc),
            (SortField::Name, SortDir::Asc) => (SortField::Name, SortDir::Desc),
            (SortField::Name, SortDir::Desc) => (SortField::Id, SortDir::Asc),
            (SortField::Id, SortDir::Asc) => (SortField::Id, SortDir::Desc),
            (SortField::Id, SortDir::Desc) => (SortField::Version, SortDir::Asc),
            (SortField::Version, SortDir::Asc) => (SortField::Version, SortDir::Desc),
            (SortField::Version, SortDir::Desc) => (SortField::None, SortDir::Asc),
        };
        self.sort_field = next_field;
        self.sort_dir = next_dir;
        self.apply_filter();
        let label = match self.sort_field {
            SortField::None => "Sort: none".to_string(),
            f => format!("Sort: {}{}", f, self.sort_dir.indicator()),
        };
        self.set_status(label);
    }

    pub fn cycle_pin_filter(&mut self) {
        self.pin_filter = self.pin_filter.cycle();
        self.selected = 0;
        self.apply_filter();
        self.ensure_selection_visible();
        self.set_status(format!("{}", self.pin_filter));
    }

    pub fn spinner(&self) -> char {
        const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        FRAMES[self.tick % FRAMES.len()]
    }

    fn ensure_detail_hint(detail: &mut PackageDetail) {
        if !detail.description.is_empty()
            || !detail.publisher.is_empty()
            || !detail.homepage.is_empty()
            || !detail.license.is_empty()
        {
            return;
        }

        let source = if detail.source.is_empty() {
            "the configured winget sources"
        } else if detail.source.eq_ignore_ascii_case("local") {
            "local install records"
        } else {
            "the package manifest"
        };

        detail.description =
            format!("Additional metadata is not available from {source} for this package.");
    }

    pub fn refresh_view(&mut self) {
        self.view_generation += 1;
        let generation = self.view_generation;
        let backend = self.backend.clone();
        let tx = self.message_tx.clone();
        let mode = self.mode;
        let query = self.search_query.clone();
        let source_arg = self.source_filter.as_arg();

        tokio::spawn(async move {
            let result = match mode {
                AppMode::Search => {
                    if query.is_empty() {
                        Ok(Vec::new())
                    } else {
                        backend.search(&query, source_arg).await
                    }
                }
                AppMode::Installed => backend.list_installed(source_arg).await,
                AppMode::Upgrades => backend.list_upgrades(source_arg).await,
            };

            match result {
                Ok(mut packages) => {
                    if mode != AppMode::Search {
                        match backend.list_pins().await {
                            Ok(pins) => Self::annotate_pins(&mut packages, pins),
                            Err(e) => {
                                let _ = tx.send(AppMessage::StatusUpdate(format!(
                                    "Pin info unavailable: {}",
                                    e
                                )));
                            }
                        }
                    }
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
        self.detail_generation += 1;

        // Return cached detail immediately if available
        if let Some(cached) = self.detail_cache.get(id) {
            self.detail = Some(cached.clone());
            self.detail_loading = false;
            return;
        }

        // Determine if this package can be looked up via `winget show --exact`.
        // Truncated IDs, ARP entries, and MSIX sideloads have no manifest so the
        // call would always fail. Show a local detail stub instead.
        let is_truncated = id.ends_with('…') || id.ends_with("...");
        let is_local = id.starts_with("ARP\\") || id.starts_with("MSIX\\");
        let pkg_source_empty = self
            .filtered_packages
            .iter()
            .find(|p| p.id == id)
            .is_some_and(|p| p.source.is_empty());

        if is_truncated || is_local || pkg_source_empty {
            if let Some(pkg) = self.filtered_packages.iter().find(|p| p.id == id) {
                let kind = if is_truncated {
                    "Package ID was truncated by winget"
                } else if id.starts_with("ARP\\") {
                    "Installed via Windows registry (Add/Remove Programs)"
                } else if id.starts_with("MSIX\\") {
                    "Installed as an MSIX/AppX package"
                } else {
                    "Installed locally (not from a winget source)"
                };
                let detail = PackageDetail {
                    id: pkg.id.clone(),
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    source: if pkg.source.is_empty() {
                        "local".to_string()
                    } else {
                        pkg.source.clone()
                    },
                    pin_state: pkg.pin_state.clone(),
                    description: format!(
                        "{}\n\n\
                         This package has no manifest in any configured winget source. \
                         Detailed metadata (publisher, homepage, license) is not available.\n\n\
                         To manage this package, use its original installer or \
                         the Windows Settings > Apps panel.",
                        kind
                    ),
                    ..PackageDetail::default()
                };
                self.detail_cache.insert(id.to_string(), detail.clone());
                self.detail = Some(detail);
            }
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
                pin_state: pkg.pin_state.clone(),
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
                Operation::Pin { id } => backend.pin(id).await,
                Operation::Unpin { id } => backend.unpin(id).await,
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
                    // Remember the currently selected package so we can
                    // re-anchor the cursor after the list is replaced.
                    let prev_id = self.selected_package().map(|p| p.id.clone());
                    self.packages = packages;
                    self.apply_filter();
                    // Restore cursor to the same package (if it is still present)
                    // so that pressing 'r' to refresh does not jump the cursor.
                    if let Some(id) = prev_id {
                        if let Some(idx) = self.filtered_packages.iter().position(|p| p.id == id) {
                            self.selected = idx;
                        }
                    }
                    self.loading = false;
                    let count = self.filtered_packages.len();
                    self.set_status(format!(
                        "{count} package{} found",
                        if count == 1 { "" } else { "s" }
                    ));
                    // Auto-load detail for the (restored) selected package
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
                        detail.merge_over(existing)
                    } else {
                        detail
                    };
                    let mut merged = merged;
                    Self::ensure_detail_hint(&mut merged);
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
                        | Operation::Upgrade { id }
                        | Operation::Pin { id }
                        | Operation::Unpin { id } => {
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
                    self.detail_loading = false;
                    if let Some(detail) = &mut self.detail {
                        Self::ensure_detail_hint(detail);
                    }
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

    use super::*;
    use crate::backend::WingetBackend;
    use crate::models::{Package, PackageDetail, PackagePin, PinState, Source};

    /// Minimal backend that records `show` calls
    struct SpyBackend {
        show_calls: std::sync::Mutex<Vec<String>>,
    }

    impl SpyBackend {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                show_calls: std::sync::Mutex::new(Vec::new()),
            })
        }

        fn show_calls(&self) -> Vec<String> {
            self.show_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl WingetBackend for SpyBackend {
        async fn search(&self, _: &str, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn list_installed(&self, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn list_upgrades(&self, _: Option<&str>) -> Result<Vec<Package>> {
            Ok(vec![])
        }
        async fn show(&self, id: &str) -> Result<PackageDetail> {
            self.show_calls.lock().unwrap().push(id.to_string());
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
        async fn list_pins(&self) -> Result<Vec<PackagePin>> {
            Ok(vec![])
        }
        async fn pin(&self, _: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn unpin(&self, _: &str) -> Result<String> {
            Ok(String::new())
        }
        async fn list_sources(&self) -> Result<Vec<Source>> {
            Ok(vec![])
        }
    }

    fn make_app(backend: Arc<dyn WingetBackend>) -> App {
        App::new(backend, crate::config::Config::default())
    }

    fn pkg(id: &str) -> Package {
        Package {
            id: id.to_string(),
            name: id.to_string(),
            version: "1.0".to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
            pin_state: PinState::None,
        }
    }

    /// Simulate receiving a PackagesLoaded message synchronously (bypasses tokio channel).
    fn deliver_packages(app: &mut App, packages: Vec<Package>) {
        let gen = app.view_generation;
        app.message_tx
            .send(AppMessage::PackagesLoaded {
                generation: gen,
                packages,
            })
            .unwrap();
        app.process_messages();
    }

    #[tokio::test]
    async fn packages_loaded_preserves_selection_by_id() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);

        // Load an initial list; select the second package (index 1 = VS Code)
        app.view_generation = 1;
        deliver_packages(
            &mut app,
            vec![
                pkg("Google.Chrome"),
                pkg("Microsoft.VisualStudioCode"),
                pkg("7zip.7zip"),
            ],
        );
        app.selected = 1;
        assert_eq!(
            app.selected_package().unwrap().id,
            "Microsoft.VisualStudioCode"
        );

        // Simulate refresh: list comes back re-ordered (Chrome is now at index 1)
        app.view_generation = 2;
        deliver_packages(
            &mut app,
            vec![
                pkg("7zip.7zip"),
                pkg("Google.Chrome"),
                pkg("Microsoft.VisualStudioCode"),
            ],
        );

        // Cursor must follow VS Code to its new index (2), not stay at index 1
        assert_eq!(
            app.selected, 2,
            "cursor should follow the package to its new position"
        );
        assert_eq!(
            app.selected_package().unwrap().id,
            "Microsoft.VisualStudioCode",
            "selected package must remain VS Code after refresh"
        );
    }

    #[tokio::test]
    async fn packages_loaded_keeps_bounds_when_selected_package_disappears() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);

        // Select the last package (index 2 = 7zip)
        app.view_generation = 1;
        deliver_packages(
            &mut app,
            vec![
                pkg("Google.Chrome"),
                pkg("Microsoft.VisualStudioCode"),
                pkg("7zip.7zip"),
            ],
        );
        app.selected = 2;

        // After refresh, 7zip is gone (e.g. it was uninstalled)
        app.view_generation = 2;
        deliver_packages(
            &mut app,
            vec![pkg("Google.Chrome"), pkg("Microsoft.VisualStudioCode")],
        );

        // selected must be clamped to the last valid index
        assert!(
            app.selected < app.filtered_packages.len(),
            "selection must remain in bounds after package disappears"
        );
    }

    #[test]
    fn load_detail_skips_truncated_id() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy.clone() as Arc<dyn WingetBackend>);
        let truncated = "MSIX\\bsky.app-C52C8C38_1.0.0.0_neutr\u{2026}";
        app.load_detail(truncated);
        // No show call should have been enqueued
        assert!(
            spy.show_calls().is_empty(),
            "winget show must not be called for truncated id"
        );
        assert!(
            !app.detail_loading,
            "should not be loading for truncated id"
        );
    }

    #[test]
    fn load_detail_skips_ascii_dot_truncated_id() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy.clone() as Arc<dyn WingetBackend>);
        // winget produces "..." ASCII truncation on some terminals
        let truncated = "Microsoft.Sysinternals.R...";
        app.load_detail(truncated);
        assert!(
            spy.show_calls().is_empty(),
            "winget show must not be called for ASCII-dot truncated id"
        );
        assert!(
            !app.detail_loading,
            "should not be loading for ASCII-dot truncated id"
        );
    }

    #[tokio::test]
    async fn load_detail_proceeds_for_normal_id() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy.clone() as Arc<dyn WingetBackend>);
        app.load_detail("Google.Chrome");
        // detail_generation was incremented — an async fetch was started
        assert_eq!(
            app.detail_generation, 1,
            "generation should advance for a normal id"
        );
    }

    // ── AppMode ───────────────────────────────────────────────────────────────

    #[test]
    fn app_mode_cycle_forward() {
        assert_eq!(AppMode::Search.cycle(), AppMode::Installed);
        assert_eq!(AppMode::Installed.cycle(), AppMode::Upgrades);
        assert_eq!(AppMode::Upgrades.cycle(), AppMode::Search);
    }

    #[test]
    fn app_mode_cycle_back() {
        assert_eq!(AppMode::Search.cycle_back(), AppMode::Upgrades);
        assert_eq!(AppMode::Installed.cycle_back(), AppMode::Search);
        assert_eq!(AppMode::Upgrades.cycle_back(), AppMode::Installed);
    }

    #[test]
    fn app_mode_label() {
        assert_eq!(AppMode::Search.label(), "Search");
        assert_eq!(AppMode::Installed.label(), "Installed");
        assert_eq!(AppMode::Upgrades.label(), "Upgrades");
    }

    // ── move_selection ────────────────────────────────────────────────────────

    fn make_packages(n: usize) -> Vec<Package> {
        (0..n)
            .map(|i| Package {
                id: format!("Pkg.{i}"),
                name: format!("Package {i}"),
                version: "1.0".to_string(),
                source: "winget".to_string(),
                available_version: String::new(),
                pin_state: PinState::None,
            })
            .collect()
    }

    #[test]
    fn move_selection_forward_one() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(5);
        app.filtered_packages = app.packages.clone();
        app.selected = 0;
        app.move_selection(1);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_selection_backward_one() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(5);
        app.filtered_packages = app.packages.clone();
        app.selected = 3;
        app.move_selection(-1);
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn move_selection_wraps_past_end() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(3);
        app.filtered_packages = app.packages.clone();
        app.selected = 2; // last item
        app.move_selection(1);
        assert_eq!(app.selected, 0, "should wrap to first item");
    }

    #[test]
    fn move_selection_wraps_past_start() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(3);
        app.filtered_packages = app.packages.clone();
        app.selected = 0;
        app.move_selection(-1);
        assert_eq!(app.selected, 2, "should wrap to last item");
    }

    #[test]
    fn move_selection_empty_list_is_noop() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.selected = 0;
        app.move_selection(1);
        assert_eq!(app.selected, 0);
        app.move_selection(-1);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn move_selection_large_delta_wraps_correctly() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(5);
        app.filtered_packages = app.packages.clone();
        app.selected = 1;
        // -20 from index 1 in a list of 5 → 1 + (-20) = -19 → rem_euclid(5) = 1
        app.move_selection(-20);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn ensure_selection_visible_scrolls_viewport_down() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(20);
        app.filtered_packages = app.packages.clone();
        app.layout.package_list.height = 8; // 5 visible rows after header/borders
        app.selected = 9;
        *app.table_state.offset_mut() = 0;

        app.ensure_selection_visible();

        assert_eq!(app.table_state.offset(), 5);
    }

    // ── selected_package ──────────────────────────────────────────────────────

    #[test]
    fn selected_package_returns_none_on_empty_list() {
        let spy = SpyBackend::new();
        let app = make_app(spy as Arc<dyn WingetBackend>);
        assert!(app.selected_package().is_none());
    }

    #[test]
    fn selected_package_returns_correct_package() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.filtered_packages = make_packages(3);
        app.selected = 1;
        assert_eq!(app.selected_package().unwrap().id, "Pkg.1");
    }

    // ── spinner ───────────────────────────────────────────────────────────────

    #[test]
    fn spinner_returns_valid_braille_char() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        for tick in 0..20 {
            app.tick = tick;
            assert!(
                FRAMES.contains(&app.spinner()),
                "spinner tick={tick} returned unexpected char"
            );
        }
    }

    fn make_package(name: &str, id: &str, version: &str) -> Package {
        Package {
            name: name.to_string(),
            id: id.to_string(),
            version: version.to_string(),
            source: "winget".to_string(),
            available_version: String::new(),
            pin_state: PinState::None,
        }
    }

    #[test]
    fn spinner_cycles_every_ten_ticks() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.tick = 0;
        let first = app.spinner();
        app.tick = 10;
        assert_eq!(
            app.spinner(),
            first,
            "spinner should return to the same frame every 10 ticks"
        );
    }

    // ── apply_filter ──────────────────────────────────────────────────────────

    #[test]
    fn apply_filter_clamps_selection_when_list_shrinks() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(5);
        app.filtered_packages = app.packages.clone();
        app.selected = 4;
        app.packages = make_packages(2);
        app.apply_filter();
        assert!(
            app.selected < 2,
            "selection should be within new list bounds"
        );
    }

    #[test]
    fn apply_filter_clears_multi_select() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = make_packages(3);
        app.filtered_packages = app.packages.clone();
        app.selected_packages = [0, 1, 2].iter().cloned().collect();
        app.apply_filter();
        assert!(
            app.selected_packages.is_empty(),
            "selected_packages should be cleared by apply_filter"
        );
    }

    #[test]
    fn apply_filter_backfills_source_when_server_omits_it() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.source_filter = SourceFilter::Winget;
        app.packages = vec![Package {
            name: "Pkg One".to_string(),
            id: "Pkg.One".to_string(),
            version: "1.0.0".to_string(),
            source: String::new(),
            available_version: String::new(),
            pin_state: PinState::None,
        }];

        app.apply_filter();

        assert_eq!(app.filtered_packages[0].source, "winget");
    }

    #[test]
    fn apply_filter_no_sort_preserves_order() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("Zebra", "Z.Zebra", "1.0"),
            make_package("Apple", "A.Apple", "2.0"),
        ];
        app.apply_filter();
        assert_eq!(app.filtered_packages[0].name, "Zebra");
        assert_eq!(app.filtered_packages[1].name, "Apple");
    }

    #[test]
    fn apply_filter_sort_by_name_asc() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("Zebra", "Z.Zebra", "1.0"),
            make_package("Apple", "A.Apple", "2.0"),
            make_package("Mango", "M.Mango", "1.5"),
        ];
        app.sort_field = crate::models::SortField::Name;
        app.sort_dir = crate::models::SortDir::Asc;
        app.apply_filter();
        let names: Vec<&str> = app
            .filtered_packages
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["Apple", "Mango", "Zebra"]);
    }

    #[test]
    fn apply_filter_sort_by_name_desc() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("Zebra", "Z.Zebra", "1.0"),
            make_package("Apple", "A.Apple", "2.0"),
            make_package("Mango", "M.Mango", "1.5"),
        ];
        app.sort_field = crate::models::SortField::Name;
        app.sort_dir = crate::models::SortDir::Desc;
        app.apply_filter();
        let names: Vec<&str> = app
            .filtered_packages
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["Zebra", "Mango", "Apple"]);
    }

    #[test]
    fn apply_filter_sort_by_id_asc() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("Z App", "Z.App", "1.0"),
            make_package("A App", "A.App", "1.0"),
        ];
        app.sort_field = crate::models::SortField::Id;
        app.sort_dir = crate::models::SortDir::Asc;
        app.apply_filter();
        assert_eq!(app.filtered_packages[0].id, "A.App");
        assert_eq!(app.filtered_packages[1].id, "Z.App");
    }

    #[test]
    fn apply_filter_sort_by_version_asc() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("B", "B.B", "2.0"),
            make_package("A", "A.A", "1.0"),
            make_package("C", "C.C", "3.0"),
        ];
        app.sort_field = crate::models::SortField::Version;
        app.sort_dir = crate::models::SortDir::Asc;
        app.apply_filter();
        let versions: Vec<&str> = app
            .filtered_packages
            .iter()
            .map(|p| p.version.as_str())
            .collect();
        assert_eq!(versions, ["1.0", "2.0", "3.0"]);
    }

    #[test]
    fn apply_filter_sort_by_version_numeric_multi_digit() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.packages = vec![
            make_package("A", "A.A", "10.0"),
            make_package("B", "B.B", "2.0"),
            make_package("C", "C.C", "1.9"),
        ];
        app.sort_field = crate::models::SortField::Version;
        app.sort_dir = crate::models::SortDir::Asc;
        app.apply_filter();
        let versions: Vec<&str> = app
            .filtered_packages
            .iter()
            .map(|p| p.version.as_str())
            .collect();
        assert_eq!(versions, ["1.9", "2.0", "10.0"]);
    }

    #[test]
    fn apply_filter_pinned_only_keeps_pinned_packages() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        let mut pinned = make_package("Pinned", "Pinned.App", "1.0");
        pinned.pin_state = PinState::Pinned;
        let unpinned = make_package("Regular", "Regular.App", "1.0");
        app.mode = AppMode::Installed;
        app.pin_filter = PinFilter::PinnedOnly;
        app.packages = vec![pinned, unpinned];
        app.apply_filter();
        assert_eq!(app.filtered_packages.len(), 1);
        assert_eq!(app.filtered_packages[0].id, "Pinned.App");
    }

    #[test]
    fn cycle_pin_filter_updates_state() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        assert_eq!(app.pin_filter, PinFilter::All);
        app.cycle_pin_filter();
        assert_eq!(app.pin_filter, PinFilter::PinnedOnly);
        app.cycle_pin_filter();
        assert_eq!(app.pin_filter, PinFilter::UnpinnedOnly);
    }

    // ── compare_versions ─────────────────────────────────────────────────────

    #[test]
    fn compare_versions_numeric_beats_lexicographic() {
        assert_eq!(compare_versions("2.0", "10.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_versions("1.9", "1.10"), std::cmp::Ordering::Less);
    }

    #[test]
    fn compare_versions_equal() {
        assert_eq!(
            compare_versions("1.2.3", "1.2.3"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn compare_versions_windows_quad() {
        assert_eq!(
            compare_versions("10.0.19041.0", "10.0.22621.0"),
            std::cmp::Ordering::Less
        );
    }

    // ── process_messages ──────────────────────────────────────────────────────

    #[test]
    fn process_messages_error_sets_status() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.detail_loading = true;
        app.message_tx
            .send(AppMessage::Error("something broke".to_string()))
            .unwrap();
        app.process_messages();
        assert!(
            app.status_message.contains("something broke"),
            "status should contain the error text"
        );
        assert!(
            !app.detail_loading,
            "detail loading should stop after an error"
        );
    }

    #[test]
    fn process_messages_status_update() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.message_tx
            .send(AppMessage::StatusUpdate("hello".to_string()))
            .unwrap();
        app.process_messages();
        assert_eq!(app.status_message, "hello");
    }

    #[tokio::test]
    async fn process_messages_batch_upgrade_completion_clears_multi_select() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.selected_packages = [0usize, 1usize].into_iter().collect();
        app.message_tx
            .send(AppMessage::OperationComplete(OpResult {
                operation: Operation::BatchUpgrade {
                    ids: vec!["Pkg.One".into(), "Pkg.Two".into()],
                },
                success: true,
                message: "done".into(),
            }))
            .unwrap();

        app.process_messages();

        assert!(app.selected_packages.is_empty());
        assert!(app.status_message.contains("done"));
    }

    #[tokio::test]
    async fn process_messages_packages_loaded_updates_list() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.view_generation = 1;
        let pkgs = make_packages(3);
        app.message_tx
            .send(AppMessage::PackagesLoaded {
                generation: 1,
                packages: pkgs,
            })
            .unwrap();
        app.process_messages();
        assert_eq!(app.filtered_packages.len(), 3);
        assert!(!app.loading);
    }

    #[test]
    fn process_messages_stale_packages_discarded() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.view_generation = 2;
        let pkgs = make_packages(3);
        app.message_tx
            .send(AppMessage::PackagesLoaded {
                generation: 1,
                packages: pkgs,
            })
            .unwrap();
        app.process_messages();
        assert!(
            app.filtered_packages.is_empty(),
            "stale packages should not update the list"
        );
    }

    #[test]
    fn process_messages_detail_loaded_updates_detail() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.detail_generation = 1;
        let detail = PackageDetail {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "132.0".to_string(),
            ..PackageDetail::default()
        };
        app.message_tx
            .send(AppMessage::DetailLoaded {
                generation: 1,
                detail,
            })
            .unwrap();
        app.process_messages();
        let loaded = app.detail.as_ref().expect("detail should be set");
        assert_eq!(loaded.id, "Google.Chrome");
        assert!(!app.detail_loading);
    }

    #[test]
    fn sparse_detail_gets_fallback_description() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.detail_generation = 1;
        app.detail = Some(PackageDetail {
            id: "ARP\\Machine\\X64\\Steam App 3065800".to_string(),
            name: "Marathon".to_string(),
            version: "Unknown".to_string(),
            source: "local".to_string(),
            ..PackageDetail::default()
        });
        app.message_tx
            .send(AppMessage::DetailLoaded {
                generation: 1,
                detail: PackageDetail::default(),
            })
            .unwrap();
        app.process_messages();
        let loaded = app.detail.as_ref().expect("detail should be set");
        assert!(
            loaded
                .description
                .contains("Additional metadata is not available"),
            "sparse details should explain why rich metadata is missing"
        );
    }

    #[test]
    fn process_messages_stale_detail_discarded() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.detail_generation = 3;
        let detail = PackageDetail {
            id: "Old.Package".to_string(),
            ..PackageDetail::default()
        };
        app.message_tx
            .send(AppMessage::DetailLoaded {
                generation: 2,
                detail,
            })
            .unwrap();
        app.process_messages();
        assert!(app.detail.is_none(), "stale detail should not be displayed");
    }

    // ── FocusZone ────────────────────────────────────────────────────────────

    #[test]
    fn focus_zone_toggle_list_to_detail() {
        assert_eq!(FocusZone::PackageList.toggle(), FocusZone::DetailPanel);
    }

    #[test]
    fn focus_zone_toggle_detail_to_list() {
        assert_eq!(FocusZone::DetailPanel.toggle(), FocusZone::PackageList);
    }

    #[test]
    fn focus_zone_toggle_is_involution() {
        let zone = FocusZone::PackageList;
        assert_eq!(zone.toggle().toggle(), zone);
    }

    // ── cycle_sort ────────────────────────────────────────────────────────────

    #[test]
    fn cycle_sort_progresses_through_all_states() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.cycle_sort();
        assert_eq!(app.sort_field, crate::models::SortField::Name);
        assert_eq!(app.sort_dir, crate::models::SortDir::Asc);
        app.cycle_sort();
        assert_eq!(app.sort_dir, crate::models::SortDir::Desc);
        app.cycle_sort();
        assert_eq!(app.sort_field, crate::models::SortField::Id);
        assert_eq!(app.sort_dir, crate::models::SortDir::Asc);
        app.cycle_sort();
        assert_eq!(app.sort_dir, crate::models::SortDir::Desc);
        app.cycle_sort();
        assert_eq!(app.sort_field, crate::models::SortField::Version);
        assert_eq!(app.sort_dir, crate::models::SortDir::Asc);
        app.cycle_sort();
        assert_eq!(app.sort_dir, crate::models::SortDir::Desc);
        app.cycle_sort();
        assert_eq!(app.sort_field, crate::models::SortField::None);
    }

    // ── scroll_detail ─────────────────────────────────────────────────────────

    #[test]
    fn scroll_detail_forward_clamps_at_max() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.layout.detail_panel.height = 10;
        app.detail_content_lines = 20;
        app.detail_scroll = 0;
        app.scroll_detail(100);
        assert_eq!(app.detail_scroll, 13);
    }

    #[test]
    fn scroll_detail_backward_clamps_at_zero() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.layout.detail_panel.height = 10;
        app.detail_content_lines = 20;
        app.detail_scroll = 5;
        app.scroll_detail(-100);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn scroll_detail_with_short_content_stays_at_zero() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.layout.detail_panel.height = 20;
        app.detail_content_lines = 5;
        app.detail_scroll = 0;
        app.scroll_detail(10);
        assert_eq!(app.detail_scroll, 0);
    }

    // ── detail_cache ──────────────────────────────────────────────────────────

    #[test]
    fn detail_loaded_is_cached() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy as Arc<dyn WingetBackend>);
        app.detail_generation = 1;
        let detail = PackageDetail {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "132.0".to_string(),
            ..PackageDetail::default()
        };
        app.message_tx
            .send(AppMessage::DetailLoaded {
                generation: 1,
                detail,
            })
            .unwrap();
        app.process_messages();
        assert!(app.detail_cache.contains_key("Google.Chrome"));
    }

    #[test]
    fn load_detail_uses_cache_on_second_call() {
        let spy = SpyBackend::new();
        let mut app = make_app(spy.clone() as Arc<dyn WingetBackend>);
        let cached = PackageDetail {
            id: "Google.Chrome".to_string(),
            name: "Google Chrome".to_string(),
            version: "132.0".to_string(),
            publisher: "Google LLC".to_string(),
            ..PackageDetail::default()
        };
        app.detail_cache
            .insert("Google.Chrome".to_string(), cached);
        let calls_before = spy.show_calls().len();
        app.load_detail("Google.Chrome");
        assert_eq!(spy.show_calls().len(), calls_before);
        assert_eq!(app.detail.as_ref().unwrap().name, "Google Chrome");
        assert!(!app.detail_loading);
    }
}
