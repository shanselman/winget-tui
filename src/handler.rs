use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};

use crate::app::{App, AppMode, ConfirmDialog, FocusZone, InputMode};
use crate::models::{Operation, SortDir, SortField};

/// Handle the next crossterm event, waiting up to 50 ms for one to arrive.
///
/// Returns `true` when an event was read (meaning app state may have changed
/// and the UI should be redrawn), `false` when the poll timed out with no
/// event (no redraw needed).
pub fn handle_events(app: &mut App) -> anyhow::Result<bool> {
    if !event::poll(std::time::Duration::from_millis(50))? {
        return Ok(false);
    }

    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            // Confirm dialog takes priority
            if app.confirm.is_some() {
                handle_confirm(app, key.code)?;
                return Ok(true);
            }

            // Version input prompt takes priority after confirm
            if app.input_mode == InputMode::VersionInput {
                handle_version_input(app, key.code)?;
                return Ok(true);
            }

            // Help overlay
            if app.show_help {
                handle_help_input(app, key.code);
                return Ok(true);
            }

            match app.input_mode {
                InputMode::Search => handle_search_input(app, key.code)?,
                InputMode::LocalFilter => handle_local_filter_input(app, key.code)?,
                InputMode::Normal => handle_normal_mode(app, key.code, key.modifiers)?,
                InputMode::VersionInput => unreachable!("handled above"),
            };
        }
        Event::Mouse(mouse) => {
            handle_mouse(app, mouse)?;
        }
        _ => {}
    }
    Ok(true)
}

fn handle_help_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('?') | KeyCode::Esc => {
            app.show_help = false;
            app.help_scroll = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.help_scroll = app.help_scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.help_scroll = app.help_scroll.saturating_add(1).min(app.help_max_scroll);
        }
        KeyCode::PageUp => {
            app.help_scroll = app.help_scroll.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.help_scroll = app.help_scroll.saturating_add(10).min(app.help_max_scroll);
        }
        KeyCode::Home => {
            app.help_scroll = 0;
        }
        KeyCode::End => {
            app.help_scroll = app.help_max_scroll;
        }
        _ => {}
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

fn handle_local_filter_input(app: &mut App, key: KeyCode) -> anyhow::Result<bool> {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.local_filter.clear();
            app.apply_filter();
            app.ensure_selection_visible();
            load_detail_for_selected(app);
            app.set_status("Filter cleared");
        }
        KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            app.apply_filter();
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        KeyCode::Backspace => {
            app.local_filter.pop();
            app.apply_filter();
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        // Allow navigating the filtered list without leaving filter mode
        KeyCode::Up => {
            app.move_selection(-1);
            load_detail_for_selected(app);
        }
        KeyCode::Down => {
            app.move_selection(1);
            load_detail_for_selected(app);
        }
        KeyCode::PageUp if !app.filtered_packages.is_empty() => {
            let page = list_page_size(app);
            app.selected = app.selected.saturating_sub(page);
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        KeyCode::PageDown if !app.filtered_packages.is_empty() => {
            let page = list_page_size(app);
            let max = app.filtered_packages.len() - 1;
            app.selected = (app.selected + page).min(max);
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        KeyCode::Home if !app.filtered_packages.is_empty() => {
            app.selected = 0;
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        KeyCode::End if !app.filtered_packages.is_empty() => {
            app.selected = app.filtered_packages.len() - 1;
            app.ensure_selection_visible();
            load_detail_for_selected(app);
        }
        KeyCode::Char(c) => {
            app.local_filter.push(c);
            app.apply_filter();
            app.ensure_selection_visible();
            load_detail_for_selected(app);
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
            } else if !app.filtered_packages.is_empty() {
                let page = list_page_size(app);
                app.selected = app.selected.saturating_sub(page);
                app.ensure_selection_visible();
                load_detail_for_selected(app);
            }
        }
        KeyCode::PageDown => {
            if app.focus == FocusZone::DetailPanel {
                let page = app.layout.detail_panel.height.saturating_sub(3) as isize;
                app.scroll_detail(page);
            } else if !app.filtered_packages.is_empty() {
                let page = list_page_size(app);
                let max = app.filtered_packages.len() - 1;
                app.selected = (app.selected + page).min(max);
                app.ensure_selection_visible();
                load_detail_for_selected(app);
            }
        }
        KeyCode::Home => {
            if app.focus == FocusZone::DetailPanel {
                app.detail_scroll = 0;
            } else if !app.filtered_packages.is_empty() {
                app.selected = 0;
                app.ensure_selection_visible();
                load_detail_for_selected(app);
            }
        }
        KeyCode::End => {
            if app.focus == FocusZone::DetailPanel {
                let viewport = app.layout.detail_panel.height.saturating_sub(3) as usize;
                app.detail_scroll = app.detail_content_lines.saturating_sub(viewport);
            } else if !app.filtered_packages.is_empty() {
                app.selected = app.filtered_packages.len() - 1;
                app.ensure_selection_visible();
                load_detail_for_selected(app);
            }
        }

        // Enter: load detail for selected package
        KeyCode::Enter => {
            load_detail_for_selected(app);
        }

        // Search in Search view, local filter in Installed/Upgrades
        KeyCode::Char('/') | KeyCode::Char('s') => {
            if app.mode == AppMode::Search {
                app.input_mode = InputMode::Search;
            } else {
                app.input_mode = InputMode::LocalFilter;
            }
        }

        // Filter
        KeyCode::Char('f') => {
            app.source_filter = app.source_filter.cycle();
            app.selected = 0;
            app.loading = true;
            app.set_status(format!("Filter: {} -- loading...", app.source_filter));
            app.refresh_view();
        }

        // Pin filter
        KeyCode::Char('P') => {
            if app.mode == AppMode::Search {
                app.set_status("Pinned filter is available in Installed and Upgrades");
            } else {
                app.cycle_pin_filter();
                if let Some(pkg) = app.selected_package() {
                    let id = pkg.id.clone();
                    app.load_detail(&id);
                }
            }
        }

        // Refresh
        KeyCode::Char('r') => {
            app.loading = true;
            app.set_status("Refreshing...");
            app.refresh_view();
        }

        // Export current visible list to CSV
        KeyCode::Char('e') => match app.export_list_csv() {
            Ok(path) => app.set_status(format!(
                "Exported {} package{} to {path}",
                app.filtered_packages.len(),
                if app.filtered_packages.len() == 1 {
                    ""
                } else {
                    "s"
                }
            )),
            Err(msg) => app.set_status(msg),
        },

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

        // Pin / unpin the selected installed package
        KeyCode::Char('p') => {
            if app.mode == AppMode::Search {
                app.set_status("Pinning applies to installed packages, not search results");
            } else if let Some(pkg) = app.selected_package() {
                if pkg.is_truncated() {
                    app.set_status(
                        "Cannot pin: package ID was truncated by winget — use winget directly",
                    );
                } else {
                    let id = pkg.id.clone();
                    let (message, operation) = if pkg.pin_state.is_pinned() {
                        (format!("Remove pin for {}?", id), Operation::Unpin { id })
                    } else {
                        (
                            format!("Pin {} and block upgrades until unpinned?", id),
                            Operation::Pin { id },
                        )
                    };
                    app.confirm = Some(ConfirmDialog { message, operation });
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
                    let name = pkg.name.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Upgrade {}? (ID truncated in winget output)", name),
                        operation: Operation::Upgrade { id: name },
                    });
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
        KeyCode::Char('U')
            if app.mode == AppMode::Upgrades && !app.selected_packages.is_empty() =>
        {
            let selected_count = app.selected_packages.len();
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
                let skipped = selected_count.saturating_sub(count);
                let skipped_note = if skipped > 0 {
                    format!(
                        " ({} skipped — truncated ID{})",
                        skipped,
                        if skipped == 1 { "" } else { "s" }
                    )
                } else {
                    String::new()
                };
                app.confirm = Some(ConfirmDialog {
                    message: format!(
                        "Upgrade {} selected package{}{}?",
                        count,
                        if count == 1 { "" } else { "s" },
                        skipped_note
                    ),
                    operation: Operation::BatchUpgrade { ids },
                });
            }
        }

        // Toggle selection (Space)
        KeyCode::Char(' ')
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() =>
        {
            let idx = app.selected;
            if app.selected_packages.contains(&idx) {
                app.selected_packages.remove(&idx);
            } else {
                app.selected_packages.insert(idx);
            }
            app.move_selection(1);
            load_detail_for_selected(app);
        }

        // Select all / deselect all
        KeyCode::Char('a')
            if app.mode == AppMode::Upgrades && !app.filtered_packages.is_empty() =>
        {
            if app.selected_packages.len() == app.filtered_packages.len() {
                app.selected_packages.clear();
            } else {
                app.selected_packages = (0..app.filtered_packages.len()).collect();
            }
        }

        // Open homepage
        KeyCode::Char('o') => open_detail_url(
            app,
            |d| &d.homepage,
            "No homepage URL available for this package",
            "Opening ",
        ),

        // Open release notes / changelog in default browser
        KeyCode::Char('c') => open_detail_url(
            app,
            |d| &d.release_notes_url,
            "No changelog URL available for this package",
            "Opening changelog ",
        ),

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
    app.input_mode = InputMode::Normal;
    app.local_filter.clear();
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

/// Returns the number of visible data rows in the package list, based on the
/// last-rendered layout.  Falls back to 20 if a render hasn't happened yet.
fn list_page_size(app: &App) -> usize {
    let h = app.package_list_viewport_rows();
    if h == 0 {
        20
    } else {
        h
    }
}

fn load_detail_for_selected(app: &mut App) {
    let pkg = app.selected_package();
    if let Some(pkg) = pkg {
        let id = pkg.id.clone();
        app.detail_scroll = 0;
        app.load_detail(&id);
    }
}

/// Handle a click on the package-list header row.
///
/// Column widths are defined as percentages in `ui.rs`.  We approximate the
/// boundary by splitting the usable content area (total width minus two border
/// columns and one scrollbar column) into the same proportions.
///
/// Clicking a sortable column:
/// - If it is already the active sort field, toggles Asc ↔ Desc.
/// - Otherwise, activates that field in Asc order.
///
/// Clicking an unsortable column (Source; or Available in non-Upgrades views)
/// is a no-op.
fn click_sort_header(app: &mut App, col: u16) {
    let list = app.layout.package_list;
    // Usable content width: strip left border (1), right border/scrollbar (2)
    let content_width = list.width.saturating_sub(3) as u32;
    if content_width == 0 {
        return;
    }
    let x0 = (list.x + 1) as u32; // first content column
    let col = col as u32;
    if col < x0 || col >= x0 + content_width {
        return;
    }
    let offset = col - x0;

    // Determine the sort field based on column percentages defined in ui.rs.
    // Non-Upgrades: Name 25%, ID 35%, Version 20%, Source 20% (unsortable)
    // Upgrades:     Name 25%, ID 30%, Version 15%, Available 15% (sortable), Source 15% (unsortable)
    let field = if app.mode == AppMode::Upgrades {
        let boundary_name = content_width * 25 / 100;
        let boundary_id = boundary_name + content_width * 30 / 100;
        let boundary_version = boundary_id + content_width * 15 / 100;
        let boundary_available = boundary_version + content_width * 15 / 100;
        if offset < boundary_name {
            SortField::Name
        } else if offset < boundary_id {
            SortField::Id
        } else if offset < boundary_version {
            SortField::Version
        } else if offset < boundary_available {
            SortField::AvailableVersion
        } else {
            return; // Source — not sortable
        }
    } else {
        let boundary_name = content_width * 25 / 100;
        let boundary_id = boundary_name + content_width * 35 / 100;
        let boundary_version = boundary_id + content_width * 20 / 100;
        if offset < boundary_name {
            SortField::Name
        } else if offset < boundary_id {
            SortField::Id
        } else if offset < boundary_version {
            SortField::Version
        } else {
            return; // Source — not sortable
        }
    };

    if app.sort_field == field {
        // Same column: toggle direction
        app.sort_dir = if app.sort_dir == SortDir::Asc {
            SortDir::Desc
        } else {
            SortDir::Asc
        };
    } else {
        app.sort_field = field;
        app.sort_dir = SortDir::Asc;
    }
    app.apply_filter();
    let label = format!("Sort: {}{}", app.sort_field, app.sort_dir.indicator());
    app.set_status(&label);
}

/// Select the package row at the given terminal row coordinate and load its detail.
fn select_package_at_row(app: &mut App, row: u16) {
    let list = app.layout.package_list;
    let content_y = app.layout.list_content_y;
    let content_end_y = list.y + list.height.saturating_sub(1);
    if row >= content_y && row < content_end_y {
        let clicked_idx = (row - content_y) as usize + app.table_state.offset();
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

            // Click on search/filter bar
            if app.layout.search_bar.width > 0 && in_rect(col, row, app.layout.search_bar) {
                app.input_mode = if app.mode == AppMode::Search {
                    InputMode::Search
                } else {
                    InputMode::LocalFilter
                };
                return Ok(false);
            }

            // Click on package list
            if in_rect(col, row, app.layout.package_list) {
                app.focus = FocusZone::PackageList;
                let list = app.layout.package_list;
                let scrollbar_col = list.x + list.width - 1;
                if col == scrollbar_col && !app.filtered_packages.is_empty() {
                    scrollbar_jump(app, row);
                    return Ok(false);
                }

                // Click on header row → sort by that column
                let header_row = app.layout.list_content_y.saturating_sub(1);
                if row == header_row && app.layout.list_content_y > 0 {
                    click_sort_header(app, col);
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
                let offset = app.table_state.offset_mut();
                *offset = offset.saturating_sub(3);
            } else if in_rect(col, row, app.layout.detail_panel) {
                app.scroll_detail(-3);
            }
        }
        MouseEventKind::ScrollDown => {
            if in_rect(col, row, app.layout.package_list) {
                let max = app.filtered_packages.len().saturating_sub(1);
                let offset = app.table_state.offset_mut();
                *offset = (*offset + 3).min(max);
            } else if in_rect(col, row, app.layout.detail_panel) {
                app.scroll_detail(3);
            }
        }

        // Right-click on a package
        MouseEventKind::Down(MouseButton::Right) if in_rect(col, row, app.layout.package_list) => {
            select_package_at_row(app, row);
        }

        // Drag on scrollbar track
        MouseEventKind::Drag(MouseButton::Left)
            if in_rect(col, row, app.layout.package_list) && !app.filtered_packages.is_empty() =>
        {
            let list = app.layout.package_list;
            let scrollbar_col = list.x + list.width - 1;
            if col == scrollbar_col {
                scrollbar_jump(app, row);
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
        app.ensure_selection_visible();
        load_detail_for_selected(app);
    }
}

fn in_rect(col: u16, row: u16, rect: ratatui::layout::Rect) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Open a URL from the current package detail pane, updating the status bar.
///
/// `get_url`       – extracts the URL field from a `PackageDetail`  
/// `not_available` – status message shown when the URL field is empty  
/// `opening_prefix`– prefix prepended to the URL in the "Opening …" message
fn open_detail_url(
    app: &mut App,
    get_url: impl Fn(&crate::models::PackageDetail) -> &str,
    not_available: &'static str,
    opening_prefix: &'static str,
) {
    // Extract URL (or emit an early-exit status) while the detail is borrowed,
    // then release the borrow before calling set_status (which needs &mut App).
    let url = match &app.detail {
        None => {
            app.set_status("No package selected");
            return;
        }
        Some(detail) => {
            let u = get_url(detail);
            if u.is_empty() {
                app.set_status(not_available);
                return;
            }
            u.to_string()
        }
    };
    if open_url(&url) {
        app.set_status(format!("{opening_prefix}{}…", url));
    } else {
        app.set_status("Blocked: URL must start with http:// or https://");
    }
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
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::layout::Rect;

    use super::*;
    use crate::app::{App, ConfirmDialog, InputMode};
    use crate::backend::WingetBackend;
    use crate::models::{Operation, Package, PackageDetail, PackagePin, PinState, Source};

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
            pin_state: PinState::None,
        }];
        app.filtered_packages = app.packages.clone();
        app.selected = 0;
        app
    }

    fn make_app_with_pkgs(count: usize) -> App {
        let mut app = make_app();
        app.packages = (0..count)
            .map(|i| Package {
                id: format!("pkg{i}"),
                name: format!("Package {i}"),
                version: "1.0.0".to_string(),
                source: "winget".to_string(),
                available_version: String::new(),
                pin_state: PinState::None,
            })
            .collect();
        app.filtered_packages = app.packages.clone();
        app.selected = 0;
        app
    }

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
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
        app.input_mode = InputMode::Search;
        let _ = handle_search_input(&mut app, KeyCode::Char('a'));
        let _ = handle_search_input(&mut app, KeyCode::Char('b'));
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn search_input_backspace_removes_last_char() {
        let mut app = make_app();
        app.input_mode = InputMode::Search;
        app.search_query = "abc".into();
        let _ = handle_search_input(&mut app, KeyCode::Backspace);
        assert_eq!(app.search_query, "ab");
    }

    #[test]
    fn search_input_enter_with_empty_query_stays_in_search_mode() {
        let mut app = make_app();
        app.input_mode = InputMode::Search;
        app.search_query = String::new();
        let _ = handle_search_input(&mut app, KeyCode::Enter);
        // Empty query: input_mode switches to Normal but no search is triggered
        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.search_query.is_empty());
    }

    #[test]
    fn slash_key_in_installed_view_enters_local_filter_mode() {
        let mut app = make_app();
        app.mode = AppMode::Installed;

        let _ = handle_normal_mode(&mut app, KeyCode::Char('/'), KeyModifiers::NONE);

        assert_eq!(app.input_mode, InputMode::LocalFilter);
    }

    #[test]
    fn slash_key_in_search_view_enters_search_mode() {
        let mut app = make_app();
        app.mode = AppMode::Search;

        let _ = handle_normal_mode(&mut app, KeyCode::Char('/'), KeyModifiers::NONE);

        assert_eq!(app.input_mode, InputMode::Search);
    }

    #[test]
    fn local_filter_char_input_updates_filter_and_narrows_list() {
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Installed;
        app.filtered_packages = vec![
            Package {
                id: "Google.Chrome".to_string(),
                name: "Google Chrome".to_string(),
                version: "1.0".to_string(),
                source: "winget".to_string(),
                available_version: String::new(),
                pin_state: PinState::None,
            },
            Package {
                id: "Mozilla.Firefox".to_string(),
                name: "Mozilla Firefox".to_string(),
                version: "1.0".to_string(),
                source: "winget".to_string(),
                available_version: String::new(),
                pin_state: PinState::None,
            },
        ];
        app.packages = app.filtered_packages.clone();
        app.input_mode = InputMode::LocalFilter;
        let rt = test_runtime();
        let _guard = rt.enter();

        let _ = handle_local_filter_input(&mut app, KeyCode::Char('c'));
        let _ = handle_local_filter_input(&mut app, KeyCode::Char('h'));

        assert_eq!(app.local_filter, "ch");
        assert_eq!(app.filtered_packages.len(), 1);
        assert_eq!(app.filtered_packages[0].id, "Google.Chrome");
    }

    #[test]
    fn local_filter_esc_clears_filter_and_restores_list() {
        let mut app = make_app_with_pkgs(2);
        app.mode = AppMode::Installed;
        app.local_filter = "pkg1".to_string();
        app.input_mode = InputMode::LocalFilter;
        app.apply_filter();
        let rt = test_runtime();
        let _guard = rt.enter();

        let _ = handle_local_filter_input(&mut app, KeyCode::Esc);

        assert_eq!(app.input_mode, InputMode::Normal);
        assert!(app.local_filter.is_empty());
        assert_eq!(app.filtered_packages.len(), 2);
    }

    #[test]
    fn local_filter_enter_keeps_filter_but_exits_input_mode() {
        let mut app = make_app_with_pkgs(2);
        app.mode = AppMode::Installed;
        app.local_filter = "pkg1".to_string();
        app.input_mode = InputMode::LocalFilter;
        app.apply_filter();
        let rt = test_runtime();
        let _guard = rt.enter();

        let _ = handle_local_filter_input(&mut app, KeyCode::Enter);

        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.local_filter, "pkg1");
        assert_eq!(app.filtered_packages.len(), 1);
    }

    #[test]
    fn local_filter_up_down_navigate_without_leaving_filter_mode() {
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Installed;
        app.input_mode = InputMode::LocalFilter;
        let rt = test_runtime();
        let _guard = rt.enter();

        // Initial selection is 0; Down should advance it
        app.selected = 0;
        let _ = handle_local_filter_input(&mut app, KeyCode::Down);
        assert_eq!(app.selected, 1, "Down should move selection to index 1");
        assert_eq!(
            app.input_mode,
            InputMode::LocalFilter,
            "input_mode must stay LocalFilter"
        );

        // Up should move it back
        let _ = handle_local_filter_input(&mut app, KeyCode::Up);
        assert_eq!(app.selected, 0, "Up should move selection back to index 0");
        assert_eq!(app.input_mode, InputMode::LocalFilter);
    }

    #[test]
    fn local_filter_home_end_navigate_to_bounds() {
        let mut app = make_app_with_pkgs(5);
        app.mode = AppMode::Installed;
        app.input_mode = InputMode::LocalFilter;
        let rt = test_runtime();
        let _guard = rt.enter();

        app.selected = 2;
        let _ = handle_local_filter_input(&mut app, KeyCode::Home);
        assert_eq!(app.selected, 0, "Home should jump to first item");
        assert_eq!(app.input_mode, InputMode::LocalFilter);

        let _ = handle_local_filter_input(&mut app, KeyCode::End);
        assert_eq!(app.selected, 4, "End should jump to last item");
        assert_eq!(app.input_mode, InputMode::LocalFilter);
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

    #[test]
    fn batch_upgrade_confirm_uses_clean_count_when_all_ids_are_valid() {
        let mut app = make_app_with_pkgs(2);
        app.mode = AppMode::Upgrades;
        app.selected_packages = [0usize, 1usize].into_iter().collect();

        let _ = handle_normal_mode(&mut app, KeyCode::Char('U'), KeyModifiers::NONE);

        let confirm = app.confirm.expect("confirm dialog should be set");
        assert_eq!(confirm.message, "Upgrade 2 selected packages?");
        match confirm.operation {
            Operation::BatchUpgrade { ids } => assert_eq!(ids.len(), 2),
            _ => panic!("expected BatchUpgrade operation"),
        }
    }

    #[test]
    fn single_upgrade_with_truncated_id_uses_name_query() {
        let mut app = make_app_with_pkg("Microsoft.Azure.Function...", "4.0", "4.1");
        app.mode = AppMode::Upgrades;

        let _ = handle_normal_mode(&mut app, KeyCode::Char('u'), KeyModifiers::NONE);

        let confirm = app.confirm.expect("confirm dialog should be set");
        assert!(
            confirm.message.contains("ID truncated"),
            "confirm text should explain name-based fallback"
        );
        match confirm.operation {
            Operation::Upgrade { id } => assert_eq!(id, "Test Package"),
            _ => panic!("expected Upgrade operation"),
        }
    }

    #[test]
    fn batch_upgrade_confirm_reports_truncated_skips() {
        let mut app = make_app();
        app.mode = AppMode::Upgrades;
        app.packages = vec![
            Package {
                id: "Good.App".to_string(),
                name: "Good".to_string(),
                version: "1.0".to_string(),
                source: "winget".to_string(),
                available_version: "1.1".to_string(),
                pin_state: PinState::None,
            },
            Package {
                id: "Bad.App...".to_string(),
                name: "Bad".to_string(),
                version: "1.0".to_string(),
                source: "winget".to_string(),
                available_version: "1.1".to_string(),
                pin_state: PinState::None,
            },
        ];
        app.filtered_packages = app.packages.clone();
        app.selected_packages = [0usize, 1usize].into_iter().collect();

        let _ = handle_normal_mode(&mut app, KeyCode::Char('U'), KeyModifiers::NONE);

        let confirm = app.confirm.expect("confirm dialog should be set");
        assert!(confirm.message.contains("Upgrade 1 selected package"));
        assert!(confirm.message.contains("1 skipped"));
        match confirm.operation {
            Operation::BatchUpgrade { ids } => assert_eq!(ids, vec!["Good.App".to_string()]),
            _ => panic!("expected BatchUpgrade operation"),
        }
    }

    #[test]
    fn batch_upgrade_all_truncated_shows_status_and_skips_confirm() {
        let mut app = make_app();
        app.mode = AppMode::Upgrades;
        app.packages = vec![Package {
            id: "Only.Bad...".to_string(),
            name: "Bad".to_string(),
            version: "1.0".to_string(),
            source: "winget".to_string(),
            available_version: "1.1".to_string(),
            pin_state: PinState::None,
        }];
        app.filtered_packages = app.packages.clone();
        app.selected_packages = [0usize].into_iter().collect();

        let _ = handle_normal_mode(&mut app, KeyCode::Char('U'), KeyModifiers::NONE);

        assert!(app.confirm.is_none(), "no confirm dialog should be shown");
        assert!(app
            .status_message
            .contains("all selected packages have truncated IDs"));
    }

    // ── open homepage / changelog feedback ───────────────────────────────────

    #[test]
    fn open_homepage_no_detail_shows_status() {
        let mut app = make_app();
        // no detail loaded
        let _ = handle_normal_mode(&mut app, KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(app.status_message, "No package selected");
    }

    #[test]
    fn open_homepage_empty_url_shows_status() {
        let mut app = make_app();
        app.detail = Some(PackageDetail {
            homepage: String::new(),
            ..PackageDetail::default()
        });
        let _ = handle_normal_mode(&mut app, KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(
            app.status_message,
            "No homepage URL available for this package"
        );
    }

    #[test]
    fn open_changelog_no_detail_shows_status() {
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(app.status_message, "No package selected");
    }

    #[test]
    fn open_changelog_empty_url_shows_status() {
        let mut app = make_app();
        app.detail = Some(PackageDetail {
            release_notes_url: String::new(),
            ..PackageDetail::default()
        });
        let _ = handle_normal_mode(&mut app, KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(
            app.status_message,
            "No changelog URL available for this package"
        );
    }

    #[test]
    fn export_empty_list_shows_status() {
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(app.status_message, "Nothing to export: list is empty");
    }

    // ── handle_normal_mode: quit / overlay / focus / sort ────────────────────

    #[test]
    fn q_key_sets_should_quit() {
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.should_quit);
    }

    #[test]
    fn esc_key_sets_should_quit() {
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.should_quit);
    }

    #[test]
    fn question_mark_enables_help_overlay() {
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(app.show_help, "? should enable the help overlay");
    }

    #[test]
    fn question_mark_disables_help_overlay_when_already_shown() {
        let mut app = make_app();
        app.show_help = true;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('?'), KeyModifiers::NONE);
        assert!(!app.show_help, "? should toggle the help overlay off");
    }

    #[test]
    fn tab_moves_focus_to_detail_panel() {
        let mut app = make_app();
        assert_eq!(app.focus, FocusZone::PackageList);
        let _ = handle_normal_mode(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.focus, FocusZone::DetailPanel);
    }

    #[test]
    fn back_tab_moves_focus_back_to_list() {
        let mut app = make_app();
        app.focus = FocusZone::DetailPanel;
        let _ = handle_normal_mode(&mut app, KeyCode::BackTab, KeyModifiers::NONE);
        assert_eq!(app.focus, FocusZone::PackageList);
    }

    #[test]
    fn s_key_cycles_sort_to_name_ascending() {
        use crate::models::{SortDir, SortField};
        let mut app = make_app();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('S'), KeyModifiers::NONE);
        assert_eq!(app.sort_field, SortField::Name);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    // ── handle_normal_mode: pin (p / P) ──────────────────────────────────────

    #[test]
    fn p_in_search_mode_shows_status_not_confirm() {
        let mut app = make_app();
        app.mode = AppMode::Search;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(
            app.status_message.contains("search results"),
            "p in Search mode should show informational status"
        );
        assert!(app.confirm.is_none());
    }

    #[test]
    fn p_on_truncated_id_shows_status() {
        let mut app = make_app_with_pkg("TruncatedPkg...", "1.0", "");
        app.mode = AppMode::Installed;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(app.status_message.contains("truncated"));
        assert!(app.confirm.is_none());
    }

    #[test]
    fn p_on_unpinned_pkg_creates_pin_confirm() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "");
        app.mode = AppMode::Installed;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(
            app.confirm.is_some(),
            "p on unpinned package should open a pin confirm dialog"
        );
        assert!(matches!(
            app.confirm.unwrap().operation,
            Operation::Pin { .. }
        ));
    }

    #[test]
    fn p_on_pinned_pkg_creates_unpin_confirm() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "");
        app.mode = AppMode::Installed;
        app.packages[0].pin_state = PinState::Pinned;
        app.filtered_packages[0].pin_state = PinState::Pinned;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('p'), KeyModifiers::NONE);
        assert!(
            app.confirm.is_some(),
            "p on pinned package should open an unpin confirm dialog"
        );
        assert!(matches!(
            app.confirm.unwrap().operation,
            Operation::Unpin { .. }
        ));
    }

    #[test]
    fn capital_p_in_search_mode_shows_informational_status() {
        let mut app = make_app();
        app.mode = AppMode::Search;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('P'), KeyModifiers::NONE);
        assert!(
            app.status_message.contains("Installed")
                || app.status_message.contains("Upgrades")
                || app.status_message.contains("available"),
            "P in Search mode should explain it only works in Installed/Upgrades"
        );
    }

    #[test]
    fn capital_p_in_installed_mode_cycles_pin_filter() {
        use crate::models::PinFilter;
        // Use an app with no packages so load_detail is not triggered.
        let mut app = make_app();
        app.mode = AppMode::Installed;
        assert_eq!(app.pin_filter, PinFilter::All);
        let _ = handle_normal_mode(&mut app, KeyCode::Char('P'), KeyModifiers::NONE);
        assert_eq!(app.pin_filter, PinFilter::PinnedOnly);
    }

    // ── handle_normal_mode: install (i / I) / uninstall (x) / upgrade (u) ───

    #[test]
    fn i_on_truncated_id_shows_status_not_confirm() {
        let mut app = make_app_with_pkg("Truncated…", "1.0", "");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('i'), KeyModifiers::NONE);
        assert!(app.status_message.contains("truncated"));
        assert!(app.confirm.is_none());
    }

    #[test]
    fn i_on_valid_pkg_creates_install_confirm() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('i'), KeyModifiers::NONE);
        assert!(app.confirm.is_some());
        assert!(matches!(
            app.confirm.unwrap().operation,
            Operation::Install { version: None, .. }
        ));
    }

    #[test]
    fn x_on_truncated_id_shows_status_not_confirm() {
        let mut app = make_app_with_pkg("Truncated...", "1.0", "");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(app.status_message.contains("truncated"));
        assert!(app.confirm.is_none());
    }

    #[test]
    fn x_on_valid_pkg_creates_uninstall_confirm() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(app.confirm.is_some());
        assert!(matches!(
            app.confirm.unwrap().operation,
            Operation::Uninstall { .. }
        ));
    }

    #[test]
    fn u_on_truncated_id_uses_name_fallback_confirm() {
        let mut app = make_app_with_pkg("Truncated...", "1.0", "2.0");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('u'), KeyModifiers::NONE);
        let confirm = app.confirm.expect("confirm dialog should be shown");
        assert!(confirm.message.contains("ID truncated"));
        assert!(matches!(
            confirm.operation,
            Operation::Upgrade { ref id } if id == "Test Package"
        ));
    }

    #[test]
    fn u_on_valid_pkg_creates_upgrade_confirm() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "2.0");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('u'), KeyModifiers::NONE);
        assert!(app.confirm.is_some());
        assert!(matches!(
            app.confirm.unwrap().operation,
            Operation::Upgrade { .. }
        ));
    }

    #[test]
    fn shift_i_on_truncated_id_shows_status() {
        let mut app = make_app_with_pkg("Truncated...", "1.0", "2.0");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('I'), KeyModifiers::NONE);
        assert!(app.status_message.contains("truncated"));
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn shift_i_prefills_available_version_when_present() {
        let mut app = make_app_with_pkg("Valid.Package", "1.0", "2.0");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('I'), KeyModifiers::NONE);
        assert_eq!(app.input_mode, InputMode::VersionInput);
        assert_eq!(app.version_input, "2.0");
    }

    #[test]
    fn shift_i_falls_back_to_current_version_when_no_available() {
        let mut app = make_app_with_pkg("Valid.Package", "3.5", "");
        let _ = handle_normal_mode(&mut app, KeyCode::Char('I'), KeyModifiers::NONE);
        assert_eq!(app.input_mode, InputMode::VersionInput);
        assert_eq!(app.version_input, "3.5");
    }

    // ── handle_normal_mode: multi-select (Space / a) ─────────────────────────

    #[test]
    fn a_key_selects_all_packages_in_upgrades_view() {
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(
            app.selected_packages.len(),
            3,
            "a should select all 3 packages"
        );
    }

    #[test]
    fn a_key_deselects_all_when_all_already_selected() {
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.selected_packages = (0..3).collect();
        let _ = handle_normal_mode(&mut app, KeyCode::Char('a'), KeyModifiers::NONE);
        assert!(
            app.selected_packages.is_empty(),
            "a when all selected should deselect all"
        );
    }

    // ── handle_normal_mode: keys that spawn async tasks (need runtime) ────────

    #[test]
    fn f_key_cycles_source_filter_to_winget() {
        use crate::models::SourceFilter;
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        assert_eq!(app.source_filter, SourceFilter::All);
        let _ = handle_normal_mode(&mut app, KeyCode::Char('f'), KeyModifiers::NONE);
        assert_eq!(app.source_filter, SourceFilter::Winget);
    }

    #[test]
    fn r_key_sets_loading_flag_and_status() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.loading = false;
        let _ = handle_normal_mode(&mut app, KeyCode::Char('r'), KeyModifiers::NONE);
        assert!(app.loading, "r should set loading = true");
        assert_eq!(app.status_message, "Refreshing...");
    }

    #[test]
    fn right_key_cycles_view_forward() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Search;
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(app.mode, AppMode::Installed);
    }

    #[test]
    fn left_key_cycles_view_backward() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        let _ = handle_normal_mode(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert_eq!(app.mode, AppMode::Search);
    }

    #[test]
    fn space_key_toggles_package_selection_in_upgrades() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.selected = 1;
        // First Space selects index 1
        let _ = handle_normal_mode(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(
            app.selected_packages.contains(&1),
            "index 1 should be selected after Space"
        );
        // Move selection back and press Space again to deselect
        app.selected = 1;
        let _ = handle_normal_mode(&mut app, KeyCode::Char(' '), KeyModifiers::NONE);
        assert!(
            !app.selected_packages.contains(&1),
            "index 1 should be deselected after second Space"
        );
    }

    // ── mouse hit-testing ────────────────────────────────────────────────────

    #[tokio::test]
    async fn select_package_at_row_ignores_header_row() {
        let mut app = make_app_with_pkgs(10);
        app.layout.package_list = rect(0, 10, 40, 10);
        app.layout.list_content_y = 13;
        *app.table_state.offset_mut() = 4;
        app.selected = 6;

        select_package_at_row(&mut app, 12);
        assert_eq!(app.selected, 6);
    }

    #[tokio::test]
    async fn mouse_left_click_on_second_last_column_selects_row_not_scrollbar() {
        let mut app = make_app_with_pkgs(20);
        app.layout.package_list = rect(0, 10, 40, 12);
        app.layout.list_content_y = 13;
        *app.table_state.offset_mut() = 5;
        app.selected = 0;
        // second-last column should behave like a normal list click
        let click_col = app.layout.package_list.x + app.layout.package_list.width - 2;
        let click_row = app.layout.list_content_y + 2;

        let _ = handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: click_col,
                row: click_row,
                modifiers: KeyModifiers::NONE,
            },
        );

        assert_eq!(app.selected, 7);
    }

    #[tokio::test]
    async fn mouse_wheel_up_scrolls_viewport_not_selection() {
        let mut app = make_app_with_pkgs(10);
        app.layout.package_list = rect(0, 10, 40, 12);
        app.selected = 2;
        *app.table_state.offset_mut() = 5;

        let _ = handle_mouse(
            &mut app,
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 5,
                row: 12,
                modifiers: KeyModifiers::NONE,
            },
        );

        // Selection should NOT move — only the viewport offset changes
        assert_eq!(app.selected, 2);
        assert_eq!(app.table_state.offset(), 2); // scrolled up by 3
    }

    // ── switch_view state reset ───────────────────────────────────────────────

    #[test]
    fn switch_view_clears_local_filter() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        app.local_filter = "chromium".to_string();
        // Right arrow switches Installed → Upgrades, triggering switch_view
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert!(
            app.local_filter.is_empty(),
            "switch_view must clear local_filter so the new view starts unfiltered"
        );
    }

    #[test]
    fn switch_view_clears_selected_packages() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.selected_packages = [0usize, 1, 2].iter().cloned().collect();
        // Left arrow switches Upgrades → Installed
        let _ = handle_normal_mode(&mut app, KeyCode::Left, KeyModifiers::NONE);
        assert!(
            app.selected_packages.is_empty(),
            "switch_view must clear the multi-select set; stale indices are invalid in the new view"
        );
    }

    #[test]
    fn switch_view_clears_detail_panel() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkg("Some.Package", "1.0", "");
        app.mode = AppMode::Installed;
        app.detail = Some(crate::models::PackageDetail {
            id: "Some.Package".to_string(),
            name: "Some Package".to_string(),
            ..crate::models::PackageDetail::default()
        });
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert!(
            app.detail.is_none(),
            "switch_view must clear the detail panel so stale detail from the old view is not shown"
        );
    }

    #[test]
    fn switch_view_increments_detail_generation() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        let gen_before = app.detail_generation;
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert!(
            app.detail_generation > gen_before,
            "switch_view must bump detail_generation to discard any in-flight detail requests from the old view"
        );
    }

    #[test]
    fn switch_view_resets_focus_to_package_list() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        app.focus = crate::app::FocusZone::DetailPanel;
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            app.focus,
            crate::app::FocusZone::PackageList,
            "switch_view must return keyboard focus to the package list"
        );
    }

    #[test]
    fn switch_view_no_op_when_already_on_target_mode() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(2);
        app.mode = AppMode::Installed;
        app.local_filter = "keepme".to_string();
        app.selected_packages = [0usize].iter().cloned().collect();
        let gen_before = app.detail_generation;
        // Simulate a Left key from Installed, then immediately a Right key to go back.
        // Pressing Right from Installed → Upgrades, then Left from Upgrades → Installed
        // resets state.  Here we test the early-return path: switching to the *current* mode.
        // We set up the mode to match what the Right key would produce.
        app.mode = AppMode::Upgrades;
        let _ = handle_normal_mode(&mut app, KeyCode::Right, KeyModifiers::NONE);
        // Right from Upgrades loops around to Search; that IS a view change, so
        // state resets.  Instead, directly invoke switch_view with the same mode
        // by pressing a key that goes to the already-active mode.
        // Simplest: set mode back and press a no-change cycle.
        // Actually test the private function directly.
        app.mode = AppMode::Installed;
        app.local_filter = "keepme".to_string();
        app.selected_packages = [0usize].iter().cloned().collect();
        let gen_at_test = app.detail_generation;
        // Call switch_view with the same mode (no-op path)
        switch_view(&mut app, AppMode::Installed);
        assert_eq!(
            app.local_filter, "keepme",
            "switch_view with the same mode must not clear local_filter"
        );
        assert_eq!(
            app.detail_generation, gen_at_test,
            "switch_view with the same mode must not bump detail_generation"
        );
        let _ = gen_before; // suppress unused warning
    }

    // ── scrollbar_jump ────────────────────────────────────────────────────────

    #[test]
    fn scrollbar_jump_selects_first_item_when_clicking_track_top() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(10);
        // Rect: x=0 y=5 w=30 h=12 → track_top=6, track_height=10
        app.layout.package_list = rect(0, 5, 30, 12);
        app.selected = 5;
        scrollbar_jump(&mut app, 6); // clicking at track_top → first item
        assert_eq!(
            app.selected, 0,
            "clicking the top of the scrollbar track should jump to item 0"
        );
    }

    #[test]
    fn scrollbar_jump_selects_last_item_when_clicking_track_bottom() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(10);
        // Rect: x=0 y=5 w=30 h=12 → track_top=6, track_height=10, last track row=15
        app.layout.package_list = rect(0, 5, 30, 12);
        app.selected = 0;
        scrollbar_jump(&mut app, 15); // last row of track → last item
        assert_eq!(
            app.selected, 9,
            "clicking the bottom of the scrollbar track should jump to the last item"
        );
    }

    #[test]
    fn scrollbar_jump_no_op_on_empty_list() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.layout.package_list = rect(0, 5, 30, 12);
        app.selected = 0;
        // Should not panic or mutate selected
        scrollbar_jump(&mut app, 6);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn scrollbar_jump_clamps_row_below_track_top_to_first_item() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(5);
        // Rect: x=0 y=10 w=20 h=8 → track_top=11, track_height=6
        app.layout.package_list = rect(0, 10, 20, 8);
        app.selected = 3;
        // row=5 is above track_top (11); clamped to track_top → first item
        scrollbar_jump(&mut app, 5);
        assert_eq!(
            app.selected, 0,
            "a row above the track top should be clamped and jump to item 0"
        );
    }

    // ── Help overlay scroll ───────────────────────────────────────────────────

    #[test]
    fn help_scroll_starts_at_zero() {
        let app = make_app();
        assert_eq!(app.help_scroll, 0);
        assert_eq!(app.help_max_scroll, 0);
    }

    #[test]
    fn help_scroll_down_increments_within_max() {
        let mut app = make_app();
        app.show_help = true;
        app.help_max_scroll = 10;

        handle_help_input(&mut app, KeyCode::Down);
        assert_eq!(app.help_scroll, 1);
        assert!(app.show_help, "help should still be open");
    }

    #[test]
    fn help_scroll_down_clamped_at_max() {
        let mut app = make_app();
        app.show_help = true;
        app.help_scroll = 10;
        app.help_max_scroll = 10;

        handle_help_input(&mut app, KeyCode::Down);
        assert_eq!(app.help_scroll, 10, "scroll should not exceed max");
    }

    #[test]
    fn help_scroll_up_saturates_at_zero() {
        let mut app = make_app();
        app.show_help = true;
        app.help_scroll = 0;
        app.help_max_scroll = 10;

        handle_help_input(&mut app, KeyCode::Up);
        assert_eq!(app.help_scroll, 0, "should not underflow");
    }

    #[test]
    fn help_scroll_home_resets_to_zero() {
        let mut app = make_app();
        app.show_help = true;
        app.help_scroll = 8;
        app.help_max_scroll = 10;

        handle_help_input(&mut app, KeyCode::Home);
        assert_eq!(app.help_scroll, 0);
    }

    #[test]
    fn help_scroll_end_jumps_to_max() {
        let mut app = make_app();
        app.show_help = true;
        app.help_scroll = 0;
        app.help_max_scroll = 15;

        handle_help_input(&mut app, KeyCode::End);
        assert_eq!(app.help_scroll, 15);
    }

    #[test]
    fn help_close_resets_scroll() {
        let mut app = make_app();
        app.show_help = true;
        app.help_scroll = 8;
        app.help_max_scroll = 10;

        handle_help_input(&mut app, KeyCode::Esc);
        assert!(!app.show_help);
        assert_eq!(app.help_scroll, 0, "scroll should reset when help closes");
    }

    // ── handle_tab_click ─────────────────────────────────────────────────────

    #[test]
    fn tab_click_switches_to_installed_view() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Search;
        // Register three tab regions: Search=0..10, Installed=10..20, Upgrades=20..30
        app.layout.tab_regions = vec![
            (0, 10, AppMode::Search),
            (10, 20, AppMode::Installed),
            (20, 30, AppMode::Upgrades),
        ];
        handle_tab_click(&mut app, 10); // exactly at Installed start_x
        assert_eq!(
            app.mode,
            AppMode::Installed,
            "clicking Installed tab should switch to Installed"
        );
    }

    #[test]
    fn tab_click_switches_to_upgrades_view() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Search;
        app.layout.tab_regions = vec![
            (0, 10, AppMode::Search),
            (10, 20, AppMode::Installed),
            (20, 30, AppMode::Upgrades),
        ];
        handle_tab_click(&mut app, 25); // mid-Upgrades region
        assert_eq!(
            app.mode,
            AppMode::Upgrades,
            "clicking Upgrades tab should switch to Upgrades"
        );
    }

    #[test]
    fn tab_click_end_x_is_exclusive() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Search;
        app.layout.tab_regions = vec![(0, 10, AppMode::Search), (10, 20, AppMode::Installed)];
        // col == 20 is exactly at end_x of Installed, which is exclusive — no region matches
        handle_tab_click(&mut app, 20);
        assert_eq!(
            app.mode,
            AppMode::Search,
            "col == end_x should not activate the tab (end_x is exclusive)"
        );
    }

    #[test]
    fn tab_click_outside_all_regions_is_noop() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        app.local_filter = "keepme".to_string();
        app.layout.tab_regions = vec![
            (0, 10, AppMode::Search),
            (10, 20, AppMode::Installed),
            (20, 30, AppMode::Upgrades),
        ];
        handle_tab_click(&mut app, 99); // outside all regions
        assert_eq!(
            app.mode,
            AppMode::Installed,
            "click outside tab regions should leave mode unchanged"
        );
        assert_eq!(
            app.local_filter, "keepme",
            "click outside tab regions should leave filter unchanged"
        );
    }

    #[test]
    fn tab_click_on_current_mode_tab_is_noop() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Installed;
        app.local_filter = "filter".to_string();
        app.layout.tab_regions = vec![
            (0, 10, AppMode::Search),
            (10, 20, AppMode::Installed),
            (20, 30, AppMode::Upgrades),
        ];
        // Clicking the already-active tab calls switch_view with the same mode,
        // which is a no-op — state should be preserved.
        handle_tab_click(&mut app, 15);
        assert_eq!(
            app.mode,
            AppMode::Installed,
            "clicking the current-mode tab should stay in that mode"
        );
        assert_eq!(
            app.local_filter, "filter",
            "clicking the current-mode tab must not clear local_filter"
        );
    }

    #[test]
    fn tab_click_with_empty_regions_is_noop() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app();
        app.mode = AppMode::Search;
        // No tab regions registered (e.g., layout not yet computed)
        app.layout.tab_regions = vec![];
        handle_tab_click(&mut app, 5);
        assert_eq!(
            app.mode,
            AppMode::Search,
            "no tab regions: mode must be unchanged"
        );
    }

    // ── click_sort_header ─────────────────────────────────────────────────────

    fn make_app_with_list_layout() -> App {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        // package_list starts at x=0, width=100; content_y=3 (border+pad+header)
        app.layout.package_list = rect(0, 0, 100, 10);
        app.layout.list_content_y = 3; // header row is at y=2
        app
    }

    #[test]
    fn click_sort_header_name_column_sets_name_sort() {
        let mut app = make_app_with_list_layout();
        // Content width = 100 - 3 = 97; Name occupies 0..24 (25%)
        // Click at col=5 (within Name column), row=2 (header row)
        click_sort_header(&mut app, 5);
        assert_eq!(app.sort_field, SortField::Name);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_id_column_sets_id_sort() {
        let mut app = make_app_with_list_layout();
        // Content width=97; ID starts at 24 (25%), width 34 (35%); click at col=30
        click_sort_header(&mut app, 30);
        assert_eq!(app.sort_field, SortField::Id);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_version_column_sets_version_sort() {
        let mut app = make_app_with_list_layout();
        // Version starts at ~58 (25+34=59 rounded); click at col=65
        click_sort_header(&mut app, 65);
        assert_eq!(app.sort_field, SortField::Version);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_same_column_toggles_direction() {
        let mut app = make_app_with_list_layout();
        click_sort_header(&mut app, 5); // Name Asc
        assert_eq!(app.sort_dir, SortDir::Asc);
        click_sort_header(&mut app, 5); // Name Desc
        assert_eq!(app.sort_dir, SortDir::Desc);
        click_sort_header(&mut app, 5); // Name Asc again
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_different_column_resets_to_asc() {
        let mut app = make_app_with_list_layout();
        click_sort_header(&mut app, 5); // Name Asc
        click_sort_header(&mut app, 5); // Name Desc
        assert_eq!(app.sort_dir, SortDir::Desc);
        click_sort_header(&mut app, 30); // ID → resets to Asc
        assert_eq!(app.sort_field, SortField::Id);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_source_column_is_noop() {
        let mut app = make_app_with_list_layout();
        // Source starts at ~80 (25+35+20=80%); click at col=90
        click_sort_header(&mut app, 90);
        assert_eq!(
            app.sort_field,
            SortField::None,
            "Source column must not set sort"
        );
    }

    #[test]
    fn click_sort_header_zero_width_is_noop() {
        let mut app = make_app_with_list_layout();
        app.layout.package_list = rect(0, 0, 2, 10); // content_width = 2-3 = underflows to 0
        click_sort_header(&mut app, 0);
        assert_eq!(app.sort_field, SortField::None);
    }

    #[test]
    fn click_sort_header_upgrades_available_column_sets_available_sort() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.layout.package_list = rect(0, 0, 100, 10);
        // Content width=97; Upgrades: Name 25%, ID 30%, Version 15%, Available 15%
        // boundary_available = 97*25/100 + 97*30/100 + 97*15/100 = 24+29+14=67
        // Available column: 67..82; click at col=75
        click_sort_header(&mut app, 75);
        assert_eq!(app.sort_field, SortField::AvailableVersion);
        assert_eq!(app.sort_dir, SortDir::Asc);
    }

    #[test]
    fn click_sort_header_upgrades_available_column_toggles_direction() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.layout.package_list = rect(0, 0, 100, 10);
        click_sort_header(&mut app, 75); // Available Asc
        assert_eq!(app.sort_dir, SortDir::Asc);
        click_sort_header(&mut app, 75); // Available Desc
        assert_eq!(app.sort_dir, SortDir::Desc);
    }

    #[test]
    fn click_sort_header_upgrades_source_column_is_noop() {
        let rt = test_runtime();
        let _guard = rt.enter();
        let mut app = make_app_with_pkgs(3);
        app.mode = AppMode::Upgrades;
        app.layout.package_list = rect(0, 0, 100, 10);
        // Source: 82..97 in Upgrades view; click at col=90
        click_sort_header(&mut app, 90);
        assert_eq!(
            app.sort_field,
            SortField::None,
            "Source column must not sort"
        );
    }
}
