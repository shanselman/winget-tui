use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

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
        KeyCode::Tab => {
            app.mode = app.mode.cycle();
            app.selected = 0;
            app.detail = None;
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
                let id = pkg.id.clone();
                app.confirm = Some(ConfirmDialog {
                    message: format!("Install {}?", id),
                    operation: Operation::Install {
                        id,
                        version: None,
                    },
                });
            }
        }

        // Uninstall
        KeyCode::Char('x') => {
            if let Some(pkg) = app.selected_package() {
                let id = pkg.id.clone();
                app.confirm = Some(ConfirmDialog {
                    message: format!("Uninstall {}?", id),
                    operation: Operation::Uninstall { id },
                });
            }
        }

        // Upgrade
        KeyCode::Char('u') => {
            if let Some(pkg) = app.selected_package() {
                let id = pkg.id.clone();
                app.confirm = Some(ConfirmDialog {
                    message: format!("Upgrade {}?", id),
                    operation: Operation::Upgrade { id },
                });
            }
        }

        // Enter - load detail
        KeyCode::Enter => {
            load_detail_for_selected(app);
        }

        _ => {}
    }
    Ok(false)
}

fn load_detail_for_selected(app: &mut App) {
    let id = app.selected_package().map(|p| p.id.clone());
    if let Some(id) = id {
        app.load_detail(&id);
    }
}

fn handle_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
) -> anyhow::Result<bool> {
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

                let content_y = app.layout.list_content_y;
                if row >= content_y {
                    let clicked_idx = (row - content_y) as usize;
                    if clicked_idx < app.filtered_packages.len() {
                        app.selected = clicked_idx;
                        load_detail_for_selected(app);
                    }
                }
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

        // Double-click to show detail
        MouseEventKind::Down(MouseButton::Right) => {
            // Right-click on a package shows context (loads detail)
            if in_rect(col, row, app.layout.package_list) {
                let content_y = app.layout.list_content_y;
                if row >= content_y {
                    let clicked_idx = (row - content_y) as usize;
                    if clicked_idx < app.filtered_packages.len() {
                        app.selected = clicked_idx;
                        load_detail_for_selected(app);
                    }
                }
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
    col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

/// Determine which tab was clicked based on x position
fn handle_tab_click(app: &mut App, col: u16) {
    // Tab bar layout: " winget-tui   Search  Installed  Upgrades "
    // The title " winget-tui " is ~13 chars, then 2 spaces, then tabs ~9 chars each with spacing
    let tab_start = 15u16;
    let tab_width = 11u16;

    let tabs = [AppMode::Search, AppMode::Installed, AppMode::Upgrades];
    if col >= tab_start {
        let tab_idx = ((col - tab_start) / tab_width) as usize;
        if let Some(&mode) = tabs.get(tab_idx) {
            if mode != app.mode {
                app.mode = mode;
                app.selected = 0;
                app.detail = None;
                app.loading = true;
                app.set_status("Loading...");
                app.refresh_view();
            }
        }
    }
}
