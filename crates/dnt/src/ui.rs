use dnt_data::{Kind, Library};
use dnt_store::PRESETS;
use ratatui::layout::{Constraint, Flex, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Section};
use crate::campaigns::{self, Pane};
use crate::compendium::{Category, Compendium, Focus};
use crate::render;
use crate::theme::Theme;

/// Widest the body text of an entry gets, for comfortable reading.
const READING_WIDTH: u16 = 96;

pub fn draw(frame: &mut Frame, lib: &Library, app: &mut App, theme: &Theme) {
    let [header, _, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let body = body.inner(Margin::new(1, 0));

    draw_header(frame, header, lib, app, theme);
    match app.section {
        Section::Compendium if app.editor.is_some() => {
            let filter = app.compendium.filter.clone();
            let editor = app.editor.as_mut().unwrap();
            let doc = match editor.target {
                crate::entry_editor::Target::Campaign => app.homebrew.as_ref(),
                crate::entry_editor::Target::Global => app.global_homebrew.as_ref(),
            };
            crate::ui_editor::draw(frame, body, lib, &filter, doc, editor, theme)
        }
        Section::Compendium => {
            draw_compendium(frame, body, lib, &mut app.compendium, theme);
            if let Some(flow) = &app.clone_flow {
                crate::ui_editor::draw_clone(frame, body, lib, flow, theme);
            }
        }
        Section::Campaigns => draw_campaigns(frame, body, app, theme),
        Section::Characters => {
            let campaign = app.active.map(|i| app.campaigns.list[i].settings.name.clone());
            crate::ui_sheet::draw(frame, body, lib, &app.overlays, &mut app.characters, theme, campaign.as_deref())
        }
        Section::Notes => {
            let filter = app.compendium.filter.clone();
            crate::ui_notes::draw(frame, body, lib, &filter, &mut app.notes, theme)
        }
    }
    if let Some(flow) = &app.new_flow {
        crate::ui_editor::draw_new(frame, body, flow, theme);
    }
    draw_footer(frame, footer, app, theme);
    if let Some(text) = &app.capture {
        draw_capture(frame, frame.area(), text, theme);
    }
}

fn draw_capture(frame: &mut Frame, area: Rect, text: &str, theme: &Theme) {
    let [row] = Layout::vertical([Constraint::Length(5)]).flex(Flex::Center).areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(72.min(area.width))]).flex(Flex::Center).areas(row);
    frame.render_widget(Clear, rect);
    let block = pane(pane_title("Quick capture → Inbox", true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(" enter saves · esc cancels ", theme.dim).right_aligned());
    let inner = block.inner(rect);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.to_string())])).block(block),
        rect,
    );
    frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
}

// ---- chrome ----

fn draw_header(frame: &mut Frame, area: Rect, lib: &Library, app: &App, theme: &Theme) {
    let mut spans = vec![Span::styled(" dnt ", theme.brand), Span::raw("  ")];
    for (i, s) in Section::ALL.iter().enumerate() {
        if *s == app.section {
            spans.push(Span::styled(format!(" {} {} ", i + 1, s.label()), theme.tab_active));
        } else {
            spans.push(Span::styled(format!(" {}", i + 1), theme.dim));
            spans.push(Span::styled(format!(" {} ", s.label()), theme.tab_inactive));
        }
        spans.push(Span::raw(" "));
    }

    // Always-on sources (the global homebrew) only count once they exist.
    let enabled = app.compendium.filter.enabled.as_ref().map(|set| {
        set.iter().filter(|s| lib.source_info(s).is_some() || !s.eq_ignore_ascii_case(dnt_store::GLOBAL_SOURCE)).collect::<Vec<_>>()
    });
    let sources = match &enabled {
        None => "all sources".to_string(),
        Some(set) if set.len() > 5 => format!("{} sources", set.len()),
        Some(set) => {
            let mut v: Vec<String> = set.iter().map(|s| s.to_uppercase()).collect();
            v.sort();
            v.join(" ")
        }
    };
    let right = match app.active_campaign() {
        Some(c) => vec![
            Span::styled("◈ ", Style::new().fg(theme.accent)),
            Span::styled(c.settings.name.clone(), Style::new().add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {sources} "), theme.dim),
        ],
        None => vec![Span::styled("no campaign · press 4 ", theme.dim)],
    };
    let used: usize = spans.iter().chain(&right).map(|s| s.content.width()).sum();
    spans.push(Span::raw(" ".repeat((area.width as usize).saturating_sub(used))));
    spans.extend(right);
    frame.render_widget(Paragraph::new(Line::from(spans)).style(theme.bar), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    use crate::entry_editor::Mode as EdMode;
    let hints: &[(&str, &str)] = match (app.section, app.compendium.focus) {
        _ if app.new_flow.as_ref().is_some_and(|f| f.name.is_some()) => &[("enter", "create"), ("^u", "clear"), ("esc", "back")],
        _ if app.new_flow.is_some() => &[("j k", "choose"), ("enter", "next"), ("esc", "cancel")],
        (Section::Compendium, _) if app.clone_flow.as_ref().is_some_and(|f| f.name.is_some()) => &[("enter", "create copy"), ("esc", "cancel")],
        (Section::Compendium, _) if app.clone_flow.is_some() => &[("j k", "choose"), ("enter", "clone"), ("esc", "cancel")],
        (Section::Compendium, _) if app.editor.is_some() => match app.editor.as_ref().map(|e| &e.mode) {
            Some(EdMode::Big { .. }) => &[("i", "insert"), ("esc", "normal · save"), ("^s", "save")],
            Some(EdMode::Prompt { .. } | EdMode::AddField(_)) => &[("enter", "set"), ("^u", "clear"), ("esc", "cancel")],
            Some(EdMode::ConfirmDelete) => &[("y", "delete"), ("n", "keep")],
            Some(EdMode::Choose { .. }) => &[("j k", "move"), ("enter", "choose"), ("esc", "cancel")],
            Some(_) if app.editor.as_ref().is_some_and(|e| e.replaces || e.target == crate::entry_editor::Target::Global) => &[
                ("j k", "field"),
                ("enter", "edit"),
                ("J", "as JSON"),
                ("x", "revert"),
                ("a", "add field"),
                ("E", "$EDITOR"),
                ("D", "delete"),
                ("esc", "done"),
            ],
            _ => &[
                ("j k", "field"),
                ("enter", "edit"),
                ("J", "as JSON"),
                ("x", "clear"),
                ("a", "add field"),
                ("E", "$EDITOR"),
                ("P", "make global"),
                ("D", "delete"),
                ("esc", "done"),
            ],
        },
        (Section::Compendium, Focus::Search) => &[
            ("↑↓", "move"),
            ("enter", "open"),
            ("tab", "complete · category"),
            ("^u", "clear"),
            ("esc", "list"),
        ],
        (Section::Compendium, Focus::List) => &[
            ("j k", "move"),
            ("enter", "open"),
            ("/", "search"),
            ("tab", "category"),
            ("c e n", "clone/edit/new"),
            ("1-4", "section"),
            ("q", "quit"),
        ],
        (Section::Compendium, Focus::Detail) => &[
            ("j k", "scroll"),
            ("tab n", "next link"),
            ("enter", "follow"),
            ("bksp", "back"),
            ("esc", "list"),
        ],
        (Section::Campaigns, _) if app.campaigns.name_input.is_some() => &[("enter", "create"), ("esc", "cancel")],
        (Section::Campaigns, _) if app.campaigns.filtering => &[("enter", "done"), ("esc", "clear")],
        (Section::Campaigns, _) if app.campaigns.pane == Pane::Campaigns => &[
            ("j k", "move"),
            ("enter", "activate"),
            ("n", "new"),
            ("tab", "sources"),
            ("1-4", "section"),
            ("q", "quit"),
        ],
        (Section::Campaigns, _) => &[
            ("j k", "move"),
            ("space", "toggle"),
            ("p", "preset"),
            ("a", "all"),
            ("r", "reprints"),
            ("/", "filter"),
            ("tab", "campaigns"),
        ],
        (Section::Characters, _) if app.characters.wizard.is_some() => &[("enter", "choose"), ("esc", "back a step"), ("alt+1-4", "section")],
        (Section::Characters, _) => match app.characters.sheet.as_ref() {
            _ if app.characters.name_input.is_some() => &[("enter", "create"), ("esc", "cancel")],
            None => &[("j k", "move"), ("enter", "open"), ("n", "new"), ("e", "edit file"), ("1-4", "section"), ("q", "quit")],
            Some(s) if s.popup.is_some() => &[("enter", "confirm"), ("esc", "close")],
            Some(s) if s.tab == crate::characters::Tab::Spells => &[
                ("j k", "move"),
                ("a", "add"),
                ("enter", "cast/choose"),
                ("p", "prepare"),
                ("f", "favorite"),
                ("x", "remove"),
                ("C", "end conc."),
                ("tab", "tab"),
                ("esc", "roster"),
            ],
            Some(s) if s.tab == crate::characters::Tab::Inventory => &[
                ("j k", "move"),
                ("a", "add"),
                ("+ -", "qty"),
                ("x", "remove"),
                ("space", "wear/hold"),
                ("o", "off hand"),
                ("A", "attune"),
                ("m", "monk weapon"),
                ("g", "coins"),
                ("tab", "tab"),
                ("esc", "roster"),
            ],
            Some(s) if s.tab == crate::characters::Tab::Features => {
                &[("j k", "feature"), ("J K", "scroll text"), ("u U", "level up/undo"), ("tab", "tab"), ("e", "edit"), ("esc", "roster")]
            }
            Some(s) if matches!(s.current_item(), Some(crate::characters::Item::Attack(_))) => &[
                ("j k", "move"),
                ("space", "main hand"),
                ("o", "off hand"),
                ("S", "shield"),
                ("?", "breakdown"),
                ("d h t", "dmg/heal/temp"),
                ("esc", "roster"),
            ],
            Some(_) => &[
                ("j k", "move"),
                ("space", "use"),
                ("+", "restore"),
                ("?", "breakdown"),
                ("d h t", "dmg/heal/temp"),
                ("s L", "rests"),
                ("u U", "level up/undo"),
                ("c", "choices"),
                ("e", "edit"),
                ("esc", "roster"),
            ],
        },
        (Section::Notes, _) if app.notes.popup.is_some() => &[("enter", "confirm"), ("esc", "cancel")],
        (Section::Notes, _) if app.notes.focus == crate::notes_view::Focus::Editor => &[
            ("esc", "save & tree"),
            ("i", "insert"),
            ("gd", "follow link"),
            ("K", "preview"),
            ("^o", "back"),
            (":", "command"),
            ("alt+c", "capture"),
        ],
        (Section::Notes, _) => &[
            ("j k", "move"),
            ("enter", "open"),
            ("a A", "note/folder"),
            ("s", "session"),
            ("r d", "rename/delete"),
            ("J K", "move folder"),
            ("/", "find"),
            ("i c", "inbox/hub"),
            ("alt+c", "capture"),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (key, what) in hints {
        spans.push(Span::styled(format!(" {key} "), theme.keycap));
        spans.push(Span::styled(format!(" {what}   "), theme.dim));
    }
    if !app.status.is_empty() {
        let status = format!("{} ", app.status);
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let pad = (area.width as usize).saturating_sub(used + status.width());
        spans.push(Span::raw(" ".repeat(pad.max(1))));
        spans.push(Span::styled(status, Style::new().fg(theme.accent)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub(crate) fn pane(title: Line<'static>, focused: bool, theme: &Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(theme.border_type)
        .border_style(if focused { theme.border_focused } else { theme.border })
        .title(title)
        .padding(Padding::horizontal(1))
}

pub(crate) fn pane_title(text: &str, focused: bool, theme: &Theme) -> Line<'static> {
    let style = if focused { theme.title } else { theme.pane_title };
    Line::from(Span::styled(format!(" {text} "), style))
}

/// Pad `left` and `right` spans apart to fill `width`.
pub(crate) fn spread(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let used: usize = left.iter().chain(&right).map(|s| s.content.width()).sum();
    left.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    left.extend(right);
    Line::from(left)
}

// ---- compendium ----

fn draw_compendium(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let [tabs, _, main] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3)]).areas(area);
    let list_w = (main.width * 2 / 7).clamp(32, 56);
    let [left, _, right] =
        Layout::horizontal([Constraint::Length(list_w), Constraint::Length(1), Constraint::Min(20)]).areas(main);

    draw_categories(frame, tabs, c, theme);
    draw_browser(frame, left, lib, c, theme);
    draw_detail(frame, right, lib, c, theme);
}

fn draw_categories(frame: &mut Frame, area: Rect, c: &Compendium, theme: &Theme) {
    let with_counts: usize = Category::ORDER
        .iter()
        .zip(&c.category_counts)
        .map(|(cat, n)| cat.label().width() + n.to_string().width() + 4)
        .sum();
    let show_counts = with_counts + 10 <= area.width as usize;
    let mut spans = Vec::new();
    for (cat, count) in Category::ORDER.iter().zip(&c.category_counts) {
        let active = *cat == c.category;
        let style = if active { theme.category_active } else { theme.category_inactive };
        spans.push(Span::styled(format!(" {}", cat.label()), style));
        if show_counts {
            let count_style = if active { style.remove_modifier(Modifier::BOLD) } else { theme.dim };
            spans.push(Span::styled(format!(" {count}"), count_style));
        }
        spans.push(Span::styled(" ", style));
        spans.push(Span::raw(" "));
    }
    let hint = vec![Span::styled(" tab", theme.keycap), Span::styled(" ⇄ ", theme.keycap)];
    frame.render_widget(Paragraph::new(spread(spans, hint, area.width as usize)), area);
}

/// The query with each term coloured by its role.
fn query_line(query: &str, theme: &Theme) -> Vec<Span<'static>> {
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
            _ => Style::new().add_modifier(Modifier::BOLD),
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
    spans
}

/// Search box, hints and result list in one pane.
fn draw_browser(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let focused = matches!(c.focus, Focus::Search | Focus::List);
    let count = Line::from(Span::styled(format!(" {} results ", c.results.len()), theme.dim)).right_aligned();
    let block = pane(pane_title("Browse", focused, theme), focused, theme).title_bottom(count);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [input, hint, rule, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);

    // Input line.
    let searching = c.focus == Focus::Search;
    let prompt = Span::styled("❯ ", if searching { Style::new().fg(theme.accent) } else { theme.dim });
    let mut spans = vec![prompt];
    if c.query.is_empty() && !searching {
        spans.push(Span::styled("search", theme.dim));
        spans.push(Span::styled("  /", theme.dim));
    } else {
        spans.extend(query_line(&c.query, theme));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), input);
    if searching {
        let x = input.x + 2 + (c.query.width() as u16).min(input.width.saturating_sub(3));
        frame.set_cursor_position((x, input.y));
    }

    // Hints or completions.
    let hint_line = if !c.suggestions.is_empty() {
        let mut spans = vec![Span::styled(" tab ", theme.keycap), Span::raw(" ")];
        for (i, (value, count)) in c.suggestions.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", theme.dim));
            }
            spans.push(Span::styled(value.clone(), if i == 0 { Style::new().fg(theme.facet_tag) } else { theme.muted }));
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
        spans.push(Span::styled("text\" ", theme.dim));
        spans.push(Span::styled("!", Style::new().fg(theme.accent)));
        spans.push(Span::styled("not", theme.dim));
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(hint_line), hint);
    frame.render_widget(Paragraph::new(Line::styled("─".repeat(rule.width as usize), theme.border)), rule);

    // Results.
    let height = list.height as usize;
    c.list_height = height;
    if c.selected < c.list_offset {
        c.list_offset = c.selected;
    } else if c.selected >= c.list_offset + height {
        c.list_offset = c.selected + 1 - height;
    }
    let width = list.width as usize;
    let lines: Vec<Line> = c
        .results
        .iter()
        .enumerate()
        .skip(c.list_offset)
        .take(height)
        .map(|(i, &id)| {
            let e = lib.entity(id);
            let selected = i == c.selected;
            let (glyph, color) = kind_glyph(e.kind);
            let meta = format!(" {} ", e.source);
            let name_w = width.saturating_sub(meta.width() + 3);
            let name = truncate(&e.name, name_w);
            let base = if selected {
                if focused {
                    theme.selected
                } else {
                    theme.selected_unfocused
                }
            } else {
                Style::new()
            };
            let marker = if selected && focused { Span::styled("▌", theme.selection_marker) } else { Span::styled(" ", base) };
            let line = spread(
                vec![
                    marker,
                    Span::styled(format!("{glyph} "), base.fg(color).remove_modifier(Modifier::BOLD)),
                    Span::styled(name, base),
                ],
                vec![Span::styled(
                    meta,
                    base.patch(if selected { theme.muted } else { theme.dim }).remove_modifier(Modifier::BOLD),
                )],
                width,
            );
            line.style(base)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);

    if c.results.len() > height {
        let mut state = ScrollbarState::new(c.results.len().saturating_sub(height)).position(c.list_offset);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::new().fg(theme.accent))
                .track_style(theme.border),
            area.inner(Margin::new(0, 4)),
            &mut state,
        );
    }
}

fn kind_glyph(kind: Kind) -> (&'static str, Color) {
    use Kind::*;
    match kind {
        Spell => ("✦", Color::Magenta),
        Class | Subclass => ("◆", Color::Yellow),
        ClassFeature | SubclassFeature | OptionalFeature => ("◇", Color::Yellow),
        Race | Subrace => ("●", Color::Green),
        Background => ("■", Color::Blue),
        Feat => ("★", Color::Cyan),
        Item | BaseItem | ItemGroup | MagicVariant | ItemMastery => ("♦", Color::Red),
        _ => ("¶", Color::Gray),
    }
}

fn draw_detail(frame: &mut Frame, area: Rect, lib: &Library, c: &mut Compendium, theme: &Theme) {
    let focused = c.focus == Focus::Detail;
    let Some(detail) = c.detail.as_mut() else {
        let block = pane(pane_title("Entry", false, theme), false, theme);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let [mid] = Layout::vertical([Constraint::Length(1)]).flex(Flex::Center).areas(inner);
        frame.render_widget(Paragraph::new(Line::styled("Nothing matches.", theme.dim).centered()), mid);
        c.detail_links.clear();
        return;
    };
    let e = lib.entity(detail.entity);

    let mut block = pane(Line::default(), focused, theme).padding(Padding::new(2, 2, 1, 0));
    if let Some(prev) = c.history.last() {
        let crumb = Line::from(vec![
            Span::styled(" ← ", Style::new().fg(theme.accent)),
            Span::styled(format!("{} ", lib.entity(prev.entity).name), theme.muted),
            Span::styled(format!("· {} back ", c.history.len()), theme.dim),
        ]);
        block = block.title_bottom(crumb);
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Sticky title and subtitle.
    let source = lib.source_info(&e.source).map_or(e.source.as_str(), |s| s.name.as_str());
    let page = e.data.get("page").and_then(|p| p.as_u64()).map(|p| format!(" · p. {p}")).unwrap_or_default();
    let (glyph, color) = kind_glyph(e.kind);
    let [title, subtitle, _, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{glyph} "), Style::new().fg(color)),
            Span::styled(e.name.clone(), theme.title),
        ])),
        title,
    );
    let mut sub = vec![
        Span::styled(e.kind.label(), Style::new().fg(color)),
        Span::styled(format!("  ·  {source}{page}"), theme.dim),
    ];
    let changed = lib.overridden_fields(detail.entity);
    if lib.original(detail.entity).is_some() {
        if detail.original {
            sub.push(Span::styled("   book version ", theme.keycap));
            sub.push(Span::styled("  o shows the campaign version", theme.dim));
        } else {
            sub.push(Span::styled("   campaign version ", theme.category_active));
            let what = if changed.is_empty() { String::new() } else { format!(" changed: {}", changed.join(", ")) };
            sub.push(Span::styled(format!("{what}  · o shows the original"), theme.dim));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(sub)), subtitle);

    let text_area = Rect {
        width: body.width.min(READING_WIDTH),
        ..body
    };
    let link_focus = if focused { detail.focused_link } else { None };
    let rendered = render::render(&detail.doc, text_area.width, link_focus, theme);
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
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(Style::new().fg(theme.accent))
                .track_style(theme.border),
            area.inner(Margin::new(0, 1)),
            &mut state,
        );
    }
}

// ---- other sections ----

fn draw_campaigns(frame: &mut Frame, area: Rect, app: &mut App, theme: &Theme) {
    let [left, _, right] =
        Layout::horizontal([Constraint::Length(36), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let c = &mut app.campaigns;

    // Campaign list.
    let focused = c.pane == Pane::Campaigns;
    let block = pane(pane_title("Campaigns", focused, theme), focused, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(left);
    frame.render_widget(block, left);
    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (i, camp) in c.list.iter().enumerate() {
        let active = Some(i) == app.active;
        let base = if i == c.selected {
            if focused {
                theme.selected
            } else {
                theme.selected_unfocused
            }
        } else {
            Style::new()
        };
        let marker = if active { "◈ " } else { "  " };
        let line = spread(
            vec![
                Span::styled(marker, base.fg(theme.accent)),
                Span::styled(camp.settings.name.clone(), base),
            ],
            vec![Span::styled(if active { "active " } else { "" }, base.patch(theme.dim))],
            width,
        );
        lines.push(line.style(base));
    }
    if c.list.is_empty() && c.name_input.is_none() {
        lines.push(Line::styled("No campaigns yet.", theme.muted));
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Press ", theme.dim),
            Span::styled(" n ", theme.keycap),
            Span::styled(" to create one.", theme.dim),
        ]));
    }
    if let Some(name) = &c.name_input {
        lines.push(Line::default());
        lines.push(Line::styled("New campaign", theme.label));
        lines.push(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(name.clone())]));
        let y = inner.y + lines.len() as u16 - 1;
        frame.set_cursor_position((inner.x + 2 + name.width() as u16, y.min(inner.bottom().saturating_sub(1))));
    }
    frame.render_widget(Paragraph::new(lines), inner);

    // Sources of the selected campaign.
    let focused = c.pane == Pane::Sources;
    let Some(campaign) = c.list.get(c.selected) else {
        let block = pane(pane_title("Sources", false, theme), false, theme);
        frame.render_widget(block, right);
        return;
    };
    let summary_text = if campaign.settings.sources.is_empty() {
        "all sources".to_string()
    } else {
        c.summary(campaign)
            .iter()
            .map(|(g, n)| format!("{g} {n}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let block = pane(pane_title(&format!("Books · {}", campaign.settings.name), focused, theme), focused, theme)
        .title_bottom(Line::from(Span::styled(format!(" {summary_text} "), theme.dim)).right_aligned())
        .padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(right);
    frame.render_widget(block, right);

    let [info, _, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let next_preset = PRESETS[c.preset % PRESETS.len()].0;
    let reprints = if campaign.settings.show_reprinted { "shown" } else { "hidden" };
    let mut info_spans = vec![
        Span::styled(" p ", theme.keycap),
        Span::styled(format!(" preset: {next_preset}    "), theme.dim),
        Span::styled(" r ", theme.keycap),
        Span::styled(format!(" reprinted originals: {reprints}"), theme.dim),
    ];
    if c.filtering || !c.source_filter.is_empty() {
        info_spans.push(Span::styled(format!("    /{}", c.source_filter), Style::new().fg(theme.accent)));
        if c.filtering {
            let x: u16 = info_spans.iter().map(|s| s.content.width() as u16).sum();
            frame.set_cursor_position((info.x + x, info.y));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(info_spans)), info);

    let visible = c.visible_sources();
    let height = list_area.height as usize;
    let width = list_area.width.min(READING_WIDTH) as usize;
    c.source_height = height;
    c.source_selected = c.source_selected.min(visible.len().saturating_sub(1));
    // Rows include group headers; build them, then window around the selection.
    let mut rows: Vec<(Option<usize>, Line)> = Vec::new();
    let mut last_group = "";
    for (pos, &i) in visible.iter().enumerate() {
        let s = &c.sources[i];
        if s.group != last_group {
            if !rows.is_empty() {
                rows.push((None, Line::default()));
            }
            rows.push((None, Line::styled(s.group.clone(), theme.heading)));
            last_group = &s.group;
        }
        let on = campaigns::enabled(campaign, &s.id);
        let selected = pos == c.source_selected && focused;
        let base = if selected { theme.selected } else { Style::new() };
        let check = if on { "● " } else { "○ " };
        // On the highlighted row, dim greys would vanish into the highlight.
        let faint = if selected { theme.muted } else { theme.dim };
        let line = spread(
            vec![
                Span::styled(check, if on { base.fg(theme.accent) } else { base.patch(faint) }),
                Span::styled(s.name.clone(), if on || selected { base } else { base.patch(theme.muted) }),
            ],
            vec![
                Span::styled(format!("{:<6}", s.id), base.patch(theme.muted).remove_modifier(Modifier::BOLD)),
                Span::styled(format!("{:>6} ", s.count), base.patch(faint).remove_modifier(Modifier::BOLD)),
            ],
            width,
        );
        let mut line = line;
        if selected {
            line = line.style(base);
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

pub(crate) fn truncate(s: &str, width: usize) -> String {
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
