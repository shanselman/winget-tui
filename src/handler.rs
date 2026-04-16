use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::{App, AppMode, ConfirmDialog, FocusZone, InputMode};
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
    let is_local_filter = app.mode != AppMode::Search;
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            if is_local_filter {
                // Clear filter and restore full list
                app.local_filter.clear();
                app.apply_filter();
                let count = app.filtered_packages.len();
                app.set_status(format!(
                    "{count} package{} shown",
                    if count == 1 { "" } else { "s" }
                ));
            }
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            if is_local_filter {
                // Filter is already applied in real-time; just close the input bar
                let count = app.filtered_packages.len();
                app.set_status(format!(
                    "{count} match{}",
                    if count == 1 { "" } else { "es" }
                ));
            } else if !app.search_query.is_empty() {
                app.loading = true;
                app.set_status("Searching...");
                app.refresh_view();
            }
        }
        KeyCode::Backspace => {
            if is_local_filter {
                app.local_filter.pop();
                app.apply_filter();
                let count = app.filtered_packages.len();
                app.set_status(format!(
                    "{count} match{}",
                    if count == 1 { "" } else { "es" }
                ));
            } else {
                app.search_query.pop();
            }
        }
        KeyCode::Char(c) => {
            if is_local_filter {
                app.local_filter.push(c);
                app.apply_filter();
                let count = app.filtered_packages.len();
                app.set_status(format!(
                    "{count} match{}",
                    if count == 1 { "" } else { "es" }
                ));
            } else {
                app.search_query.push(c);
            }
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

        // Left/Right switch views (Search/Installed/Upgrades)
        KeyCode::Left => {
            switch_view(app, app.mode.cycle_back());
        }
        KeyCode::Right => {
            switch_view(app, app.mode.cycle());
        }

        // Tab toggles focus between package list and detail panel
        KeyCode::Tab => {
            app.focus = app.focus.toggle();
        }
        KeyCode::BackTab => {
            app.focus = app.focus.toggle();
        }

        // Up/Down navigate the package list, or scroll detail panel when focused
        KeyCode::Up | KeyCode::Char('k') => {
            if app.focus == FocusZone::DetailPanel {
                app.scroll_detail(-1);
            } else {
                app.move_selection(-1);
                load_detail_for_selected(app);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.focus == FocusZone::DetailPanel {
                app.scroll_detail(1);
            } else {
                app.move_selection(1);
                load_detail_for_selected(app);
            }
        }
        KeyCode::PageUp => {
            if app.focus == FocusZone::DetailPanel {
                let page = app.layout.detail_panel.height.saturating_sub(3) as isize;
                app.scroll_detail(-page);
            } else {
                app.move_selection(-20);
                load_detail_for_selected(app);
            }
        }
        KeyCode::PageDown => {
            if app.focus == FocusZone::DetailPanel {
                let page = app.layout.detail_panel.height.saturating_sub(3) as isize;
                app.scroll_detail(page);
            } else {
                app.move_selection(20);
                load_detail_for_selected(app);
            }
        }
        KeyCode::Home => {
            if app.focus == FocusZone::DetailPanel {
                app.detail_scroll = 0;
            } else if !app.filtered_packages.is_empty() {
                app.selected = 0;
                load_detail_for_selected(app);
            }
        }
        KeyCode::End => {
            if app.focus == FocusZone::DetailPanel {
                let viewport = app.layout.detail_panel.height.saturating_sub(3) as usize;
                app.detail_scroll = app.detail_content_lines.saturating_sub(viewport);
            } else if !app.filtered_packages.is_empty() {
                app.selected = app.filtered_packages.len() - 1;
                load_detail_for_selected(app);
            }
        }

        // Enter: load detail for selected package
        KeyCode::Enter => {
            load_detail_for_selected(app);
        }

        // Search / Filter
        // In Search mode: focus the winget search input.
        // In Installed/Upgrades: open the local filter bar without switching views.
        KeyCode::Char('/') | KeyCode::Char('s') => {
            app.input_mode = InputMode::Search;
        }

        // Filter
        KeyCode::Char('f') => {
            app.source_filter = app.source_filter.cycle();
            app.selected = 0;
            app.loading = true;
            app.set_status(format!("Filter: {} -- loading...", app.source_filter));
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
                        "Cannot install: package ID was truncated by winget -- use winget directly",
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
                        "Cannot uninstall: package ID was truncated by winget -- use winget directly",
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
                        "Cannot upgrade: package ID was truncated by winget -- use winget directly",
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

        // Batch Upgrade (Shift+U)
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
                        "Cannot upgrade: all selected packages have truncated IDs -- use winget directly",
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

        // Toggle selection (Space)
        KeyCode::Char(' ') => {
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() {
                let idx = app.selected;
                if app.selected_packages.contains(&idx) {
                    app.selected_packages.remove(&idx);
                } else {
                    app.selected_packages.insert(idx);
                }
                app.move_selection(1);
                load_detail_for_selected(app);
            }
        }

        // Select all / deselect all
        KeyCode::Char('a') => {
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() {
                if app.selected_packages.len() == app.filtered_packages.len() {
                    app.selected_packages.clear();
                } else {
                    app.selected_packages = (0..app.filtered_packages.len()).collect();
                }
            }
        }

        // Open homepage
        KeyCode::Char('o') => {
            if let Some(detail) = &app.detail {
                if !detail.homepage.is_empty() {
                    let url = detail.homepage.clone();
                    if open_url(&url) {
                        app.set_status(format!("Opening {}...", url));
                    } else {
                        app.set_status("Blocked: URL must start with http:// or https://");
                    }
                }
            }
        }

        // Open release notes / changelog in default browser
        KeyCode::Char('c') => {
            if let Some(detail) = &app.detail {
                if !detail.release_notes_url.is_empty() {
                    let url = detail.release_notes_url.clone();
                    if open_url(&url) {
                        app.set_status(format!("Opening changelog {}…", url));
                    } else {
                        app.set_status("Blocked: URL must start with http:// or https://");
                    }
                }
            }
        }

        // Sort: cycle through Name↑ → Name↓ → ID↑ → ID↓ → Version↑ → Version↓ → None
        KeyCode::Char('S') => {
            app.cycle_sort();
        }

        _ => {}
    }
    Ok(false)
}

/// Switch the active view/mode, resetting selection and triggering a refresh
fn switch_view(app: &mut App, new_mode: AppMode) {
    if new_mode == app.mode {
        return;
    }
    app.mode = new_mode;
    app.selected = 0;
    app.selected_packages.clear();
    app.detail = None;
    app.detail_loading = false;
    // Clear local filter — it belongs to the previous view
    app.local_filter.clear();
    // Invalidate any in-flight detail requests from the previous view
    app.detail_generation += 1;
    app.focus = FocusZone::PackageList;
    app.loading = true;
    app.set_status("Loading...");
    app.refresh_view();
}

fn load_detail_for_selected(app: &mut App) {
    let pkg = app.selected_package();
    if let Some(pkg) = pkg {
        if pkg.is_truncated() {
            return;
        }
        let id = pkg.id.clone();
        app.detail_scroll = 0;
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

            // Click on search bar (only visible on search page)
            if app.layout.search_bar.width > 0 && in_rect(col, row, app.layout.search_bar) {
                app.input_mode = InputMode::Search;
                return Ok(false);
            }

            // Click on package list
            if in_rect(col, row, app.layout.package_list) {
                app.focus = FocusZone::PackageList;
                let list = app.layout.package_list;
                let scrollbar_col = list.x + list.width - 1;
                if col >= scrollbar_col.saturating_sub(1) && !app.filtered_packages.is_empty() {
                    scrollbar_jump(app, row);
                    return Ok(false);
                }

                select_package_at_row(app, row);
                return Ok(false);
            }

            // Click on detail panel
            if in_rect(col, row, app.layout.detail_panel) {
                app.focus = FocusZone::DetailPanel;
                return Ok(false);
            }
        }

        // Scroll wheel in package list or detail panel
        MouseEventKind::ScrollUp => {
            if in_rect(col, row, app.layout.package_list) {
                app.move_selection(-3);
                load_detail_for_selected(app);
            } else if in_rect(col, row, app.layout.detail_panel) {
                app.scroll_detail(-3);
            }
        }
        MouseEventKind::ScrollDown => {
            if in_rect(col, row, app.layout.package_list) {
                app.move_selection(3);
                load_detail_for_selected(app);
            } else if in_rect(col, row, app.layout.detail_panel) {
                app.scroll_detail(3);
            }
        }

        // Right-click on a package
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

fn in_rect(col: u16, row: u16, rect: ratatui::layout::Rect) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn open_url(url: &str) -> bool {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    {
        #[cfg(not(test))]
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer").arg(url).spawn();
        }
        #[cfg(not(test))]
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(url).spawn();
        }
        #[cfg(not(test))]
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(url).spawn();
        }
    }
    true
}

/// Determine which tab was clicked based on x position
fn handle_tab_click(app: &mut App, col: u16) {
    for &(start_x, end_x, mode) in &app.layout.tab_regions {
        if col >= start_x && col < end_x {
            switch_view(app, mode);
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
    use ratatui::layout::Rect;

    use super::*;
    use crate::app::{App, AppMode, ConfirmDialog, InputMode};
    use crate::backend::WingetBackend;
    use crate::models::{Operation, Package, PackageDetail, Source};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    struct NoopBackend;

    #[async_trait]
    impl WingetBackend for NoopBackend {
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

    fn make_app() -> App {
        App::new(Arc::new(NoopBackend), crate::config::Config::default())
    }

    fn make_app_with_pkg(id: &str, version: &str, available: &str) -> App {
        let mut app = make_app();
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

    // ── in_rect ──────────────────────────────────────────────────────────────

    #[test]
    fn in_rect_point_inside() {
        assert!(in_rect(5, 5, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_top_left_corner_is_inside() {
        assert!(in_rect(3, 3, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_just_outside_right_edge() {
        // x=3, width=10 → columns 3..13; column 13 is outside
        assert!(!in_rect(13, 5, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_just_outside_bottom_edge() {
        // y=3, height=10 → rows 3..13; row 13 is outside
        assert!(!in_rect(5, 13, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_last_column_inside() {
        // rightmost valid column is 3+10-1=12
        assert!(in_rect(12, 5, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_last_row_inside() {
        assert!(in_rect(5, 12, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_point_left_of_rect() {
        assert!(!in_rect(2, 5, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_point_above_rect() {
        assert!(!in_rect(5, 2, rect(3, 3, 10, 10)));
    }

    #[test]
    fn in_rect_zero_size_rect() {
        assert!(!in_rect(0, 0, rect(0, 0, 0, 0)));
    }

    // ── open_url ─────────────────────────────────────────────────────────────

    #[test]
    fn open_url_accepts_https() {
        assert!(open_url("https://example.com"));
    }

    #[test]
    fn open_url_accepts_http() {
        assert!(open_url("http://example.com"));
    }

    #[test]
    fn open_url_rejects_empty_string() {
        assert!(!open_url(""));
    }

    #[test]
    fn open_url_rejects_ftp() {
        assert!(!open_url("ftp://example.com"));
    }

    #[test]
    fn open_url_rejects_javascript_scheme() {
        assert!(!open_url("javascript:alert(1)"));
    }

    #[test]
    fn open_url_rejects_file_scheme() {
        assert!(!open_url("file:///etc/passwd"));
    }

    #[test]
    fn open_url_rejects_partial_http_prefix() {
        // Must not match "http" without the colon-slash-slash
        assert!(!open_url("httpx://example.com"));
    }

    // ── handle_confirm ───────────────────────────────────────────────────────

    #[test]
    fn handle_confirm_n_cancels_dialog() {
        let mut app = make_app();
        app.confirm = Some(ConfirmDialog {
            message: "Upgrade Foo?".into(),
            operation: Operation::Upgrade { id: "Foo".into() },
        });
        let _ = handle_confirm(&mut app, KeyCode::Char('n'));
        assert!(app.confirm.is_none(), "confirm should be cleared on 'n'");
        assert_eq!(app.status_message, "Cancelled");
    }

    #[test]
    fn handle_confirm_esc_cancels_dialog() {
        let mut app = make_app();
        app.confirm = Some(ConfirmDialog {
            message: "Upgrade Foo?".into(),
            operation: Operation::Upgrade { id: "Foo".into() },
        });
        let _ = handle_confirm(&mut app, KeyCode::Esc);
        assert!(app.confirm.is_none());
        assert_eq!(app.status_message, "Cancelled");
    }

    #[test]
    fn handle_confirm_other_key_leaves_dialog() {
        let mut app = make_app();
        app.confirm = Some(ConfirmDialog {
            message: "Upgrade Foo?".into(),
            operation: Operation::Upgrade { id: "Foo".into() },
        });
        let _ = handle_confirm(&mut app, KeyCode::Char('x'));
        assert!(
            app.confirm.is_some(),
            "unrecognised key must not clear the dialog"
        );
    }

    // ── handle_search_input ──────────────────────────────────────────────────

    #[test]
    fn search_input_esc_returns_to_normal_mode() {
        let mut app = make_app();
        app.input_mode = InputMode::Search;
        let _ = handle_search_input(&mut app, KeyCode::Esc);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn search_input_char_appends_to_query() {
        let mut app = make_app();
        app.mode = AppMode::Search;
        app.input_mode = InputMode::Search;
        let _ = handle_search_input(&mut app, KeyCode::Char('a'));
        let _ = handle_search_input(&mut app, KeyCode::Char('b'));
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn search_input_backspace_removes_last_char() {
        let mut app = make_app();
        app.mode = AppMode::Search;
        app.input_mode = InputMode::Search;
        app.search_query = "abc".into();
        let _ = handle_search_input(&mut app, KeyCode::Backspace);
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn search_input_enter_with_empty_query_stays_in_search_mode() {
        let mut app = make_app();
        app.mode = AppMode::Search;
        app.input_mode = InputMode::Search;
        app.search_query = String::new();
        let _ = handle_search_input(&mut app, KeyCode::Enter);
        // Empty query: input_mode switches to Normal but no search is triggered
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search_query.is_empty());
    }

    // ── local filter (Installed / Upgrades) ──────────────────────────────────

    #[test]
    fn local_filter_char_appends_to_local_filter_not_search_query() {
        let mut app = make_app();
        // Default mode is Installed, so typing should go to local_filter
        app.input_mode = InputMode::Search;
        let _ = handle_search_input(&mut app, KeyCode::Char('v'));
        let _ = handle_search_input(&mut app, KeyCode::Char('s'));
        assert_eq!(
            app.local_filter, "vs",
            "local_filter should accumulate the typed chars"
        );
        assert_eq!(
            app.search_query, "",
            "search_query must not be modified in Installed mode"
        );
    }

    #[test]
    fn local_filter_backspace_removes_from_local_filter() {
        let mut app = make_app();
        app.input_mode = InputMode::Search;
        app.local_filter = "vsc".into();
        let _ = handle_search_input(&mut app, KeyCode::Backspace);
        assert_eq!(app.local_filter, "vs");
    }

    #[test]
    fn local_filter_esc_clears_filter_and_restores_packages() {
        let mut app = make_app();
        app.packages = vec![
            Package {
                id: "A.VSCode".into(),
                name: "VS Code".into(),
                version: "1.0".into(),
                source: "winget".into(),
                available_version: String::new(),
            },
            Package {
                id: "B.Notepad".into(),
                name: "Notepad++".into(),
                version: "1.0".into(),
                source: "winget".into(),
                available_version: String::new(),
            },
        ];
        app.filtered_packages = app.packages.clone();
        app.local_filter = "vscode".into();
        app.apply_filter();
        assert_eq!(
            app.filtered_packages.len(),
            1,
            "filter should narrow the list"
        );

        app.input_mode = InputMode::Search;
        let _ = handle_search_input(&mut app, KeyCode::Esc);
        assert_eq!(app.local_filter, "", "filter should be cleared on Esc");
        assert_eq!(
            app.filtered_packages.len(),
            2,
            "full list should be restored"
        );
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn slash_key_in_installed_mode_does_not_switch_to_search_mode() {
        let mut app = make_app();
        // Default mode is Installed
        assert_eq!(app.mode, AppMode::Installed);
        let _ = handle_normal_mode(&mut app, KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(
            app.mode,
            AppMode::Installed,
            "mode must not change to Search"
        );
        assert_eq!(
            app.input_mode,
            InputMode::Search,
            "input_mode should switch to Search for the filter bar"
        );
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
                assert_eq!(
                    version,
                    Some("2.0".to_string()),
                    "version should be trimmed"
                );
            }
            _ => panic!("expected Install operation"),
        }
    }
}
