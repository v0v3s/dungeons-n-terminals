use dnt_data::Library;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Section};
use crate::compendium::{Category, Compendium, Focus};
use crate::render;
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, lib: &Library, app: &mut App, theme: &Theme) {
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(1)]).areas(frame.area());

    draw_header(frame, header, app, theme);
    match app.section {
        Section::Compendium => draw_compendium(frame, body, lib, &mut app.compendium, theme),
        Section::Characters => draw_placeholder(
            frame,
            body,
            "Characters",
            "M3",
            &[
                "Character roster for the active campaign",
                "Creation wizard: species, class, background, ability scores, skills, equipment, spells",
                "Sheet tabs: Overview / Combat / Spells / Inventory / Features / Notes",
                "HP, hit dice, spell slots, class resources, conditions",
                "Short and long rests; breakdown and override for every stat",
            ],
            theme,
        ),
        Section::Notes => draw_placeholder(
            frame,
            body,
            "Notes",
            "M4",
            &[
                "Campaign-scoped Markdown notes in a file tree",
                "Vim-style editor",
                "[[Note]] and [[spell:Fireball]] links, followed with gd",
                "Session journal and quick capture",
                "Tags and full-text search",
            ],
            theme,
        ),
    }
    draw_footer(frame, footer, app, theme);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut spans = vec![Span::styled(" dnt ", theme.brand), Span::raw(" ")];
    for (i, s) in Section::ALL.iter().enumerate() {
        let label = format!(" {} {} ", i + 1, s.label());
        spans.push(if *s == app.section {
            Span::styled(label, theme.tab_active)
        } else {
            Span::styled(label, theme.tab_inactive)
        });
    }
    let sources = match &app.compendium.filter.enabled {
        None => "all sources".to_string(),
        Some(set) => {
            let mut v: Vec<String> = set.iter().map(|s| s.to_uppercase()).collect();
            v.sort();
            v.join(" ")
        }
    };
    let right = format!(" {sources} ");
    let used: usize = spans.iter().map(|s| s.content.width()).sum();
    let pad = (area.width as usize).saturating_sub(used + right.width());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(right, theme.dim));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.bar), area);
}

fn pane(title: Line<'static>, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused { theme.border_focused } else { theme.border })
        .title(title)
}

fn draw_compendium(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let [tabs, main] = Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(area);
    let list_w = (main.width / 3).clamp(30, 52);
    let [left, right] = Layout::horizontal([Constraint::Length(list_w), Constraint::Min(20)]).areas(main);
    let [search, list] = Layout::vertical([Constraint::Length(5), Constraint::Min(1)]).areas(left);

    draw_categories(frame, tabs, c, theme);
    draw_search(frame, search, c, theme);
    draw_list(frame, list, lib, c, theme);
    draw_detail(frame, right, lib, c, theme);
}

fn draw_categories(frame: &mut Frame, area: Rect, c: &Compendium, theme: &Theme) {
    let mut spans = vec![Span::raw(" ")];
    for cat in Category::ORDER {
        let label = format!(" {} ", cat.label());
        spans.push(if cat == c.category {
            Span::styled(label, theme.category_active)
        } else {
            Span::styled(label, theme.category_inactive)
        });
    }
    spans.push(Span::styled("   tab ⇄", theme.dim));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The query with each term coloured by its role.
fn query_line(query: &str, theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    let mut in_quote = false;
    let mut token = String::new();
    let flush = |token: &mut String, spans: &mut Vec<Span<'static>>| {
        if token.is_empty() {
            return;
        }
        let body = token.trim_start_matches('!');
        let style = match body.chars().next() {
            Some('@') => Style::new().fg(theme.facet_source),
            Some('#') => Style::new().fg(theme.facet_tag),
            Some('*') => Style::new().fg(theme.facet_damage),
            Some('^') => Style::new().fg(theme.facet_level),
            Some('"') => Style::new().fg(theme.facet_text),
            _ => Style::new(),
        };
        let style = if token.starts_with('!') { style.add_modifier(Modifier::CROSSED_OUT) } else { style };
        spans.push(Span::styled(std::mem::take(token), style));
    };
    for ch in query.chars() {
        if ch == '"' {
            in_quote = !in_quote;
        }
        if ch == ' ' && !in_quote {
            flush(&mut token, &mut spans);
            spans.push(Span::raw(" "));
        } else {
            token.push(ch);
        }
    }
    flush(&mut token, &mut spans);
    Line::from(spans)
}

fn draw_search(frame: &mut Frame, area: Rect, c: &Compendium, theme: &Theme) {
    let focused = c.focus == Focus::Search;
    let block = pane(Line::from(" Search "), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [input, hint] = Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).areas(inner);

    let text = if c.query.is_empty() && !focused {
        Line::styled("/ to search", theme.dim)
    } else {
        query_line(&c.query, theme)
    };
    frame.render_widget(Paragraph::new(text), input);
    if focused {
        let x = input.x + (c.query.width() as u16).min(input.width.saturating_sub(1));
        frame.set_cursor_position((x, input.y));
    }

    let hint_line = if !c.suggestions.is_empty() {
        let mut spans = vec![Span::styled("tab ", Style::new().fg(theme.accent))];
        for (i, (value, count)) in c.suggestions.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", theme.dim));
            }
            spans.push(Span::raw(value.clone()));
            spans.push(Span::styled(format!(" {count}"), theme.dim));
        }
        Line::from(spans)
    } else {
        let mut spans = vec![Span::styled("@", Style::new().fg(theme.facet_source)), Span::styled("source ", theme.dim)];
        for (prefix, what) in c.category.hints() {
            let color = match prefix {
                '#' => theme.facet_tag,
                '*' => theme.facet_damage,
                _ => theme.facet_level,
            };
            spans.push(Span::styled(prefix.to_string(), Style::new().fg(color)));
            spans.push(Span::styled(format!("{what} "), theme.dim));
        }
        spans.push(Span::styled("\"", Style::new().fg(theme.facet_text)));
        spans.push(Span::styled("text\" !not", theme.dim));
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(hint_line).wrap(ratatui::widgets::Wrap { trim: true }), hint);
}

fn draw_list(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let title = Line::from(format!(" {} · {} ", c.category.label(), c.results.len()));
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
        frame.render_widget(Paragraph::new(Line::styled(" No results.", theme.dim)).block(block), area);
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

fn draw_placeholder(frame: &mut Frame, area: Rect, title: &str, milestone: &str, items: &[&str], theme: &Theme) {
    let width = items.iter().map(|i| i.width()).max().unwrap_or(20) as u16 + 8;
    let height = items.len() as u16 + 6;
    let [row] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(width.min(area.width))]).flex(Flex::Center).areas(row);

    let mut lines = vec![
        Line::styled(format!("Coming in {milestone}"), Style::new().fg(theme.accent)),
        Line::default(),
    ];
    lines.extend(items.iter().map(|i| {
        Line::from(vec![Span::styled("• ", theme.dim), Span::raw(i.to_string())])
    }));
    let block = pane(Line::from(format!(" {title} ")), false, theme);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(lines).block(block.padding(ratatui::widgets::Padding::uniform(1))),
        rect,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let hints: &[(&str, &str)] = match (app.section, app.compendium.focus) {
        (Section::Compendium, Focus::Search) => &[
            ("type", "filter"),
            ("↑↓ ^n^p", "move"),
            ("enter", "open"),
            ("tab", "complete/category"),
            ("^u", "clear"),
            ("esc", "list"),
        ],
        (Section::Compendium, Focus::List) => &[
            ("j/k", "move"),
            ("enter", "open"),
            ("/", "search"),
            ("tab", "category"),
            ("1-3", "section"),
            ("q", "quit"),
        ],
        (Section::Compendium, Focus::Detail) => &[
            ("j/k", "scroll"),
            ("tab/n", "next link"),
            ("enter", "follow"),
            ("bksp", "back"),
            ("esc", "list"),
        ],
        _ => &[("1-3", "section"), ("q", "quit")],
    };
    let mut spans = Vec::new();
    for (key, what) in hints {
        spans.push(Span::styled(format!(" {key} "), Style::new().fg(theme.accent)));
        spans.push(Span::styled(format!("{what} "), theme.dim));
    }
    if !app.status.is_empty() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(app.status.clone(), Style::new().fg(theme.accent)));
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
