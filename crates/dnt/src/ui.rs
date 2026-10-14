use dnt_data::Library;
use dnt_store::PRESETS;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Section};
use crate::campaigns::{self, Pane};
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
        Section::Campaigns => draw_campaigns(frame, body, app, theme),
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
        Some(set) if set.len() > 6 => format!("{} sources", set.len()),
        Some(set) => {
            let mut v: Vec<String> = set.iter().map(|s| s.to_uppercase()).collect();
            v.sort();
            v.join(" ")
        }
    };
    let (campaign, campaign_style) = match app.active_campaign() {
        Some(c) => (c.settings.name.clone(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
        None => ("no campaign — press 4".to_string(), theme.dim),
    };
    let right = vec![
        Span::styled(campaign, campaign_style),
        Span::styled(format!("  {sources} "), theme.dim),
    ];
    let used: usize = spans.iter().chain(&right).map(|s| s.content.width()).sum();
    let pad = (area.width as usize).saturating_sub(used);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right);
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

fn draw_campaigns(frame: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    let [left, right] = Layout::horizontal([Constraint::Length(36), Constraint::Min(30)]).areas(area);
    let c = &mut app.campaigns;

    // Campaign list.
    let block = pane(Line::from(" Campaigns "), c.pane == Pane::Campaigns, theme);
    let inner = block.inner(left);
    frame.render_widget(block, left);
    let mut lines: Vec<Line> = Vec::new();
    for (i, camp) in c.list.iter().enumerate() {
        let marker = if Some(i) == app.active { "● " } else { "  " };
        let mut line = Line::from(vec![
            Span::styled(marker, Style::new().fg(theme.accent)),
            Span::raw(camp.settings.name.clone()),
        ]);
        if i == c.selected {
            line = line.style(theme.selected);
        }
        lines.push(line);
    }
    if c.list.is_empty() && c.name_input.is_none() {
        lines.push(Line::styled("No campaigns yet.", theme.dim));
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::raw("Press "),
            Span::styled("n", Style::new().fg(theme.accent)),
            Span::raw(" to create one."),
        ]));
    }
    if let Some(name) = &c.name_input {
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled("Name: ", Style::new().fg(theme.accent)), Span::raw(name.clone())]));
        let y = inner.y + lines.len() as u16 - 1;
        frame.set_cursor_position((inner.x + 6 + name.width() as u16, y.min(inner.bottom().saturating_sub(1))));
    }
    frame.render_widget(Paragraph::new(lines), inner);

    // Sources of the selected campaign.
    let Some(campaign) = c.list.get(c.selected) else {
        let block = pane(Line::from(" Sources "), false, theme);
        frame.render_widget(block, right);
        return;
    };
    let summary = c.summary(campaign);
    let summary_text = if campaign.settings.sources.is_empty() {
        "all sources".to_string()
    } else {
        summary.iter().map(|(g, n)| format!("{g} {n}")).collect::<Vec<_>>().join(" · ")
    };
    let title = Line::from(vec![
        Span::styled(format!(" Sources · {} ", campaign.settings.name), Style::new().add_modifier(Modifier::BOLD)),
        Span::styled(format!("{summary_text} "), theme.dim),
    ]);
    let block = pane(title, c.pane == Pane::Sources, theme);
    let inner = block.inner(right);
    frame.render_widget(block, right);

    let [info, list_area] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
    let next_preset = PRESETS[c.preset % PRESETS.len()].0;
    let reprints = if campaign.settings.show_reprinted { "shown" } else { "hidden" };
    let filter = if c.filtering || !c.source_filter.is_empty() {
        Span::styled(format!("  /{}", c.source_filter), Style::new().fg(theme.accent))
    } else {
        Span::raw("")
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("p", Style::new().fg(theme.accent)),
                Span::styled(format!(" apply preset: {next_preset}   "), theme.dim),
                Span::styled("r", Style::new().fg(theme.accent)),
                Span::styled(format!(" reprinted originals: {reprints}"), theme.dim),
                filter,
            ]),
            Line::default(),
        ]),
        info,
    );

    let visible = c.visible_sources();
    let height = list_area.height as usize;
    c.source_height = height;
    c.source_selected = c.source_selected.min(visible.len().saturating_sub(1));
    // Rows include group headers; build them, then window around the selection.
    let mut rows: Vec<(Option<usize>, Line)> = Vec::new();
    let mut last_group = "";
    for (pos, &i) in visible.iter().enumerate() {
        let s = &c.sources[i];
        if s.group != last_group {
            rows.push((None, Line::styled(s.group.clone(), theme.subheading.fg(theme.accent))));
            last_group = &s.group;
        }
        let on = campaigns::enabled(campaign, &s.id);
        let check = if on { "[x] " } else { "[ ] " };
        let mut line = Line::from(vec![
            Span::styled(check, if on { Style::new().fg(theme.accent) } else { theme.dim }),
            Span::raw(s.name.clone()),
            Span::styled(format!("  {} · {}", s.id, s.count), theme.dim),
        ]);
        if pos == c.source_selected && c.pane == Pane::Sources {
            line = line.style(theme.selected);
        }
        rows.push((Some(pos), line));
    }
    let sel_row = rows.iter().position(|(p, _)| *p == Some(c.source_selected)).unwrap_or(0);
    if sel_row < c.source_offset {
        c.source_offset = sel_row.saturating_sub(1);
    } else if sel_row >= c.source_offset + height {
        c.source_offset = sel_row + 1 - height;
    }
    let lines: Vec<Line> = rows.into_iter().skip(c.source_offset).take(height).map(|(_, l)| l).collect();
    frame.render_widget(Paragraph::new(lines), list_area);
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
            ("1-4", "section"),
            ("q", "quit"),
        ],
        (Section::Compendium, Focus::Detail) => &[
            ("j/k", "scroll"),
            ("tab/n", "next link"),
            ("enter", "follow"),
            ("bksp", "back"),
            ("esc", "list"),
        ],
        (Section::Campaigns, _) if app.campaigns.name_input.is_some() => &[("enter", "create"), ("esc", "cancel")],
        (Section::Campaigns, _) if app.campaigns.filtering => &[("type", "filter"), ("enter", "done"), ("esc", "clear")],
        (Section::Campaigns, _) if app.campaigns.pane == Pane::Campaigns => &[
            ("j/k", "move"),
            ("enter", "activate"),
            ("n", "new"),
            ("tab/l", "sources"),
            ("1-4", "section"),
            ("q", "quit"),
        ],
        (Section::Campaigns, _) => &[
            ("j/k", "move"),
            ("space", "toggle"),
            ("p", "preset"),
            ("a", "all"),
            ("r", "reprints"),
            ("/", "filter"),
            ("tab/h", "campaigns"),
        ],
        _ => &[("1-4", "section"), ("q", "quit")],
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
