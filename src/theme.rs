use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub background: Color,
    pub surface: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub accent_dim: Color,
    pub success: Color,
    pub error: Color,
    pub info: Color,
    pub selection: Color,
    pub install: Color,
    pub danger: Color,
    pub on_accent: Color,
    pub on_success: Color,
    pub on_info: Color,
    pub on_selection: Color,
    pub on_install: Color,
    pub on_danger: Color,
}

impl Theme {
    pub const fn original() -> Self {
        Self {
            background: Color::Rgb(30, 30, 30),
            surface: Color::Rgb(45, 45, 45),
            text_primary: Color::Rgb(232, 220, 183),
            text_secondary: Color::Rgb(158, 158, 158),
            accent: Color::Rgb(238, 201, 141),
            accent_dim: Color::Rgb(137, 130, 112),
            success: Color::Rgb(86, 185, 127),
            error: Color::Rgb(249, 99, 113),
            info: Color::Rgb(97, 175, 239),
            selection: Color::Rgb(198, 120, 221),
            install: Color::Rgb(189, 63, 57),
            danger: Color::Rgb(249, 99, 113),
            on_accent: Color::Rgb(30, 30, 30),
            on_success: Color::Rgb(30, 30, 30),
            on_info: Color::Rgb(30, 30, 30),
            on_selection: Color::Rgb(30, 30, 30),
            on_install: Color::Rgb(240, 240, 240),
            on_danger: Color::Rgb(30, 30, 30),
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
            success: Color::Rgb(128, 255, 128),
            error: Color::Rgb(255, 108, 108),
            info: Color::Rgb(102, 204, 170),
            selection: Color::Rgb(50, 145, 65),
            install: Color::Rgb(82, 180, 92),
            danger: Color::Rgb(255, 108, 108),
            on_accent: Color::Rgb(5, 18, 8),
            on_success: Color::Rgb(5, 18, 8),
            on_info: Color::Rgb(5, 18, 8),
            on_selection: Color::Rgb(5, 18, 8),
            on_install: Color::Rgb(5, 18, 8),
            on_danger: Color::Rgb(5, 18, 8),
        }
    }

    pub const fn nord() -> Self {
        Self {
            background: Color::Rgb(46, 52, 64),
            surface: Color::Rgb(59, 66, 82),
            text_primary: Color::Rgb(236, 239, 244),
            text_secondary: Color::Rgb(168, 180, 198),
            accent: Color::Rgb(136, 192, 208),
            accent_dim: Color::Rgb(115, 128, 151),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(238, 150, 157),
            info: Color::Rgb(150, 181, 211),
            selection: Color::Rgb(184, 147, 177),
            install: Color::Rgb(235, 203, 139),
            danger: Color::Rgb(238, 150, 157),
            on_accent: Color::Rgb(46, 52, 64),
            on_success: Color::Rgb(46, 52, 64),
            on_info: Color::Rgb(46, 52, 64),
            on_selection: Color::Rgb(46, 52, 64),
            on_install: Color::Rgb(46, 52, 64),
            on_danger: Color::Rgb(46, 52, 64),
        }
    }

    pub const fn terminal() -> Self {
        Self {
            background: Color::Reset,
            surface: Color::Reset,
            text_primary: Color::Reset,
            text_secondary: Color::Reset,
            accent: Color::Cyan,
            accent_dim: Color::DarkGray,
            success: Color::Green,
            error: Color::LightRed,
            info: Color::LightCyan,
            selection: Color::Magenta,
            install: Color::Yellow,
            danger: Color::Red,
            on_accent: Color::Black,
            on_success: Color::Black,
            on_info: Color::Black,
            on_selection: Color::White,
            on_install: Color::Black,
            on_danger: Color::White,
        }
    }

    pub const fn from_name(name: ThemeName) -> Self {
        match name {
            ThemeName::Original => Self::original(),
            ThemeName::Retro => Self::retro(),
            ThemeName::Nord => Self::nord(),
            ThemeName::Terminal => Self::terminal(),
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
    Terminal,
}

impl ThemeName {
    pub fn parse(value: &str) -> Self {
        if value.eq_ignore_ascii_case("retro") {
            Self::Retro
        } else if value.eq_ignore_ascii_case("nord") {
            Self::Nord
        } else if value.eq_ignore_ascii_case("terminal") || value.eq_ignore_ascii_case("system") {
            Self::Terminal
        } else {
            Self::Original
        }
    }
}

pub fn root(theme: &Theme) -> Style {
    Style::default().fg(theme.text_primary).bg(theme.background)
}

pub fn surface(theme: &Theme) -> Style {
    Style::default().fg(theme.text_primary).bg(theme.surface)
}

pub fn secondary(theme: &Theme) -> Style {
    let style = Style::default()
        .fg(theme.text_secondary)
        .bg(theme.background);
    if theme.text_secondary == Color::Reset {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    }
}

pub fn surface_secondary(theme: &Theme) -> Style {
    let style = Style::default().fg(theme.text_secondary).bg(theme.surface);
    if theme.text_secondary == Color::Reset {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    }
}

pub fn success_text(theme: &Theme) -> Style {
    Style::default().fg(theme.success).bg(theme.background)
}

pub fn info_text(theme: &Theme) -> Style {
    Style::default().fg(theme.info).bg(theme.background)
}

pub fn border_focused(theme: &Theme) -> Style {
    Style::default().fg(theme.accent).bg(theme.background)
}

pub fn border_unfocused(theme: &Theme) -> Style {
    Style::default().fg(theme.accent_dim).bg(theme.background)
}

pub fn selected_row(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

pub fn marked_row(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.success)
        .bg(theme.background)
        .add_modifier(Modifier::BOLD)
}

pub fn table_header(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .bg(theme.background)
        .add_modifier(Modifier::BOLD)
}

pub fn title(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .bg(theme.background)
        .add_modifier(Modifier::BOLD)
}

pub fn detail_label(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .bg(theme.background)
        .add_modifier(Modifier::BOLD)
}

pub fn navbar_active(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

pub fn navbar_inactive(theme: &Theme) -> Style {
    secondary(theme)
}

pub fn status_normal(theme: &Theme) -> Style {
    surface(theme)
}

pub fn status_loading(theme: &Theme) -> Style {
    Style::default().fg(theme.accent).bg(theme.surface)
}

pub fn status_error(theme: &Theme) -> Style {
    Style::default().fg(theme.error).bg(theme.surface)
}

pub fn action_install(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_install)
        .bg(theme.install)
        .add_modifier(Modifier::BOLD)
}

pub fn action_confirm(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_success)
        .bg(theme.success)
        .add_modifier(Modifier::BOLD)
}

pub fn action_key(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_accent)
        .bg(theme.accent)
        .add_modifier(Modifier::BOLD)
}

pub fn action_danger(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_danger)
        .bg(theme.danger)
        .add_modifier(Modifier::BOLD)
}

pub fn source_winget(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_info)
        .bg(theme.info)
        .add_modifier(Modifier::BOLD)
}

pub fn source_msstore(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.on_selection)
        .bg(theme.selection)
        .add_modifier(Modifier::BOLD)
}

pub fn help_section(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.accent)
        .bg(theme.surface)
        .add_modifier(Modifier::BOLD)
}

pub fn help_key(theme: &Theme) -> Style {
    Style::default().fg(theme.info).bg(theme.surface)
}

pub const LOGO_HEIGHT: u16 = 3;

pub fn logo_lines(theme: &Theme) -> Vec<Line<'static>> {
    #[rustfmt::skip]
    const GRID: [[u8; 31]; 6] = [
        [1,0,0,0,1, 0,1, 0,1,0,0,1, 0,0,1,1,1, 0,1,1,1, 0,1,1,1, 0,0,0,0,0,0],
        [1,0,0,0,1, 0,1, 0,1,1,0,1, 0,1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,0,0,0],
        [1,0,1,0,1, 0,1, 0,1,0,1,1, 0,1,0,1,1, 0,1,1,0, 0,0,1,0, 0,0,0,0,0,0],
        [1,0,1,0,1, 0,1, 0,1,0,0,1, 0,1,0,0,1, 0,1,0,0, 0,0,1,0, 0,0,0,0,0,0],
        [0,1,0,1,0, 0,1, 0,1,0,0,1, 0,0,1,1,1, 0,1,1,1, 0,0,1,0, 0,0,0,0,0,0],
        [0,0,0,0,0, 0,0, 0,0,0,0,0, 0,0,0,0,0, 0,0,0,0, 0,0,0,0, 0,0,0,0,0,0],
    ];

    let mut lines = Vec::new();
    for text_row in 0..3 {
        let top = &GRID[text_row * 2];
        let bottom = &GRID[text_row * 2 + 1];
        let mut spans = Vec::new();

        for column in 0..31 {
            match (top[column] == 1, bottom[column] == 1) {
                (false, false) => spans.push(Span::raw(" ")),
                (true, true) => spans.push(Span::styled(
                    "\u{2588}",
                    Style::default().fg(theme.accent).bg(theme.background),
                )),
                (true, false) => spans.push(Span::styled(
                    "\u{2580}",
                    Style::default().fg(theme.accent).bg(theme.background),
                )),
                (false, true) => spans.push(Span::styled(
                    "\u{2584}",
                    Style::default().fg(theme.accent).bg(theme.background),
                )),
            }
        }
        lines.push(Line::from(spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_TEXT_CONTRAST: f64 = 4.5;
    const MIN_NON_TEXT_CONTRAST: f64 = 3.0;

    fn rgb(color: Color) -> Option<(u8, u8, u8)> {
        match color {
            Color::Rgb(red, green, blue) => Some((red, green, blue)),
            _ => None,
        }
    }

    fn relative_luminance(color: Color) -> Option<f64> {
        let (red, green, blue) = rgb(color)?;
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue))
    }

    fn contrast(foreground: Color, background: Color) -> Option<f64> {
        let foreground = relative_luminance(foreground)?;
        let background = relative_luminance(background)?;
        let (lighter, darker) = if foreground > background {
            (foreground, background)
        } else {
            (background, foreground)
        };
        Some((lighter + 0.05) / (darker + 0.05))
    }

    fn assert_contrast(
        theme_name: &str,
        pair_name: &str,
        foreground: Color,
        background: Color,
        minimum: f64,
    ) {
        let Some(ratio) = contrast(foreground, background) else {
            return;
        };
        assert!(
            ratio >= minimum,
            "{theme_name} {pair_name} contrast {ratio:.2}:1 is below {minimum:.1}:1"
        );
    }

    #[test]
    fn theme_names_are_case_insensitive() {
        assert_eq!(ThemeName::parse("ORIGINAL"), ThemeName::Original);
        assert_eq!(ThemeName::parse("ReTrO"), ThemeName::Retro);
        assert_eq!(ThemeName::parse("NORD"), ThemeName::Nord);
        assert_eq!(ThemeName::parse("terminal"), ThemeName::Terminal);
        assert_eq!(ThemeName::parse("SYSTEM"), ThemeName::Terminal);
        assert_eq!(ThemeName::parse("unknown"), ThemeName::Original);
    }

    #[test]
    fn from_name_maps_each_variant_to_matching_theme() {
        assert_eq!(Theme::from_name(ThemeName::Original), Theme::original());
        assert_eq!(Theme::from_name(ThemeName::Retro), Theme::retro());
        assert_eq!(Theme::from_name(ThemeName::Nord), Theme::nord());
        assert_eq!(Theme::from_name(ThemeName::Terminal), Theme::terminal());
    }

    #[test]
    fn rendered_semantic_pairs_meet_contrast_floors() {
        for (name, theme) in [
            ("original", Theme::original()),
            ("retro", Theme::retro()),
            ("nord", Theme::nord()),
        ] {
            for (pair, foreground, background) in [
                ("root text", theme.text_primary, theme.background),
                (
                    "root secondary text",
                    theme.text_secondary,
                    theme.background,
                ),
                ("surface text", theme.text_primary, theme.surface),
                (
                    "surface secondary text",
                    theme.text_secondary,
                    theme.surface,
                ),
                ("accent text", theme.accent, theme.background),
                ("surface accent text", theme.accent, theme.surface),
                ("success text", theme.success, theme.background),
                ("error text", theme.error, theme.surface),
                ("info text", theme.info, theme.background),
                ("surface info text", theme.info, theme.surface),
                ("selected row", theme.on_accent, theme.accent),
                ("confirm action", theme.on_success, theme.success),
                ("Winget badge", theme.on_info, theme.info),
                ("MsStore badge", theme.on_selection, theme.selection),
                ("install action", theme.on_install, theme.install),
                ("danger action", theme.on_danger, theme.danger),
            ] {
                assert_contrast(name, pair, foreground, background, MIN_TEXT_CONTRAST);
            }

            for (pair, foreground, background) in [
                ("focused border", theme.accent, theme.background),
                ("unfocused border", theme.accent_dim, theme.background),
            ] {
                assert_contrast(name, pair, foreground, background, MIN_NON_TEXT_CONTRAST);
            }

            assert_contrast(
                name,
                "primary/secondary role separation",
                theme.text_primary,
                theme.text_secondary,
                1.5,
            );
        }
    }

    #[test]
    fn terminal_theme_inherits_terminal_foreground_and_background() {
        let theme = Theme::terminal();
        assert_eq!(theme.background, Color::Reset);
        assert_eq!(theme.surface, Color::Reset);
        assert_eq!(theme.text_primary, Color::Reset);
        assert_eq!(theme.text_secondary, Color::Reset);
        assert!(secondary(&theme).add_modifier.contains(Modifier::DIM));
        assert!(surface_secondary(&theme)
            .add_modifier
            .contains(Modifier::DIM));
    }

    #[test]
    fn original_preserves_existing_colors_except_accessibility_exceptions() {
        let theme = Theme::original();
        assert_eq!(theme.background, Color::Rgb(30, 30, 30));
        assert_eq!(theme.surface, Color::Rgb(45, 45, 45));
        assert_eq!(theme.text_primary, Color::Rgb(232, 220, 183));
        assert_eq!(theme.text_secondary, Color::Rgb(158, 158, 158));
        assert_eq!(theme.accent, Color::Rgb(238, 201, 141));
        assert_eq!(theme.accent_dim, Color::Rgb(137, 130, 112));
        assert_eq!(theme.success, Color::Rgb(86, 185, 127));
        assert_eq!(theme.info, Color::Rgb(97, 175, 239));
        assert_eq!(theme.selection, Color::Rgb(198, 120, 221));
        assert_eq!(theme.install, Color::Rgb(189, 63, 57));
        assert_eq!(theme.error, Color::Rgb(249, 99, 113));
        assert_eq!(theme.danger, Color::Rgb(249, 99, 113));
        assert_eq!(theme.on_accent, Color::Rgb(30, 30, 30));
        assert_eq!(theme.on_success, Color::Rgb(30, 30, 30));
        assert_eq!(theme.on_info, Color::Rgb(30, 30, 30));
        assert_eq!(theme.on_selection, Color::Rgb(30, 30, 30));
        assert_eq!(theme.on_install, Color::Rgb(240, 240, 240));
        assert_eq!(theme.on_danger, Color::Rgb(30, 30, 30));
    }

    #[test]
    fn root_and_surface_use_expected_fg_and_bg() {
        let theme = Theme::original();
        assert_eq!(
            root(&theme),
            Style::default().fg(theme.text_primary).bg(theme.background)
        );
        assert_eq!(
            surface(&theme),
            Style::default().fg(theme.text_primary).bg(theme.surface)
        );
    }

    #[test]
    fn secondary_styles_use_text_secondary_and_dim_only_for_reset() {
        let original = Theme::original();
        let secondary_style = secondary(&original);
        assert_eq!(secondary_style.fg, Some(original.text_secondary));
        assert_eq!(secondary_style.bg, Some(original.background));
        assert!(!secondary_style.add_modifier.contains(Modifier::DIM));

        let terminal = Theme::terminal();
        assert!(secondary(&terminal).add_modifier.contains(Modifier::DIM));
        assert!(surface_secondary(&terminal)
            .add_modifier
            .contains(Modifier::DIM));
    }

    #[test]
    fn border_styles_use_accent_colors_on_background() {
        let theme = Theme::original();
        assert_eq!(
            border_focused(&theme),
            Style::default().fg(theme.accent).bg(theme.background)
        );
        assert_eq!(
            border_unfocused(&theme),
            Style::default().fg(theme.accent_dim).bg(theme.background)
        );
    }

    #[test]
    fn selected_and_marked_row_styles_are_bold() {
        let theme = Theme::original();
        let selected = selected_row(&theme);
        assert_eq!(selected.fg, Some(theme.on_accent));
        assert_eq!(selected.bg, Some(theme.accent));
        assert!(selected.add_modifier.contains(Modifier::BOLD));

        let marked = marked_row(&theme);
        assert_eq!(marked.fg, Some(theme.success));
        assert_eq!(marked.bg, Some(theme.background));
        assert!(marked.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn table_header_and_title_styles_are_bold_accent() {
        let theme = Theme::original();
        let header = table_header(&theme);
        assert_eq!(header.fg, Some(theme.accent));
        assert_eq!(header.bg, Some(theme.background));
        assert!(header.add_modifier.contains(Modifier::BOLD));

        let title_style = title(&theme);
        assert_eq!(title_style.fg, Some(theme.text_primary));
        assert_eq!(title_style.bg, Some(theme.background));
        assert!(title_style.add_modifier.contains(Modifier::BOLD));

        let label = detail_label(&theme);
        assert_eq!(label.fg, Some(theme.accent));
        assert_eq!(label.bg, Some(theme.background));
        assert!(label.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn navbar_styles_reflect_active_and_inactive_state() {
        let theme = Theme::original();
        let active = navbar_active(&theme);
        assert_eq!(active.fg, Some(theme.on_accent));
        assert_eq!(active.bg, Some(theme.accent));
        assert!(active.add_modifier.contains(Modifier::BOLD));

        assert_eq!(navbar_inactive(&theme), secondary(&theme));
    }

    #[test]
    fn status_styles_use_surface_background_with_semantic_fg() {
        let theme = Theme::original();
        assert_eq!(status_normal(&theme), surface(&theme));
        assert_eq!(
            status_loading(&theme),
            Style::default().fg(theme.accent).bg(theme.surface)
        );
        assert_eq!(
            status_error(&theme),
            Style::default().fg(theme.error).bg(theme.surface)
        );
    }

    #[test]
    fn action_styles_pair_on_colors_with_semantic_backgrounds() {
        let theme = Theme::original();
        for (style, fg, bg) in [
            (action_install(&theme), theme.on_install, theme.install),
            (action_confirm(&theme), theme.on_success, theme.success),
            (action_key(&theme), theme.on_accent, theme.accent),
            (action_danger(&theme), theme.on_danger, theme.danger),
        ] {
            assert_eq!(style.fg, Some(fg));
            assert_eq!(style.bg, Some(bg));
            assert!(style.add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn source_badge_styles_pair_on_colors_with_source_backgrounds() {
        let theme = Theme::original();
        let winget = source_winget(&theme);
        assert_eq!(winget.fg, Some(theme.on_info));
        assert_eq!(winget.bg, Some(theme.info));
        assert!(winget.add_modifier.contains(Modifier::BOLD));

        let msstore = source_msstore(&theme);
        assert_eq!(msstore.fg, Some(theme.on_selection));
        assert_eq!(msstore.bg, Some(theme.selection));
        assert!(msstore.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn help_styles_use_surface_background() {
        let theme = Theme::original();
        let section = help_section(&theme);
        assert_eq!(section.fg, Some(theme.accent));
        assert_eq!(section.bg, Some(theme.surface));
        assert!(section.add_modifier.contains(Modifier::BOLD));

        let key = help_key(&theme);
        assert_eq!(key.fg, Some(theme.info));
        assert_eq!(key.bg, Some(theme.surface));
    }

    #[test]
    fn logo_lines_renders_three_lines_of_31_columns() {
        let theme = Theme::original();
        let lines = logo_lines(&theme);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert_eq!(line.spans.len(), 31);
        }
    }
}
