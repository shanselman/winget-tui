use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// ── Winget-inspired color palette ───────────────────────────────────────────

/// Primary accent
pub const ACCENT: Color = Color::Rgb(238, 201, 141); // #EEC98D

/// Dimmed accent — for focused borders
pub const ACCENT_DIM: Color = Color::Rgb(137, 130, 112); // #898270

/// Dark warm brown — for unfocused borders and subtle accents
#[allow(dead_code)]
pub const ACCENT_DARK: Color = Color::Rgb(137, 130, 112); // #898270

/// Primary text color
pub const TEXT_PRIMARY: Color = Color::Rgb(232, 220, 183); // #E8DCB7

/// Secondary/dimmed text
pub const TEXT_SECONDARY: Color = Color::Rgb(158, 158, 158); // #9E9E9E

/// Text rendered on top of accent backgrounds
pub const TEXT_ON_ACCENT: Color = Color::Rgb(30, 30, 30); // #1E1E1E

/// Panel/surface background
pub const SURFACE: Color = Color::Rgb(45, 45, 45); // #2D2D2D

/// App background
#[allow(dead_code)]
pub const BG: Color = Color::Rgb(30, 30, 30); // #1E1E1E

/// Success (install, available version, updates)
pub const SUCCESS: Color = Color::Rgb(86, 185, 127); // #56B97F

/// Danger (uninstall, errors)
pub const DANGER: Color = Color::Rgb(231, 72, 86); // #E74856

/// Info (links, IDs)
pub const INFO: Color = Color::Rgb(97, 175, 239); // #61AFEF

/// Selection highlight (multi-select markers in upgrades)
pub const SELECTION: Color = Color::Rgb(198, 120, 221); // #C678DD

// ── Style helpers ───────────────────────────────────────────────────────────

/// Style for a focused panel border
pub fn border_focused() -> Style {
    Style::default().fg(ACCENT)
}

/// Style for an unfocused panel border
pub fn border_unfocused() -> Style {
    Style::default().fg(ACCENT_DIM)
}

/// Style for the selected row in the package list
pub fn selected_row() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Style for a multi-select marked row (not currently highlighted)
pub fn marked_row() -> Style {
    Style::default()
        .fg(SUCCESS)
        .add_modifier(Modifier::BOLD)
}

/// Style for table column headers
pub fn table_header() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Style for panel/block titles
pub fn title() -> Style {
    Style::default()
        .fg(TEXT_PRIMARY)
        .add_modifier(Modifier::BOLD)
}

/// Style for detail panel labels (Name, ID, Version, etc.)
pub fn detail_label() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Active navbar item
pub fn navbar_active() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Inactive navbar item
pub fn navbar_inactive() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

/// Key hint style (status bar)
#[allow(dead_code)]
pub fn keyhint() -> Style {
    Style::default().fg(TEXT_SECONDARY).bg(SURFACE)
}

/// Status bar style for normal messages
pub fn status_normal() -> Style {
    Style::default().fg(TEXT_PRIMARY).bg(SURFACE)
}

/// Status bar style when loading
pub fn status_loading() -> Style {
    Style::default().fg(ACCENT).bg(SURFACE)
}

/// Status bar style on error
pub fn status_error() -> Style {
    Style::default().fg(DANGER).bg(SURFACE)
}

/// Action button: install
pub fn action_install() -> Style {
    Style::default()
        .fg(TEXT_PRIMARY)
        .bg(Color::Rgb(189, 63, 57)) // #BD3F39
        .add_modifier(Modifier::BOLD)
}

/// Action button: confirm (yes)
pub fn action_confirm() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(SUCCESS)
        .add_modifier(Modifier::BOLD)
}

/// Action button: upgrade
#[allow(dead_code)]
pub fn action_upgrade() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Action button key badge (uniform style for all key indicators)
pub fn action_key() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Action button: uninstall / danger
pub fn action_danger() -> Style {
    Style::default()
        .fg(TEXT_PRIMARY)
        .bg(DANGER)
        .add_modifier(Modifier::BOLD)
}

/// Action button: info (open homepage)
#[allow(dead_code)]
pub fn action_info() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(INFO)
        .add_modifier(Modifier::BOLD)
}

/// Action button: selection (space, select all)
#[allow(dead_code)]
pub fn action_selection() -> Style {
    Style::default()
        .fg(TEXT_ON_ACCENT)
        .bg(SELECTION)
        .add_modifier(Modifier::BOLD)
}

/// Help overlay section header
pub fn help_section() -> Style {
    Style::default()
        .fg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// Help overlay key binding text
pub fn help_key() -> Style {
    Style::default().fg(INFO)
}

// ── Winget Icon (half-block pixel art) ───────────────────────────────────────

// Icon colors from the SVG (kept for potential future use)
#[allow(dead_code)]
const ICON_BROWN: Color = Color::Rgb(156, 100, 10); // #9C640A back card
#[allow(dead_code)]
const ICON_AMBER: Color = Color::Rgb(188, 130, 42); // #BC822A mid card
#[allow(dead_code)]
const ICON_GOLD: Color = Color::Rgb(222, 182, 120); // #DEB678 front card
#[allow(dead_code)]
const ICON_ARROW: Color = Color::Rgb(240, 240, 240); // #F0F0F0 arrow

/// Height of the logo in text rows
pub const LOGO_HEIGHT: u16 = 3;

/// Render "winget" as pixel word art using half-blocks.
/// 3 text rows tall (6 pixel rows), rendered in the accent color.
pub fn logo_lines() -> Vec<Line<'static>> {
    // Letters designed on a 5x6 grid (or narrower), 1px gap between each.
    //
    //  w         i     n         g         e         t
    //  #   #     #     #   #     ###       ###      ###
    //  #   #     #     ##  #     #         #         #
    //  # # #     #     # # #     # ##      ##        #
    //  # # #     #     #  ##     #  #      #         #
    //  ## ##     #     #   #     ###       ###       #
    //
    #[rustfmt::skip]
    const GRID: [[u8; 31]; 6] = [
      // w . . . .   i   n . . . .   g . . . .   e . . .   t . .
        [1,0,0,0,1, 0,1, 0,1,0,0,1, 0,0,1,1,1, 0,1,1,1, 0,1,1,1, 0,0,0,0,0,0],
        [1,0,0,0,1, 0,1, 0,1,1,0,1, 0,1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,0,0,0],
        [1,0,1,0,1, 0,1, 0,1,0,1,1, 0,1,0,1,1, 0,1,1,0, 0,0,1,0, 0,0,0,0,0,0],
        [1,0,1,0,1, 0,1, 0,1,0,0,1, 0,1,0,0,1, 0,1,0,0, 0,0,1,0, 0,0,0,0,0,0],
        [0,1,0,1,0, 0,1, 0,1,0,0,1, 0,0,1,1,1, 0,1,1,1, 0,0,1,0, 0,0,0,0,0,0],
        [0,0,0,0,0, 0,0, 0,0,0,0,0, 0,0,0,0,0, 0,0,0,0, 0,0,0,0, 0,0,0,0,0,0],
    ];

    let color = ACCENT;
    let mut lines = Vec::new();

    for text_row in 0..3 {
        let top = &GRID[text_row * 2];
        let bot = &GRID[text_row * 2 + 1];
        let mut spans = Vec::new();

        for col in 0..31 {
            let t = top[col] == 1;
            let b = bot[col] == 1;
            match (t, b) {
                (false, false) => spans.push(Span::raw(" ")),
                (true, true) => {
                    spans.push(Span::styled("\u{2588}", Style::default().fg(color)))
                }
                (true, false) => {
                    spans.push(Span::styled("\u{2580}", Style::default().fg(color)))
                }
                (false, true) => {
                    spans.push(Span::styled("\u{2584}", Style::default().fg(color)))
                }
            }
        }
        lines.push(Line::from(spans));
    }

    lines
}
