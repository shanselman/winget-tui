use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// ── Theme palette ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub accent: Color,
    pub accent_dim: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_on_accent: Color,
    pub background: Color,
    pub surface: Color,
    pub success: Color,
    pub danger: Color,
    pub info: Color,
    pub selection: Color,
}

impl Theme {
    pub const fn original() -> Self {
        Self {
            accent: Color::Rgb(238, 201, 141),
            accent_dim: Color::Rgb(137, 130, 112),
            text_primary: Color::Rgb(232, 220, 183),
            text_secondary: Color::Rgb(158, 158, 158),
            text_on_accent: Color::Rgb(30, 30, 30),
            background: Color::Rgb(30, 30, 30),
            surface: Color::Rgb(45, 45, 45),
            success: Color::Rgb(86, 185, 127),
            danger: Color::Rgb(231, 72, 86),
            info: Color::Rgb(97, 175, 239),
            selection: Color::Rgb(198, 120, 221),
        }
    }

    pub const fn retro() -> Self {
        Self {
            background: Color::Rgb(5, 18, 8),
            surface: Color::Rgb(10, 30, 15),
            text_primary: Color::Rgb(144, 255, 144),
            text_secondary: Color::Rgb(82, 180, 92),
            accent: Color::Rgb(102, 255, 102),
            accent_dim: Color::Rgb(38, 112, 48),
            text_on_accent: Color::Rgb(5, 18, 8),
            success: Color::Rgb(128, 255, 128),
            danger: Color::Rgb(255, 108, 108),
            info: Color::Rgb(102, 204, 170),
            selection: Color::Rgb(24, 92, 42),
        }
    }

    pub const fn nord() -> Self {
        Self {
            background: Color::Rgb(46, 52, 64),
            surface: Color::Rgb(59, 66, 82),
            text_primary: Color::Rgb(236, 239, 244),
            text_secondary: Color::Rgb(216, 222, 233),
            accent: Color::Rgb(136, 192, 208),
            accent_dim: Color::Rgb(76, 86, 106),
            text_on_accent: Color::Rgb(46, 52, 64),
            success: Color::Rgb(163, 190, 140),
            danger: Color::Rgb(191, 97, 106),
            info: Color::Rgb(129, 161, 193),
            selection: Color::Rgb(67, 76, 94),
        }
    }

    pub const fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Original => Self::original(),
            ThemeName::Retro => Self::retro(),
            ThemeName::Nord => Self::nord(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::original()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Original,
    Retro,
    Nord,
}

impl ThemeName {
    pub fn parse(value: &str) -> Self {
        match value {
            "retro" => Self::Retro,
            "nord" => Self::Nord,
            _ => Self::Original,
        }
    }
}

// ── Style helpers ───────────────────────────────────────────────────────────

/// Style for a focused panel border
pub fn border_focused(theme: &Theme) -> Style {
    Style::default().fg(theme.accent)
}

/// Style for an unfocused panel border
pub fn border_unfocused(theme: &Theme) -> Style {
    Style::default().fg(theme.accent_dim)
}

/// Style for the selected row in the package list
pub fn selected_row(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Style for a multi-select marked row (not currently highlighted)
pub fn marked_row(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.success)
        .add_modifier(Modifier::BOLD)
}

/// Style for table column headers
pub fn table_header(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Style for panel/block titles
pub fn title(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .add_modifier(Modifier::BOLD)
}

/// Style for detail panel labels (Name, ID, Version, etc.)
pub fn detail_label(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Active navbar item
pub fn navbar_active(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Inactive navbar item
pub fn navbar_inactive(theme: &Theme) -> Style {
    Style::default().fg(theme.text_secondary)
}

/// Key hint style (status bar)
#[allow(dead_code)]
pub fn keyhint(theme: &Theme) -> Style {
    Style::default().fg(theme.text_secondary).bg(theme.surface)
}

/// Status bar style for normal messages
pub fn status_normal(theme: &Theme) -> Style {
    Style::default().fg(theme.text_primary).bg(theme.surface)
}

/// Status bar style when loading
pub fn status_loading(theme: &Theme) -> Style {
    Style::default().fg(theme.accent).bg(theme.surface)
}

/// Status bar style on error
pub fn status_error(theme: &Theme) -> Style {
    Style::default().fg(theme.danger).bg(theme.surface)
}

/// Action button: install
pub fn action_install(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .bg(Color::Rgb(189, 63, 57)) // #BD3F39
        .add_modifier(Modifier::BOLD)
}

/// Action button: confirm (yes)
pub fn action_confirm(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.success)
        .add_modifier(Modifier::BOLD)
}

/// Action button: upgrade
#[allow(dead_code)]
pub fn action_upgrade(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Action button key badge (uniform style for all key indicators)
pub fn action_key(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Action button: uninstall / danger
pub fn action_danger(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .bg(theme.danger)
        .add_modifier(Modifier::BOLD)
}

/// Action button: info (open homepage)
#[allow(dead_code)]
pub fn action_info(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.info)
        .add_modifier(Modifier::BOLD)
}

/// Action button: selection (space, select all)
#[allow(dead_code)]
pub fn action_selection(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_on_accent)
        .bg(theme.selection)
        .add_modifier(Modifier::BOLD)
}

/// Help overlay section header
pub fn help_section(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

/// Help overlay key binding text
pub fn help_key(theme: &Theme) -> Style {
    Style::default().fg(theme.info)
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
pub fn logo_lines(theme: &Theme) -> Vec<Line<'static>> {
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

    let color = theme.accent;
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
                (true, true) => spans.push(Span::styled("\u{2588}", Style::default().fg(color))),
                (true, false) => spans.push(Span::styled("\u{2580}", Style::default().fg(color))),
                (false, true) => spans.push(Span::styled("\u{2584}", Style::default().fg(color))),
            }
        }
        lines.push(Line::from(spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_retains_existing_color_values() {
        assert_eq!(
            Theme::original(),
            Theme {
                accent: Color::Rgb(238, 201, 141),
                accent_dim: Color::Rgb(137, 130, 112),
                text_primary: Color::Rgb(232, 220, 183),
                text_secondary: Color::Rgb(158, 158, 158),
                text_on_accent: Color::Rgb(30, 30, 30),
                background: Color::Rgb(30, 30, 30),
                surface: Color::Rgb(45, 45, 45),
                success: Color::Rgb(86, 185, 127),
                danger: Color::Rgb(231, 72, 86),
                info: Color::Rgb(97, 175, 239),
                selection: Color::Rgb(198, 120, 221),
            }
        );
    }

    #[test]
    fn each_preset_produces_a_distinct_palette() {
        let original = Theme::original();
        let retro = Theme::retro();
        let nord = Theme::nord();

        assert_ne!(original, retro);
        assert_ne!(original, nord);
        assert_ne!(retro, nord);
    }
}
