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

        // Search — switch to search view and enter input mode
        KeyCode::Char('/') | KeyCode::Char('s') => {
            if app.mode != AppMode::Search {
                switch_view(app, AppMode::Search);
            }
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
            switch_view(app, mode);
            break;
        }
    }
}
