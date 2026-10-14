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
            table_alt: Style::new().add_modifier(Modifier::DIM),
            selected: Style::new().add_modifier(Modifier::REVERSED),
            border: Style::new().fg(Color::DarkGray),
            border_focused: Style::new().fg(Color::Yellow),
        }
    }
}
