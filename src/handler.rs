use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::{App, AppMode, ConfirmDialog, InputMode};
use crate::models::Operation;

pub fn handle_events(app: &mut App) -> anyhow::Result<bool> {
    if !event::poll(std::time::Duration::from_millis(50))? {
        return Ok(false);
    }

    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            // Confirm dialog takes priority
            if app.confirm.is_some() {
                return handle_confirm(app, key.code);
            }

            // Version input prompt takes priority after confirm
            if app.input_mode == InputMode::VersionInput {
                return handle_version_input(app, key.code);
            }

            // Help overlay
            if app.show_help {
                match key.code {
                    KeyCode::Char('?') | KeyCode::Esc => app.show_help = false,
                    _ => {}
                }
                return Ok(false);
            }

            match app.input_mode {
                InputMode::Search => handle_search_input(app, key.code),
                InputMode::Normal => handle_normal_mode(app, key.code, key.modifiers),
                InputMode::VersionInput => unreachable!("handled above"),
            }
        }
        Event::Mouse(mouse) => handle_mouse(app, mouse),
        _ => Ok(false),
    }
}

fn handle_confirm(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    match key {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            if let Some(confirm) = app.confirm.take() {
                app.set_status(format!("{}...", confirm.operation));
                app.loading = true;
                app.execute_operation(confirm.operation);
            }
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.confirm = None;
            app.set_status("Cancelled");
        }
        _ => {}
    }
    Ok(false)
}

fn handle_version_input(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.version_input.clear();
            app.set_status("Cancelled");
        }
        KeyCode::Enter => {
            let version = app.version_input.trim().to_string();
            app.input_mode = InputMode::Normal;
            app.version_input.clear();
            if let Some(pkg) = app.selected_package() {
                let id = pkg.id.clone();
                let (msg, ver) = if version.is_empty() {
                    (format!("Install {}?", id), None)
                } else {
                    (format!("Install {} v{}?", id, version), Some(version))
                };
                app.confirm = Some(ConfirmDialog {
                    message: msg,
                    operation: Operation::Install { id, version: ver },
                });
            }
        }
        KeyCode::Backspace => {
            app.version_input.pop();
        }
        KeyCode::Char(c) => {
            app.version_input.push(c);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_search_input(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            if !app.search_query.is_empty() {
                app.mode = AppMode::Search;
                app.loading = true;
                app.set_status("Searching...");
                app.refresh_view();
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_normal_mode(
    app: &mut App,
    key: KeyCode,
    modifiers: KeyModifiers,
) -> anyhow::Result<bool> {
    match key {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('?') => {
            app.show_help = !app.show_help;
        }
        KeyCode::Tab | KeyCode::Right => {
            app.mode = app.mode.cycle();
            app.selected = 0;
            app.selected_packages.clear();
            app.detail = None;
            app.detail_loading = false;
            app.loading = true;
            app.set_status("Loading...");
            app.refresh_view();
        }
        KeyCode::BackTab | KeyCode::Left => {
            app.mode = app.mode.cycle_back();
            app.selected = 0;
            app.selected_packages.clear();
            app.detail = None;
            app.detail_loading = false;
            app.loading = true;
            app.set_status("Loading...");
            app.refresh_view();
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_selection(-1);
            load_detail_for_selected(app);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_selection(1);
            load_detail_for_selected(app);
        }
        KeyCode::PageUp => {
            app.move_selection(-20);
            load_detail_for_selected(app);
        }
        KeyCode::PageDown => {
            app.move_selection(20);
            load_detail_for_selected(app);
        }
        KeyCode::Home => {
            if !app.filtered_packages.is_empty() {
                app.selected = 0;
                load_detail_for_selected(app);
            }
        }
        KeyCode::End => {
            if !app.filtered_packages.is_empty() {
                app.selected = app.filtered_packages.len() - 1;
                load_detail_for_selected(app);
            }
        }

        // Search
        KeyCode::Char('/') | KeyCode::Char('s') => {
            app.input_mode = InputMode::Search;
        }

        // Filter — cycles source and re-fetches from winget
        KeyCode::Char('f') => {
            app.source_filter = app.source_filter.cycle();
            app.selected = 0;
            app.loading = true;
            app.set_status(format!("Filter: {} — loading...", app.source_filter));
            app.refresh_view();
        }

        // Refresh
        KeyCode::Char('r') => {
            app.loading = true;
            app.set_status("Refreshing...");
            app.refresh_view();
        }

        // Install
        KeyCode::Char('i') => {
            if let Some(pkg) = app.selected_package() {
                if pkg.is_truncated() {
                    app.set_status(
                        "Cannot install: package ID was truncated by winget — use winget directly",
                    );
                } else {
                    let id = pkg.id.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Install {}?", id),
                        operation: Operation::Install { id, version: None },
                    });
                }
            }
        }

        // Install specific version (Shift+I)
        KeyCode::Char('I') => {
            if let Some(pkg) = app.selected_package() {
                if pkg.is_truncated() {
                    app.set_status(
                        "Cannot install: package ID was truncated by winget — use winget directly",
                    );
                } else {
                    // Pre-fill with available_version if present, else current version
                    let prefill = if !pkg.available_version.is_empty() {
                        pkg.available_version.clone()
                    } else {
                        pkg.version.clone()
                    };
                    app.version_input = prefill;
                    app.input_mode = InputMode::VersionInput;
                }
            }
        }

        // Uninstall
        KeyCode::Char('x') => {
            if let Some(pkg) = app.selected_package() {
                if pkg.is_truncated() {
                    app.set_status(
                        "Cannot uninstall: package ID was truncated by winget — use winget directly",
                    );
                } else {
                    let id = pkg.id.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Uninstall {}?", id),
                        operation: Operation::Uninstall { id },
                    });
                }
            }
        }

        // Upgrade
        KeyCode::Char('u') => {
            if let Some(pkg) = app.selected_package() {
                if pkg.is_truncated() {
                    app.set_status(
                        "Cannot upgrade: package ID was truncated by winget — use winget directly",
                    );
                } else {
                    let id = pkg.id.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Upgrade {}?", id),
                        operation: Operation::Upgrade { id },
                    });
                }
            }
        }

        // Batch Upgrade (Shift+U) — upgrade all selected packages
        KeyCode::Char('U') => {
            if app.mode == AppMode::Upgrades && !app.selected_packages.is_empty() {
                let ids: Vec<String> = app
                    .selected_packages
                    .iter()
                    .filter_map(|&idx| {
                        app.filtered_packages
                            .get(idx)
                            .filter(|p| !p.is_truncated())
                            .map(|p| p.id.clone())
                    })
                    .collect();
                if ids.is_empty() {
                    app.set_status(
                        "Cannot upgrade: all selected packages have truncated IDs — use winget directly",
                    );
                } else {
                    let count = ids.len();
                    app.confirm = Some(ConfirmDialog {
                        message: format!(
                            "Upgrade {} selected package{}?",
                            count,
                            if count == 1 { "" } else { "s" }
                        ),
                        operation: Operation::BatchUpgrade { ids },
                    });
                }
            }
        }

        // Toggle selection (Space) — Upgrades mode only
        KeyCode::Char(' ') => {
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() {
                let idx = app.selected;
                if app.selected_packages.contains(&idx) {
                    app.selected_packages.remove(&idx);
                } else {
                    app.selected_packages.insert(idx);
                }
                // Move down after toggling for fast multi-select
                app.move_selection(1);
                load_detail_for_selected(app);
            }
        }

        // Select all / deselect all (a) — Upgrades mode only
        KeyCode::Char('a') => {
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() {
                if app.selected_packages.len() == app.filtered_packages.len() {
                    app.selected_packages.clear();
                } else {
                    app.selected_packages = (0..app.filtered_packages.len()).collect();
                }
            }
        }

        // Enter - load detail
        KeyCode::Enter => {
            load_detail_for_selected(app);
        }

        // Open homepage in default browser
        KeyCode::Char('o') => {
            if let Some(detail) = &app.detail {
                if !detail.homepage.is_empty() {
                    let url = detail.homepage.clone();
                    if open_url(&url) {
                        app.set_status(format!("Opening {}…", url));
                    } else {
                        app.set_status("Blocked: URL must start with http:// or https://");
                    }
                }
            }
        }

        _ => {}
    }
    Ok(false)
}

fn load_detail_for_selected(app: &mut App) {
    let pkg = app.selected_package();
    // Skip detail fetch for truncated IDs — winget show --exact will fail
    if let Some(pkg) = pkg {
        if pkg.is_truncated() {
            return;
        }
        let id = pkg.id.clone();
        app.load_detail(&id);
    }
}

/// Select the package row at the given terminal row coordinate and load its detail.
fn select_package_at_row(app: &mut App, row: u16) {
    let content_y = app.layout.list_content_y;
    if row >= content_y {
        let clicked_idx = (row - content_y) as usize + app.table_scroll_offset;
        if clicked_idx < app.filtered_packages.len() {
            app.selected = clicked_idx;
            load_detail_for_selected(app);
        }
    }
}

fn handle_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) -> anyhow::Result<bool> {
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Dismiss dialogs/help on click outside
            if app.show_help {
                app.show_help = false;
                return Ok(false);
            }
            if app.confirm.is_some() {
                app.confirm = None;
                app.set_status("Cancelled");
                return Ok(false);
            }

            // Click on tab bar — switch views
            if in_rect(col, row, app.layout.tab_bar) {
                handle_tab_click(app, col);
                return Ok(false);
            }

            // Click on search bar (check before filter since they share a row)
            if in_rect(col, row, app.layout.search_bar) {
                app.input_mode = InputMode::Search;
                return Ok(false);
            }

            // Click on filter area
            if in_rect(col, row, app.layout.filter_bar) {
                app.source_filter = app.source_filter.cycle();
                app.selected = 0;
                app.loading = true;
                app.set_status(format!("Filter: {} — loading...", app.source_filter));
                app.refresh_view();
                return Ok(false);
            }

            // Click on package list — select row or start scrollbar drag
            if in_rect(col, row, app.layout.package_list) {
                let list = app.layout.package_list;
                // Scrollbar is the rightmost column inside the border
                let scrollbar_col = list.x + list.width - 1;
                if col >= scrollbar_col.saturating_sub(1) && !app.filtered_packages.is_empty() {
                    // Click on scrollbar track — jump to proportional position
                    scrollbar_jump(app, row);
                    return Ok(false);
                }

                select_package_at_row(app, row);
                return Ok(false);
            }
        }

        // Scroll wheel in package list
        MouseEventKind::ScrollUp => {
            if in_rect(col, row, app.layout.package_list) {
                app.move_selection(-3);
                load_detail_for_selected(app);
            }
        }
        MouseEventKind::ScrollDown => {
            if in_rect(col, row, app.layout.package_list) {
                app.move_selection(3);
                load_detail_for_selected(app);
            }
        }

        // Right-click on a package shows context (loads detail)
        MouseEventKind::Down(MouseButton::Right) => {
            if in_rect(col, row, app.layout.package_list) {
                select_package_at_row(app, row);
            }
        }

        // Drag on scrollbar track
        MouseEventKind::Drag(MouseButton::Left) => {
            if in_rect(col, row, app.layout.package_list) && !app.filtered_packages.is_empty() {
                let list = app.layout.package_list;
                let scrollbar_col = list.x + list.width - 1;
                if col >= scrollbar_col.saturating_sub(1) {
                    scrollbar_jump(app, row);
                }
            }
        }

        _ => {}
    }

    Ok(false)
}

/// Map a Y position on the scrollbar track to a package index
fn scrollbar_jump(app: &mut App, row: u16) {
    let list = app.layout.package_list;
    // Track area: inside top and bottom borders
    let track_top = list.y + 1;
    let track_height = list.height.saturating_sub(2);
    if track_height == 0 || app.filtered_packages.is_empty() {
        return;
    }
    let clamped = row.clamp(track_top, track_top + track_height - 1);
    let ratio = (clamped - track_top) as f64 / (track_height - 1).max(1) as f64;
    let new_idx = (ratio * (app.filtered_packages.len() - 1) as f64).round() as usize;
    if new_idx != app.selected {
        app.selected = new_idx;
        load_detail_for_selected(app);
    }
}

/// Check if a coordinate is within a Rect
fn in_rect(col: u16, row: u16, rect: ratatui::layout::Rect) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Open a URL in the system default browser.
/// Only accepts http:// and https:// URLs to prevent command injection.
fn open_url(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(url).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    true
}

/// Determine which tab was clicked based on x position
fn handle_tab_click(app: &mut App, col: u16) {
    for &(start_x, end_x, mode) in &app.layout.tab_regions {
        if col >= start_x && col < end_x {
            if mode != app.mode {
                app.mode = mode;
                app.selected = 0;
                app.selected_packages.clear();
                app.detail = None;
                app.detail_loading = false;
                app.loading = true;
                app.set_status("Loading...");
                app.refresh_view();
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use crossterm::event::KeyCode;

    use super::*;
    use crate::app::InputMode;
    use crate::backend::WingetBackend;
    use crate::models::{Package, PackageDetail, Source};

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

    fn make_app_with_pkg(id: &str, version: &str, available: &str) -> App {
        let mut app = App::new(Arc::new(StubBackend));
        app.packages = vec![Package {
            id: id.to_string(),
            name: "Test Package".to_string(),
            version: version.to_string(),
            source: "winget".to_string(),
            available_version: available.to_string(),
        }];
        app.filtered_packages = app.packages.clone();
        app.selected = 0;
        app
    }

    // ── handle_version_input ─────────────────────────────────────────────────

    #[test]
    fn version_input_char_appends() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = "1.".to_string();
        let _ = handle_version_input(&mut app, KeyCode::Char('5'));
        assert_eq!(app.version_input, "1.5");
        assert_eq!(app.input_mode, InputMode::VersionInput);
    }

    #[test]
    fn version_input_backspace_removes_last_char() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = "1.5".to_string();
        let _ = handle_version_input(&mut app, KeyCode::Backspace);
        assert_eq!(app.version_input, "1.");
        assert_eq!(app.input_mode, InputMode::VersionInput);
    }

    #[test]
    fn version_input_backspace_on_empty_stays_empty() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = String::new();
        let _ = handle_version_input(&mut app, KeyCode::Backspace);
        assert_eq!(app.version_input, "");
    }

    #[test]
    fn version_input_escape_cancels_and_returns_to_normal() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = "2.0".to_string();
        let _ = handle_version_input(&mut app, KeyCode::Esc);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.version_input, "");
    }

    #[test]
    fn version_input_enter_with_version_creates_versioned_confirm() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = "2.0.1".to_string();
        let _ = handle_version_input(&mut app, KeyCode::Enter);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.version_input, "");
        let confirm = app.confirm.expect("confirm dialog should be set");
        assert!(confirm.message.contains("Test.App"));
        assert!(confirm.message.contains("2.0.1"));
        match confirm.operation {
            Operation::Install { id, version } => {
                assert_eq!(id, "Test.App");
                assert_eq!(version, Some("2.0.1".to_string()));
            }
            _ => panic!("expected Install operation"),
        }
    }

    #[test]
    fn version_input_enter_with_empty_version_installs_without_version() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = String::new();
        let _ = handle_version_input(&mut app, KeyCode::Enter);
        let confirm = app.confirm.expect("confirm dialog should be set");
        match confirm.operation {
            Operation::Install { version, .. } => {
                assert_eq!(version, None, "empty version should install latest");
            }
            _ => panic!("expected Install operation"),
        }
    }

    #[test]
    fn version_input_enter_trims_whitespace() {
        let mut app = make_app_with_pkg("Test.App", "1.0", "");
        app.input_mode = InputMode::VersionInput;
        app.version_input = "  2.0  ".to_string();
        let _ = handle_version_input(&mut app, KeyCode::Enter);
        let confirm = app.confirm.expect("confirm dialog should be set");
        match confirm.operation {
            Operation::Install { version, .. } => {
                assert_eq!(version, Some("2.0".to_string()), "version should be trimmed");
            }
            _ => panic!("expected Install operation"),
        }
    }
}
