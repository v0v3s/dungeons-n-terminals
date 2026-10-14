use dnt_data::Library;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::compendium::{Compendium, Focus};
use crate::render;
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, lib: &Library, c: &mut Compendium, status: &str, theme: &Theme) {
    let [main, footer] = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());
    let list_w = (main.width / 3).clamp(28, 48);
    let [left, right] = Layout::horizontal([Constraint::Length(list_w), Constraint::Min(20)]).areas(main);
    let [search, list] = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(left);

    draw_search(frame, search, c, theme);
    draw_list(frame, list, lib, c, theme);
    draw_detail(frame, right, lib, c, theme);
    draw_footer(frame, footer, c, status, theme);
}

fn pane(title: Line<'static>, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused { theme.border_focused } else { theme.border })
        .title(title)
}

fn draw_search(frame: &mut Frame, area: Rect, c: &Compendium, theme: &Theme) {
    let focused = c.focus == Focus::Search;
    let title = Line::from(vec![
        Span::raw(" Search "),
        Span::styled(format!("[{}] ", c.category.label()), Style::new().fg(theme.accent)),
    ]);
    let block = pane(title, focused, theme);
    let inner = block.inner(area);
    let text = if c.query.is_empty() && !focused {
        Line::styled("press / to search", theme.dim)
    } else {
        Line::from(c.query.clone())
    };
    frame.render_widget(Paragraph::new(text).block(block), area);
    if focused {
        let x = inner.x + (c.query.width() as u16).min(inner.width.saturating_sub(1));
        frame.set_cursor_position((x, inner.y));
    }
}

fn draw_list(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let title = Line::from(format!(" {} results ", c.results.len()));
    let block = pane(title, c.focus == Focus::List, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let height = inner.height as usize;
    c.list_height = height;
    if c.selected < c.list_offset {
        c.list_offset = c.selected;
    } else if c.selected >= c.list_offset + height {
        c.list_offset = c.selected + 1 - height;
    }

    let width = inner.width as usize;
    let lines: Vec<Line> = c
        .results
        .iter()
        .enumerate()
        .skip(c.list_offset)
        .take(height)
        .map(|(i, &id)| {
            let e = lib.entity(id);
            let meta = format!(" {} {}", short_kind(e.kind), e.source);
            let name_w = width.saturating_sub(meta.width());
            let name = truncate(&e.name, name_w);
            let pad = name_w.saturating_sub(name.width());
            let mut line = Line::from(vec![
                Span::raw(name),
                Span::raw(" ".repeat(pad)),
                Span::styled(meta, theme.dim),
            ]);
            if i == c.selected {
                line = line.style(theme.selected);
            }
            line
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_detail(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let Some(detail) = c.detail.as_mut() else {
        let block = pane(Line::from(" Compendium "), false, theme);
        frame.render_widget(Paragraph::new(Line::styled("No results.", theme.dim)).block(block), area);
        c.detail_links.clear();
        return;
    };
    let e = lib.entity(detail.entity);
    let source = lib.source_info(&e.source).map_or(e.source.as_str(), |s| s.name.as_str());
    let mut title = vec![
        Span::styled(format!(" {} ", e.name), Style::new().add_modifier(Modifier::BOLD)),
        Span::styled(format!("{} · {source} ", e.kind.label()), theme.dim),
    ];
    if !c.history.is_empty() {
        title.push(Span::styled(format!("← {} ", c.history.len()), Style::new().fg(theme.accent)));
    }
    let block = pane(Line::from(title), c.focus == Focus::Detail, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Leave a column for the scrollbar and a one-column margin.
    let text_area = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(3),
        ..inner
    };
    let focused = if c.focus == Focus::Detail { detail.focused_link } else { None };
    let rendered = render::render(&detail.doc, text_area.width, focused, theme);
    c.detail_height = text_area.height as usize;
    c.detail_lines = rendered.lines.len();
    detail.scroll = detail.scroll.min(c.detail_lines.saturating_sub(c.detail_height));
    c.detail_links = rendered.links;

    let scroll = detail.scroll;
    let visible: Vec<Line> = rendered.lines.into_iter().skip(scroll).take(c.detail_height).collect();
    frame.render_widget(Paragraph::new(visible), text_area);

    if c.detail_lines > c.detail_height {
        let mut state = ScrollbarState::new(c.detail_lines.saturating_sub(c.detail_height)).position(scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight).style(theme.dim),
            inner,
            &mut state,
        );
    }
}

fn draw_footer(frame: &mut Frame, area: Rect, c: &Compendium, status: &str, theme: &Theme) {
    let hints: &[(&str, &str)] = match c.focus {
        Focus::Search => &[("type", "filter"), ("↑↓/^n^p", "move"), ("enter", "open"), ("tab", "category"), ("esc", "list")],
        Focus::List => &[("j/k", "move"), ("enter", "open"), ("/", "search"), ("tab", "category"), ("q", "quit")],
        Focus::Detail => &[("j/k", "scroll"), ("tab/n", "next link"), ("enter", "follow"), ("bksp", "back"), ("esc", "list")],
    };
    let mut spans = Vec::new();
    for (key, what) in hints {
        spans.push(Span::styled(format!(" {key} "), Style::new().fg(theme.accent)));
        spans.push(Span::styled(format!("{what} "), theme.dim));
    }
    if !status.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(status.to_string(), Style::new().fg(theme.accent)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn short_kind(kind: dnt_data::Kind) -> &'static str {
    use dnt_data::Kind::*;
    match kind {
        Class => "class",
        Subclass => "subcl",
        ClassFeature | SubclassFeature => "feat.",
        Spell => "spell",
        Race | Subrace => "spec.",
        Background => "bg",
        Feat => "feat",
        OptionalFeature => "opt.",
        Item | BaseItem | ItemGroup | MagicVariant => "item",
        ItemMastery => "mast.",
        Condition | Disease | Status => "cond.",
        Language => "lang",
        Skill => "skill",
        Sense => "sense",
        Action => "action",
        VariantRule => "rule",
    }
}

fn truncate(s: &str, width: usize) -> String {
    if s.width() <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for ch in s.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw + 1 > width {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('…');
    out
}
