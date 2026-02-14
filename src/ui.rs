use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
};

use unicode_width::UnicodeWidthStr;

use crate::app::{App, AppMode, ConfirmDialog, InputMode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // filter + search
            Constraint::Min(5),   // main content
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    // Store layout regions for mouse hit-testing
    app.layout.tab_bar = chunks[0];

    draw_tab_bar(f, app, chunks[0]);
    draw_filter_bar(f, app, chunks[1]);
    draw_main_content(f, app, chunks[2]);
    draw_status_bar(f, app, chunks[3]);

    if let Some(confirm) = &app.confirm {
        draw_confirm_dialog(f, confirm);
    }

    if app.show_help {
        draw_help_overlay(f);
    }
}

fn draw_tab_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let tabs = [
        (AppMode::Search, "🔍 Search"),
        (AppMode::Installed, "📦 Installed"),
        (AppMode::Upgrades, "⬆️  Upgrades"),
    ];

    // Calculate tab positions for mouse click hit-testing
    let title = " winget-tui ";
    let title_width = UnicodeWidthStr::width(title) as u16;
    let spacing = 2u16;
    let mut current_x = title_width + spacing;
    let mut tab_regions = Vec::new();

    let spans: Vec<Span> = tabs
        .iter()
        .flat_map(|(mode, label)| {
            let style = if *mode == app.mode {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let tab_text = format!(" {} ", label);
            let tab_width = UnicodeWidthStr::width(tab_text.as_str()) as u16;
            tab_regions.push((current_x, current_x + tab_width, *mode));
            current_x += tab_width + 1; // +1 for separator space
            vec![
                Span::styled(tab_text, style),
                Span::raw(" "),
            ]
        })
        .collect();

    let title_span = Span::styled(
        title,
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    );

    let mut all_spans = vec![title_span, Span::raw("  ")];
    all_spans.extend(spans);

    f.render_widget(Paragraph::new(Line::from(all_spans)), area);
    app.layout.tab_regions = tab_regions;
}

fn draw_filter_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(1)])
        .split(area);

    // Store regions for mouse clicks
    app.layout.filter_bar = chunks[0];
    app.layout.search_bar = chunks[1];

    // Source filter with icon
    let filter_text = format!(" 🔽 Filter: [{}] ", app.source_filter);
    let filter = Paragraph::new(filter_text).style(Style::default().fg(Color::Yellow));
    f.render_widget(filter, chunks[0]);

    // Search input
    let search_style = if app.input_mode == InputMode::Search {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let search_text = if app.search_query.is_empty() && app.input_mode != InputMode::Search {
        " / to search...".to_string()
    } else {
        format!(" 🔍 {}", app.search_query)
    };

    let search = Paragraph::new(search_text).style(search_style);
    f.render_widget(search, chunks[1]);

    // Show cursor in search mode
    if app.input_mode == InputMode::Search {
        let cursor_x = chunks[1].x + 4 + UnicodeWidthStr::width(app.search_query.as_str()) as u16;
        f.set_cursor_position((cursor_x, chunks[1].y));
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
    // Content starts after border (1) + header row (1)
    app.layout.list_content_y = chunks[0].y + 2;

    draw_package_list(f, app, chunks[0]);
    draw_detail_panel(f, app, chunks[1]);
}

fn draw_package_list(f: &mut Frame, app: &mut App, area: Rect) {
    let (icon, title) = match app.mode {
        AppMode::Search => ("🔍", "Search Results"),
        AppMode::Installed => ("📦", "Installed"),
        AppMode::Upgrades => ("⬆️ ", "Upgrades"),
    };

    let header_cells = if app.mode == AppMode::Upgrades {
        vec!["Name", "ID", "Version", "Available", "Source"]
    } else {
        vec!["Name", "ID", "Version", "Source"]
    };

    let header = Row::new(
        header_cells
            .iter()
            .map(|h| {
                Cell::from(*h).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            }),
    )
    .height(1);

    let rows: Vec<Row> = app
        .filtered_packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            let is_selected = i == app.selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            // Show checkbox for marked packages in Upgrades view, otherwise use arrow
            let prefix = if app.mode == AppMode::Upgrades {
                let is_marked = app.selected_packages.contains(&i);
                match (is_selected, is_marked) {
                    (true, true) => "►✓",
                    (false, true) => " ✓",
                    (true, false) => "► ",
                    (false, false) => "  ",
                }
            } else {
                if is_selected { "► " } else { "  " }
            };

            let cells: Vec<Cell> = if app.mode == AppMode::Upgrades {
                // In Upgrades view, the name field is truncated to 17 chars instead of 18
                // to ensure consistent column width for all rows, accounting for the
                // checkbox character (✓) that may appear in any row
                vec![
                    Cell::from(format!("{}{}", prefix, truncate(&pkg.name, 17))),
                    Cell::from(truncate(&pkg.id, 25)),
                    Cell::from(pkg.version.clone()),
                    Cell::from(
                        Span::styled(&pkg.available_version, Style::default().fg(Color::Green)),
                    ),
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(format!(" {icon} {title} ({}) ", app.filtered_packages.len()))
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Loading / empty state
    let loading_msg = if app.loading {
        Some(format!(" {} Loading...", app.spinner()))
    } else if app.filtered_packages.is_empty() {
        Some(
            match app.mode {
                AppMode::Search if app.search_query.is_empty() => {
                    " Type / to search for packages"
                }
                AppMode::Search => " No results found",
                AppMode::Installed => " No packages found",
                AppMode::Upgrades => " ✅ All packages are up to date!",
            }
            .to_string(),
        )
    } else {
        None
    };

    if let Some(msg) = loading_msg {
        let p = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
        return;
    }

    let table = Table::new(rows, &widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = TableState::default();
    state.select(Some(app.selected));
    f.render_stateful_widget(table, area, &mut state);
    // Capture scroll offset for mouse click hit-testing
    app.table_scroll_offset = state.offset();

    // Scrollbar
    if app.filtered_packages.len() > (area.height as usize).saturating_sub(3) {
        let mut scrollbar_state = ScrollbarState::new(app.filtered_packages.len())
            .position(app.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        f.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

fn draw_detail_panel(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.detail_loading {
        format!(" {} Loading Details... ", app.spinner())
    } else {
        " 📋 Package Details ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    if let Some(detail) = &app.detail {
        let label_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("  Name      ", label_style),
                Span::raw(&detail.name),
            ]),
            Line::from(vec![
                Span::styled("  ID        ", label_style),
                Span::styled(&detail.id, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled("  Version   ", label_style),
                Span::raw(&detail.version),
            ]),
            Line::from(vec![
                Span::styled("  Publisher ", label_style),
                Span::raw(&detail.publisher),
            ]),
            Line::from(vec![
                Span::styled("  Source    ", label_style),
                Span::raw(&detail.source),
            ]),
        ];

        if !detail.license.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  License   ", label_style),
                Span::raw(&detail.license),
            ]));
        }

        if !detail.homepage.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("  🌐 ", Style::default().fg(Color::Blue)),
                Span::styled(
                    &detail.homepage,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));
        }

        if !detail.description.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled("  Description", label_style)));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    &detail.description,
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }

        lines.push(Line::raw(""));

        // Show context-appropriate actions
        let mut actions: Vec<Span> = vec![Span::raw("  ")];
        let has_upgrade = app
            .selected_package()
            .is_some_and(|p| !p.available_version.is_empty());
        match app.mode {
            AppMode::Search => {
                actions.push(Span::styled(
                    " ⏎ i ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
                actions.push(Span::raw(" Install "));
            }
            AppMode::Installed => {
                if has_upgrade {
                    actions.push(Span::styled(
                        " u ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ));
                    actions.push(Span::raw(" Upgrade  "));
                }
                actions.push(Span::styled(
                    " ✕ x ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ));
                actions.push(Span::raw(" Uninstall "));
            }
            AppMode::Upgrades => {
                actions.push(Span::styled(
                    " u ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                actions.push(Span::raw(" Upgrade  "));
                actions.push(Span::styled(
                    " ✕ x ",
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ));
                actions.push(Span::raw(" Uninstall "));
            }
        }
        lines.push(Line::from(actions));

        let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
        f.render_widget(p, area);
    } else {
        let msg = if app.filtered_packages.is_empty() {
            "  No package selected".to_string()
        } else if app.loading {
            format!("  {} Loading...", app.spinner())
        } else if app
            .selected_package()
            .is_some_and(|p| p.is_truncated())
        {
            "  ⚠ Package ID is truncated — details unavailable".to_string()
        } else {
            "  Select a package to view details".to_string()
        };
        let p = Paragraph::new(msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(p, area);
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(14), // filter badge
            Constraint::Min(1),    // status message
            Constraint::Length(50), // keyhints
        ])
        .split(area);

    // Filter badge
    let filter_style = match app.source_filter {
        crate::models::SourceFilter::All => Style::default().fg(Color::White).bg(Color::DarkGray),
        crate::models::SourceFilter::Winget => {
            Style::default().fg(Color::Black).bg(Color::Blue)
        }
        crate::models::SourceFilter::MsStore => {
            Style::default().fg(Color::Black).bg(Color::Magenta)
        }
    };
    let filter_badge = Paragraph::new(format!(" ◉ {} ", app.source_filter)).style(filter_style);
    f.render_widget(filter_badge, chunks[0]);

    // Status message with spinner when loading
    let status_text = if app.loading {
        format!(" {} {}", app.spinner(), app.status_message)
    } else {
        format!(" {}", app.status_message)
    };
    let status_style = if app.status_message.contains("failed") || app.status_message.contains("Error") {
        Style::default().fg(Color::Red).bg(Color::DarkGray)
    } else if app.loading {
        Style::default().fg(Color::Yellow).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White).bg(Color::DarkGray)
    };
    let status = Paragraph::new(status_text).style(status_style);
    f.render_widget(status, chunks[1]);

    let keyhints = match app.input_mode {
        InputMode::Search => " Esc: cancel  Enter: search ",
        InputMode::Normal => {
            if app.mode == AppMode::Upgrades {
                if app.selected_packages.is_empty() {
                    " Space: select  ↑↓: nav  /: search  ?: help "
                } else {
                    " Space: select  Shift+U: batch upgrade  ?: help "
                }
            } else {
                " ↑↓: nav  ←→/Tab: view  /: search  f: filter  ?: help "
            }
        }
    };
    let hints = Paragraph::new(keyhints)
        .style(Style::default().fg(Color::Gray).bg(Color::DarkGray))
        .alignment(Alignment::Right);
    f.render_widget(hints, chunks[2]);
}

fn draw_confirm_dialog(f: &mut Frame, confirm: &ConfirmDialog) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" ⚠ Confirm ")
        .title_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::DarkGray));

    let lines = vec![
        Line::raw(""),
        Line::from(vec![Span::raw("  "), Span::raw(&confirm.message)]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " y ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yes   "),
            Span::styled(
                " n ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" No"),
        ]),
    ];

    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(" ❓ Help — Keybindings ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(Color::DarkGray));

    let section = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let key = Style::default().fg(Color::Cyan);

    let help_text = vec![
        Line::raw(""),
        Line::from(Span::styled(" 🧭 Navigation", section)),
        Line::from(vec![
            Span::styled("  ↑/k  ↓/j    ", key),
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
            Span::styled("  ←→/Tab      ", key),
            Span::raw("Cycle views (Shift+Tab: reverse)"),
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
        Line::from(Span::styled(" ⚡ Actions", section)),
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
            Span::raw("Toggle selection (Upgrades view)"),
        ]),
        Line::from(vec![
            Span::styled("  Shift+U     ", key),
            Span::raw("Batch upgrade selected packages"),
        ]),
        Line::from(vec![
            Span::styled("  Enter       ", key),
            Span::raw("Show package details"),
        ]),
        Line::raw(""),
        Line::from(Span::styled(" 🖱 Mouse", section)),
        Line::from(vec![
            Span::styled("  Click       ", key),
            Span::raw("Select tabs, rows, filter"),
        ]),
        Line::from(vec![
            Span::styled("  Scroll      ", key),
            Span::raw("Navigate list"),
        ]),
        Line::raw(""),
        Line::from(Span::styled(" General", section)),
        Line::from(vec![
            Span::styled("  ?           ", key),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc     ", key),
            Span::raw("Quit / Close dialog"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+C      ", key),
            Span::raw("Quit"),
        ]),
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

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 1).collect();
        format!("{truncated}…")
    }
}
