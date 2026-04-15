use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::{App, AppMode, ConfirmDialog, FocusZone, InputMode};
use crate::theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    let header_height = theme::LOGO_HEIGHT; // logo + tabs, no extra spacing
    let show_search_bar = app.mode == AppMode::Search || app.input_mode == InputMode::Search;

    if show_search_bar {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // header (logo + tabs)
                Constraint::Length(1),             // spacing after header
                Constraint::Length(1),             // search bar
                Constraint::Length(1),             // spacing before cards
                Constraint::Min(5),               // main content
                Constraint::Length(1),             // status bar
            ])
            .split(f.area());

        draw_header(f, app, chunks[0]);
        // chunks[1] is spacing after header
        draw_search_bar(f, app, chunks[2]);
        // chunks[3] is spacing before cards
        draw_main_content(f, app, chunks[4]);
        draw_status_bar(f, app, chunks[5]);
    } else {
        app.layout.search_bar = Rect::default();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height), // header (logo + tabs)
                Constraint::Length(1),             // spacing
                Constraint::Min(5),               // main content
                Constraint::Length(1),             // status bar
            ])
            .split(f.area());

        draw_header(f, app, chunks[0]);
        // chunks[1] is the spacing line
        draw_main_content(f, app, chunks[2]);
        draw_status_bar(f, app, chunks[3]);
    }

    if let Some(confirm) = &app.confirm {
        draw_confirm_dialog(f, confirm);
    }

    if app.show_help {
        draw_help_overlay(f);
    }
}

fn draw_header(f: &mut Frame, app: &mut App, area: Rect) {
    // Split: logo on left (34 chars) | spacing (3 chars) | tabs on right
    let logo_width = 33u16; // 31 word-art + 1 padding each side
    let gap = 4u16;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(logo_width),
            Constraint::Length(gap),
            Constraint::Min(1),
        ])
        .split(area);

    // Store tab bar region for mouse clicks
    app.layout.tab_bar = chunks[2];

    // Draw pixel-art logo (vertically centered in the area, excluding spacing row)
    let logo_lines = theme::logo_lines();
    let logo = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo, chunks[0]);

    // Draw tabs vertically centered in the right area
    let tabs_area = chunks[2];
    let tabs = [
        (AppMode::Search, "\u{25C7} Search"),     // ◇ Search
        (AppMode::Installed, "\u{25A3} Installed"), // ▣ Installed
        (AppMode::Upgrades, "\u{25B3} Upgrades"),  // △ Upgrades
    ];

    // Calculate vertical center row (center within the logo height, not the spacing)
    let center_y = tabs_area.y + theme::LOGO_HEIGHT / 2;

    // Build tab spans and track click regions
    let mut current_x = tabs_area.x;
    let mut tab_regions = Vec::new();

    let spans: Vec<Span> = tabs
        .iter()
        .flat_map(|(mode, label)| {
            let style = if *mode == app.mode {
                theme::navbar_active()
            } else {
                theme::navbar_inactive()
            };
            let tab_text = format!(" {} ", label);
            let tab_width = UnicodeWidthStr::width(tab_text.as_str()) as u16;
            tab_regions.push((current_x, current_x + tab_width, *mode));
            current_x += tab_width + 1;
            vec![Span::styled(tab_text, style), Span::raw(" ")]
        })
        .collect();

    let tab_line = Line::from(spans);
    let tab_rect = Rect {
        x: tabs_area.x,
        y: center_y,
        width: tabs_area.width,
        height: 1,
    };
    f.render_widget(Paragraph::new(tab_line), tab_rect);
    app.layout.tab_regions = tab_regions;
}

fn draw_search_bar(f: &mut Frame, app: &mut App, area: Rect) {
    // Store region for mouse clicks
    app.layout.search_bar = area;

    let search_style = if app.input_mode == InputMode::Search {
        Style::default()
            .fg(theme::TEXT_PRIMARY)
            .bg(theme::SURFACE)
    } else {
        Style::default().fg(theme::TEXT_SECONDARY)
    };

    let search_text = if app.search_query.is_empty() && app.input_mode != InputMode::Search {
        " / to search...".to_string()
    } else {
        format!(" {}", app.search_query)
    };

    let search = Paragraph::new(search_text).style(search_style);
    f.render_widget(search, area);

    // Show cursor in search mode
    if app.input_mode == InputMode::Search {
        let cursor_x = area.x + 1 + UnicodeWidthStr::width(app.search_query.as_str()) as u16;
        f.set_cursor_position((cursor_x, area.y));
    }
}

fn draw_main_content(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Store regions for mouse hit-testing
    app.layout.package_list = chunks[0];
    app.layout.detail_panel = chunks[1];
    // Content starts after border (1) + padding (1) + header row (1)
    app.layout.list_content_y = chunks[0].y + 3;

    draw_package_list(f, app, chunks[0]);
    draw_detail_panel(f, app, chunks[1]);
}

fn draw_package_list(f: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.focus == FocusZone::PackageList;

    let title = match app.mode {
        AppMode::Search => "Search Results".to_string(),
        AppMode::Installed => "Installed".to_string(),
        AppMode::Upgrades => {
            let sel = app.selected_packages.len();
            if sel > 0 {
                format!("Upgrades -- {} selected", sel)
            } else {
                "Upgrades".to_string()
            }
        }
    };

    let header_cells = if app.mode == AppMode::Upgrades {
        vec!["     Name", "ID", "Version", "Available", "Source"]
    } else {
        vec!["  Name", "ID", "Version", "Source"]
    };

    let header = Row::new(
        header_cells
            .iter()
            .map(|h| Cell::from(*h).style(theme::table_header())),
    )
    .height(1);

    let rows: Vec<Row> = app
        .filtered_packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let is_selected = i == app.selected;
            let is_marked = app.mode == AppMode::Upgrades && app.selected_packages.contains(&i);
            let style = if is_selected {
                theme::selected_row()
            } else if is_marked {
                theme::marked_row()
            } else {
                Style::default()
            };

            let prefix = if app.mode == AppMode::Upgrades {
                if is_marked && is_selected {
                    "\u{25CF}[x] "  // ● selected + marked
                } else if is_marked {
                    " [x] "
                } else if is_selected {
                    "\u{25CF}[ ] "  // ● selected
                } else {
                    " [ ] "
                }
            } else if is_selected {
                "\u{25CF} "  // ●
            } else {
                "  "
            };

            let cells: Vec<Cell> = if app.mode == AppMode::Upgrades {
                vec![
                    Cell::from(format!("{}{}", prefix, truncate(&pkg.name, 18))),
                    Cell::from(truncate(&pkg.id, 25)),
                    Cell::from(pkg.version.clone()),
                    Cell::from(Span::styled(
                        &pkg.available_version,
                        Style::default().fg(theme::SUCCESS),
                    )),
                    Cell::from(pkg.source.clone()),
                ]
            } else {
                vec![
                    Cell::from(format!("{}{}", prefix, truncate(&pkg.name, 18))),
                    Cell::from(truncate(&pkg.id, 28)),
                    Cell::from(pkg.version.clone()),
                    Cell::from(pkg.source.clone()),
                ]
            };

            Row::new(cells).style(style)
        })
        .collect();

    let widths = if app.mode == AppMode::Upgrades {
        vec![
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ]
    } else {
        vec![
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ]
    };

    let border_style = if is_focused {
        theme::border_focused()
    } else {
        theme::border_unfocused()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(format!(" {} ({}) ", title, app.filtered_packages.len()))
        .title_style(theme::title())
        .padding(ratatui::widgets::Padding::top(1));

    // Loading / empty state
    let loading_msg = if app.loading {
        Some(format!(" {} Loading...", app.spinner()))
    } else if app.filtered_packages.is_empty() {
        Some(
            match app.mode {
                AppMode::Search if app.search_query.is_empty() => " Type / to search for packages",
                AppMode::Search => " No results found",
                AppMode::Installed => " No packages found",
                AppMode::Upgrades => " All packages are up to date!",
            }
            .to_string(),
        )
    } else {
        None
    };

    if let Some(msg) = loading_msg {
        let p = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(theme::TEXT_SECONDARY));
        f.render_widget(p, area);
        return;
    }

    let table = Table::new(rows, &widths)
        .header(header)
        .block(block)
        .row_highlight_style(theme::selected_row());

    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(table, area, &mut state);
    // Capture scroll offset for mouse click hit-testing
    app.table_scroll_offset = state.offset();

    // Scrollbar
    if app.filtered_packages.len() > (area.height as usize).saturating_sub(3) {
        let mut scrollbar_state =
            ScrollbarState::new(app.filtered_packages.len()).position(app.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("\u{25B2}"))  // ▲
            .end_symbol(Some("\u{25BC}"))    // ▼
            .track_symbol(Some("\u{2502}"))  // │
            .thumb_symbol("\u{2588}");       // █
        f.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn draw_detail_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusZone::DetailPanel;

    let title = if app.detail_loading {
        format!(" {} Loading Details... ", app.spinner())
    } else {
        " Package Details ".to_string()
    };

    let border_style = if is_focused {
        theme::border_focused()
    } else {
        theme::border_unfocused()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title)
        .title_style(theme::title())
        .padding(ratatui::widgets::Padding::top(1));

    if let Some(detail) = &app.detail {
        let label_style = theme::detail_label();

        let available_version = app
            .selected_package()
            .map(|p| p.available_version.as_str())
            .unwrap_or("");

        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Name      ", label_style),
                Span::raw(&detail.name),
            ]),
            Line::from(vec![
                Span::styled("  ID        ", label_style),
                Span::styled(&detail.id, Style::default().fg(theme::INFO)),
            ]),
            Line::from(vec![
                Span::styled("  Version   ", label_style),
                Span::raw(&detail.version),
            ]),
        ];

        if !available_version.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  Available ", label_style),
                Span::styled(
                    available_version.to_string(),
                    Style::default()
                        .fg(theme::SUCCESS)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines.extend([
            Line::from(vec![
                Span::styled("  Publisher ", label_style),
                Span::raw(&detail.publisher),
            ]),
            Line::from(vec![
                Span::styled("  Source    ", label_style),
                Span::raw(&detail.source),
            ]),
        ]);

        if !detail.license.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  License   ", label_style),
                Span::raw(&detail.license),
            ]));
        }

        if !detail.homepage.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    &detail.homepage,
                    Style::default()
                        .fg(theme::INFO)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));
        }

        if !detail.description.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled("  Description", label_style)));
            // Manually word-wrap description to maintain consistent 2-space indent
            let desc_style = Style::default().fg(theme::TEXT_SECONDARY);
            let indent = "  ";
            // Available width: area minus borders (2) minus block padding (0 horiz) minus indent (2)
            let max_width = (area.width as usize).saturating_sub(4);
            for wrapped_line in word_wrap(&detail.description, max_width) {
                lines.push(Line::from(vec![
                    Span::raw(indent),
                    Span::styled(wrapped_line, desc_style),
                ]));
            }
        }

        lines.push(Line::raw(""));

        // Show context-appropriate actions (stacked vertically with spacing)
        let has_upgrade = !available_version.is_empty();
        match app.mode {
            AppMode::Search => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" i ", theme::action_install()),
                    Span::raw(" Install"),
                ]));
            }
            AppMode::Installed => {
                if has_upgrade {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(" u ", theme::action_key()),
                        Span::raw(" Upgrade"),
                    ]));
                    lines.push(Line::raw(""));
                }
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" x ", theme::action_danger()),
                    Span::raw(" Uninstall"),
                ]));
            }
            AppMode::Upgrades => {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" u ", theme::action_key()),
                    Span::raw(" Upgrade"),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" x ", theme::action_danger()),
                    Span::raw(" Uninstall"),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" Spc ", theme::action_key()),
                    Span::raw(" Select"),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(" a ", theme::action_key()),
                    Span::raw(" All"),
                ]));
                if !app.selected_packages.is_empty() {
                    lines.push(Line::raw(""));
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(" U ", theme::action_key()),
                        Span::raw(format!(" Upgrade {}", app.selected_packages.len())),
                    ]));
                }
            }
        }
        // Open homepage hint when available
        if !detail.homepage.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(" o ", theme::action_key()),
                Span::raw(" Open homepage"),
            ]));
        }

        let p = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        f.render_widget(p, area);
    } else {
        let msg = if app.filtered_packages.is_empty() {
            "  No package selected".to_string()
        } else if app.loading {
            format!("  {} Loading...", app.spinner())
        } else if app.selected_package().is_some_and(|p| p.is_truncated()) {
            "  Package ID is truncated -- details unavailable".to_string()
        } else {
            "  Select a package to view details".to_string()
        };
        let p = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(theme::TEXT_SECONDARY));
        f.render_widget(p, area);
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let filter_text = format!(" {} ", app.source_filter);
    let filter_len = UnicodeWidthStr::width(filter_text.as_str()) as u16 + 2; // + padding

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(filter_len), // filter badge
            Constraint::Min(1),            // status message
            Constraint::Length(70),        // global hotkeys
        ])
        .split(area);

    // Filter badge
    let filter_style = match app.source_filter {
        crate::models::SourceFilter::All => {
            Style::default().fg(theme::TEXT_PRIMARY).bg(theme::SURFACE)
        }
        crate::models::SourceFilter::Winget => {
            Style::default().fg(theme::TEXT_ON_ACCENT).bg(theme::INFO)
        }
        crate::models::SourceFilter::MsStore => {
            Style::default()
                .fg(theme::TEXT_ON_ACCENT)
                .bg(theme::SELECTION)
        }
    };
    let filter_badge = Paragraph::new(filter_text).style(filter_style);
    f.render_widget(filter_badge, chunks[0]);

    // Status message with spinner when loading
    let status_text = if app.loading {
        format!(" {} {}", app.spinner(), app.status_message)
    } else {
        format!(" {}", app.status_message)
    };
    let status_style =
        if app.status_message.contains("failed") || app.status_message.contains("Error") {
            theme::status_error()
        } else if app.loading {
            theme::status_loading()
        } else {
            theme::status_normal()
        };
    let status = Paragraph::new(status_text).style(status_style);
    f.render_widget(status, chunks[1]);

    // Global hotkey badges
    let key_style = theme::action_key();
    let sep = Span::raw(" ");
    let label_style = Style::default().fg(theme::TEXT_SECONDARY).bg(theme::SURFACE);

    let hotkeys = if app.input_mode == InputMode::Search {
        Line::from(vec![
            Span::styled(" Esc ", key_style),
            Span::styled(" Cancel ", label_style),
            sep.clone(),
            Span::styled(" Enter ", key_style),
            Span::styled(" Search ", label_style),
        ])
    } else {
        Line::from(vec![
            Span::styled(" / ", key_style),
            Span::styled(" Search ", label_style),
            sep.clone(),
            Span::styled(" f ", key_style),
            Span::styled(" Filter ", label_style),
            sep.clone(),
            Span::styled(" r ", key_style),
            Span::styled(" Refresh ", label_style),
            sep.clone(),
            Span::styled(" ? ", key_style),
            Span::styled(" Help ", label_style),
            sep,
            Span::styled(" q ", key_style),
            Span::styled(" Quit ", label_style),
        ])
    };

    let hints = Paragraph::new(hotkeys)
        .style(Style::default().bg(theme::SURFACE))
        .alignment(Alignment::Right);
    f.render_widget(hints, chunks[2]);
}

fn draw_confirm_dialog(f: &mut Frame, confirm: &ConfirmDialog) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_focused())
        .title(" Confirm ")
        .title_style(
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme::SURFACE));

    let lines = vec![
        Line::raw(""),
        Line::from(vec![Span::raw("  "), Span::raw(&confirm.message)]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(" y ", theme::action_confirm()),
            Span::raw(" Yes   "),
            Span::styled(" n ", theme::action_danger()),
            Span::raw(" No"),
        ]),
    ];

    let p = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::border_focused())
        .title(" Help -- Keybindings ")
        .title_style(
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme::SURFACE));

    let section = theme::help_section();
    let key = theme::help_key();

    let help_text = vec![
        Line::raw(""),
        Line::from(Span::styled("  Navigation", section)),
        Line::from(vec![
            Span::styled("  up/k  dn/j  ", key),
            Span::raw("Move up / down"),
        ]),
        Line::from(vec![
            Span::styled("  PgUp/PgDn   ", key),
            Span::raw("Jump 20 items"),
        ]),
        Line::from(vec![
            Span::styled("  Home/End    ", key),
            Span::raw("Jump to first / last"),
        ]),
        Line::from(vec![
            Span::styled("  lt/rt       ", key),
            Span::raw("Switch view (Search / Installed / Upgrades)"),
        ]),
        Line::from(vec![
            Span::styled("  /           ", key),
            Span::raw("Focus search"),
        ]),
        Line::from(vec![
            Span::styled("  f           ", key),
            Span::raw("Cycle source filter"),
        ]),
        Line::from(vec![
            Span::styled("  r           ", key),
            Span::raw("Refresh"),
        ]),
        Line::raw(""),
        Line::from(Span::styled("  Actions", section)),
        Line::from(vec![
            Span::styled("  i           ", key),
            Span::raw("Install selected package"),
        ]),
        Line::from(vec![
            Span::styled("  u           ", key),
            Span::raw("Upgrade selected package"),
        ]),
        Line::from(vec![
            Span::styled("  x           ", key),
            Span::raw("Uninstall selected package"),
        ]),
        Line::from(vec![
            Span::styled("  Space       ", key),
            Span::raw("Toggle select (Upgrades view)"),
        ]),
        Line::from(vec![
            Span::styled("  a           ", key),
            Span::raw("Select / deselect all (Upgrades)"),
        ]),
        Line::from(vec![
            Span::styled("  U           ", key),
            Span::raw("Batch upgrade selected packages"),
        ]),
        Line::from(vec![
            Span::styled("  Enter       ", key),
            Span::raw("Show package details / activate nav"),
        ]),
        Line::from(vec![
            Span::styled("  o           ", key),
            Span::raw("Open homepage in browser"),
        ]),
        Line::raw(""),
        Line::from(Span::styled("  Mouse", section)),
        Line::from(vec![
            Span::styled("  Click       ", key),
            Span::raw("Select nav items, rows, filter"),
        ]),
        Line::from(vec![
            Span::styled("  Scroll      ", key),
            Span::raw("Navigate list"),
        ]),
        Line::raw(""),
        Line::from(Span::styled("  General", section)),
        Line::from(vec![
            Span::styled("  ?           ", key),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc     ", key),
            Span::raw("Quit / Close dialog"),
        ]),
        Line::from(vec![Span::styled("  Ctrl+C      ", key), Span::raw("Quit")]),
        Line::raw(""),
    ];

    let p = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Truncate `s` to at most `max` **display columns**, appending '...' if truncated.
/// Uses Unicode display widths so CJK characters (width 2) are counted correctly.
fn truncate(s: &str, max: usize) -> String {
    if UnicodeWidthStr::width(s) <= max {
        return s.to_string();
    }
    // Reserve one column for the ellipsis character.
    let budget = max.saturating_sub(1);
    let mut display_width = 0usize;
    let mut result = String::new();
    for ch in s.chars() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
        if display_width + cw > budget {
            break;
        }
        display_width += cw;
        result.push(ch);
    }
    format!("{result}\u{2026}")
}

/// Word-wrap text into lines of at most `max_width` display columns.
/// Breaks on word boundaries when possible; forces a break mid-word if a
/// single word exceeds the line width.
fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    // Force-break a single word that exceeds max_width into multiple lines
    let force_break = |word: &str, lines: &mut Vec<String>| -> (String, usize) {
        let mut w = 0usize;
        let mut buf = String::new();
        for ch in word.chars() {
            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
            if w + cw > max_width && !buf.is_empty() {
                lines.push(std::mem::take(&mut buf));
                w = 0;
            }
            buf.push(ch);
            w += cw;
        }
        (buf, w)
    };

    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        let mut line_width = 0usize;
        for word in paragraph.split_whitespace() {
            let word_width = UnicodeWidthStr::width(word);
            if line_width == 0 {
                if word_width <= max_width {
                    line.push_str(word);
                    line_width = word_width;
                } else {
                    let (buf, w) = force_break(word, &mut lines);
                    line = buf;
                    line_width = w;
                }
            } else if line_width + 1 + word_width <= max_width {
                line.push(' ');
                line.push_str(word);
                line_width += 1 + word_width;
            } else {
                lines.push(std::mem::take(&mut line));
                if word_width <= max_width {
                    line = word.to_string();
                    line_width = word_width;
                } else {
                    let (buf, w) = force_break(word, &mut lines);
                    line = buf;
                    line_width = w;
                }
            }
        }
        lines.push(line);
        // line is moved, so next paragraph starts fresh
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii_within_limit() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_ascii_at_exact_limit() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_ascii_over_limit() {
        // "hello world" = 11 chars; max=8 -> keep 7 + ellipsis
        assert_eq!(truncate("hello world", 8), "hello w\u{2026}");
    }

    #[test]
    fn truncate_cjk_within_limit() {
        // Each CJK char is 2 display columns; "你好" = 4 cols, max=5
        assert_eq!(truncate("你好", 5), "你好");
    }

    #[test]
    fn truncate_cjk_over_limit() {
        // "你好世界" = 8 display cols; max=5 -> keep 2 cols (one CJK) + ellipsis
        // budget=4 -> "你好" (4 cols) fits, "你好世" would be 6 > 4
        assert_eq!(truncate("你好世界", 5), "你好\u{2026}");
    }

    #[test]
    fn truncate_mixed_ascii_cjk() {
        // "hi你好" = 2 + 4 = 6 cols; max=5 -> keep "hi你" (4 cols) + ellipsis
        assert_eq!(truncate("hi你好", 5), "hi你\u{2026}");
    }
}
