use ratatui::style::{Color, Modifier, Style};

/// Colours used across the UI. Uses the terminal's ANSI palette so it follows
/// the user's terminal theme.
pub struct Theme {
    pub accent: Color,
    pub dim: Style,
    pub heading: Style,
    pub subheading: Style,
    pub link: Style,
    pub link_focused: Style,
    pub mechanic: Color,
    pub code: Color,
    pub inset_border: Style,
    pub table_header: Style,
    pub table_alt: Style,
    pub selected: Style,
    pub border: Style,
    pub border_focused: Style,
    pub bar: Style,
    pub brand: Style,
    pub tab_active: Style,
    pub tab_inactive: Style,
    pub category_active: Style,
    pub category_inactive: Style,
    pub facet_source: Color,
    pub facet_tag: Color,
    pub facet_damage: Color,
    pub facet_level: Color,
    pub facet_text: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            accent: Color::Yellow,
            dim: Style::new().fg(Color::DarkGray),
            heading: Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            subheading: Style::new().add_modifier(Modifier::BOLD),
            link: Style::new().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            link_focused: Style::new().fg(Color::Black).bg(Color::Cyan),
            mechanic: Color::Magenta,
            code: Color::Green,
            inset_border: Style::new().fg(Color::Blue),
            table_header: Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            table_alt: Style::new(),
            selected: Style::new().add_modifier(Modifier::REVERSED),
            border: Style::new().fg(Color::DarkGray),
            border_focused: Style::new().fg(Color::Yellow),
            bar: Style::new(),
            brand: Style::new().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
            tab_active: Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            tab_inactive: Style::new().fg(Color::Gray),
            category_active: Style::new().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            category_inactive: Style::new().fg(Color::Gray),
            facet_source: Color::Blue,
            facet_tag: Color::Green,
            facet_damage: Color::Red,
            facet_level: Color::Magenta,
            facet_text: Color::Yellow,
        }
    }
}
