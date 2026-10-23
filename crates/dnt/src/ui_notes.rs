//! Drawing for the Notes section.

use dnt_data::{view, Library, SourceFilter};
use dnt_store::notes;
use edtui::{EditorMode, EditorTheme, EditorView, Highlight, Index2};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph, Widget};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::notes_view::{Focus, NotesView, Popup, PromptKind};
use crate::render;
use crate::theme::Theme;
use crate::ui::{pane, pane_title, spread, truncate};
use crate::ui_sheet::draw_picker;

pub fn draw(frame: &mut Frame, area: Rect, lib: &Library, filter: &SourceFilter, v: &mut NotesView, theme: &Theme) {
    if v.notes.is_none() {
        let block = pane(pane_title("Notes", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("No active campaign.", theme.muted),
                Line::styled("Notes belong to a campaign: create or activate one in 4 Campaigns.", theme.dim),
            ])
            .block(block),
            area,
        );
        return;
    }
    let [tree, _, editor, _, info] = Layout::horizontal([
        Constraint::Length(34),
        Constraint::Length(1),
        Constraint::Min(40),
        Constraint::Length(1),
        Constraint::Length(36),
    ])
    .areas(area);
    draw_tree(frame, tree, v, theme);
    draw_editor(frame, editor, v, theme);
    draw_info(frame, info, v, theme);
    draw_popup(frame, area, lib, filter, v, theme);
}

fn draw_tree(frame: &mut Frame, area: Rect, v: &mut NotesView, theme: &Theme) {
    let focused = v.focus == Focus::Tree && v.popup.is_none();
    let block = pane(pane_title(&format!("Notes · {}", v.campaign), focused, theme), focused, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = v.rows();
    v.cursor = v.cursor.min(rows.len().saturating_sub(1));
    let open = v.open.as_ref().map(|o| o.path.clone());
    let width = inner.width as usize;
    let lines: Vec<Line> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let base = if i == v.cursor {
                if focused {
                    theme.selected
                } else {
                    theme.selected_unfocused
                }
            } else {
                Style::new()
            };
            let indent = "  ".repeat(r.depth);
            let (icon, style) = if r.entry.is_dir {
                let expanded = v.expanded.contains(&r.entry.path);
                (if expanded { "▾ " } else { "▸ " }, base.patch(theme.heading))
            } else if open.as_deref() == Some(r.entry.path.as_str()) {
                ("● ", base.fg(theme.accent))
            } else {
                ("  ", base)
            };
            let name_style = if r.entry.is_dir { base.patch(theme.heading) } else { base };
            let mut left = vec![
                Span::styled(indent, base),
                Span::styled(icon, style),
                Span::styled(truncate(&r.entry.name, width.saturating_sub(4 + r.depth * 2)), name_style),
            ];
            if r.entry.is_dir {
                let n = count_notes(v, &r.entry.path);
                let faint = if i == v.cursor { theme.muted } else { theme.dim };
                left.push(Span::styled(format!("  {n}"), base.patch(faint).remove_modifier(Modifier::BOLD)));
            }
            Line::from(left).style(base)
        })
        .collect();
    let height = inner.height as usize;
    let scroll = (v.cursor + 1).saturating_sub(height);
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

fn count_notes(v: &NotesView, folder: &str) -> usize {
    let prefix = format!("{folder}/");
    v.index.iter().filter(|n| n.path.starts_with(&prefix)).count()
}

/// Character-based ranges for links, headings and frontmatter.
fn highlights(text: &str, theme: &Theme) -> Vec<Highlight> {
    let mut out = Vec::new();
    let char_col = |line: &str, byte: usize| line[..byte.min(line.len())].chars().count();
    let rows: Vec<&str> = text.lines().collect();
    // Frontmatter: from a first-line `---` to the next `---`.
    let front_end = (rows.first().map(|l| l.trim()) == Some("---"))
        .then(|| rows.iter().skip(1).position(|l| l.trim() == "---").map(|p| p + 1))
        .flatten();
    for (row, line) in rows.iter().enumerate() {
        let len = line.chars().count();
        if len == 0 {
            continue;
        }
        let style = if front_end.is_some_and(|end| row <= end) {
            theme.dim
        } else if is_heading(line) {
            theme.heading
        } else {
            continue;
        };
        out.push(Highlight { start: Index2::new(row, 0), end: Index2::new(row, len - 1), style });
    }
    for l in notes::links(text) {
        let Some(line) = text.lines().nth(l.line) else { continue };
        let style = if l.compendium().is_some() { Style::new().fg(Color::Magenta) } else { theme.link };
        out.push(Highlight {
            start: Index2::new(l.line, char_col(line, l.start)),
            end: Index2::new(l.line, char_col(line, l.end).saturating_sub(1)),
            style,
        });
    }
    out
}

/// Markdown ATX heading: one to six `#` followed by a space.
fn is_heading(line: &str) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
}

fn draw_editor(frame: &mut Frame, area: Rect, v: &mut NotesView, theme: &Theme) {
    let focused = v.focus == Focus::Editor && v.popup.is_none();
    let Some(open) = v.open.as_mut() else {
        let block = pane(pane_title("No note open", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
        let key = |k: &str, what: &str| {
            Line::from(vec![Span::styled(format!(" {k} "), theme.keycap), Span::styled(format!("  {what}"), theme.dim)])
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Pick a note in the tree, or:", theme.muted),
                Line::default(),
                key("s", "new session note for today"),
                key("a", "new note in the folder under the cursor (uses its template)"),
                key("A", "new folder"),
                key("i", "open the inbox      c  open the campaign hub"),
                key("/", "find a note"),
                key("alt+c", "quick capture to the inbox, from anywhere"),
                Line::default(),
                Line::styled(
                    "In the editor: vim keys · gd follows [[links]] · K previews · ctrl-o back · esc saves and returns",
                    theme.dim,
                ),
            ])
            .block(block),
            area,
        );
        return;
    };
    let (mode, mode_style) = match open.editor.mode {
        EditorMode::Normal => ("NORMAL", theme.keycap),
        EditorMode::Insert => ("INSERT", Style::new().fg(Color::Black).bg(Color::Green)),
        EditorMode::Visual => ("VISUAL", Style::new().fg(Color::Black).bg(Color::Magenta)),
        EditorMode::Search => ("SEARCH", Style::new().fg(Color::Black).bg(Color::Yellow)),
    };
    let dirty = open.dirty();
    let mut title =
        vec![Span::styled(format!(" {} ", open.path.trim_end_matches(".md")), if focused { theme.title } else { theme.pane_title })];
    if dirty {
        title.push(Span::styled("● ", Style::new().fg(theme.accent)));
    }
    let block = pane(Line::from(title), focused, theme)
        .padding(Padding::new(1, 1, 0, 0))
        .title_bottom(Line::from(vec![Span::styled(format!(" {mode} "), mode_style)]).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = open.text();
    open.editor.set_highlights(highlights(&text, theme));
    let editor_theme = EditorTheme::default()
        .base(Style::new())
        .cursor_style(if focused { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() })
        .selection_style(Style::new().bg(Color::DarkGray))
        .hide_status_line();
    EditorView::new(&mut open.editor).theme(editor_theme).wrap(true).render(inner, frame.buffer_mut());
}

fn draw_info(frame: &mut Frame, area: Rect, v: &NotesView, theme: &Theme) {
    let block = pane(pane_title("About", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let heading = |lines: &mut Vec<Line>, t: &str| {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(t.to_string(), theme.heading));
    };
    let item = |t: &str| Line::from(vec![Span::styled("• ", theme.bullet), Span::raw(truncate(t, width.saturating_sub(2)))]);

    let open = v.open.as_ref().and_then(|o| v.index.iter().find(|n| n.path == o.path));
    let hub = v.open.as_ref().is_none_or(|o| o.path == "Campaign.md");
    if let Some(note) = open {
        if !note.meta.is_empty() {
            for (k, val) in &note.meta {
                if val.is_empty() {
                    continue;
                }
                lines.push(spread(
                    vec![Span::styled(k.clone(), theme.label)],
                    vec![Span::raw(truncate(val.trim_start_matches("[[").trim_end_matches("]]"), width.saturating_sub(k.width() + 2)))],
                    width,
                ));
            }
        }
        let back = notes::backlinks(&v.index, note);
        heading(&mut lines, &format!("Mentioned in · {}", back.len()));
        if back.is_empty() {
            lines.push(Line::styled("Nothing links here yet.", theme.dim));
        }
        for b in back {
            lines.push(item(&b.title));
        }
        let text = v.open.as_ref().map(|o| o.text()).unwrap_or_default();
        let links = notes::links(&text);
        if !links.is_empty() {
            heading(&mut lines, &format!("Links · {}", links.len()));
            let mut seen = Vec::new();
            for l in links {
                if seen.contains(&l.target) {
                    continue;
                }
                seen.push(l.target.clone());
                let missing = l.compendium().is_none() && notes::resolve(&v.index, &l.target).is_none();
                let style = if l.compendium().is_some() {
                    Style::new().fg(Color::Magenta)
                } else if missing {
                    theme.dim
                } else {
                    theme.link
                };
                let suffix = if missing { "  (new)" } else { "" };
                lines.push(Line::from(vec![
                    Span::styled("→ ", theme.dim),
                    Span::styled(truncate(&l.target, width.saturating_sub(10)), style),
                    Span::styled(suffix, theme.dim),
                ]));
            }
        }
    }
    // ```dnt query blocks in the open note; the hub has defaults.
    for (title, queries) in dnt_core::notes::query_blocks(open, hub) {
        for q in queries {
            let results = notes::NoteQuery::parse(&q).run(&v.index);
            let label = if title.is_empty() { q.clone() } else { title.clone() };
            heading(&mut lines, &format!("{} · {}", truncate(&label, width.saturating_sub(6)), results.len()));
            if results.is_empty() {
                lines.push(Line::styled("Nothing yet.", theme.dim));
            }
            for r in results {
                lines.push(item(&r.title));
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_search(frame: &mut Frame, area: Rect, v: &NotesView, query: &str, selected: usize, theme: &Theme) {
    let rect = centered(area, 96, area.height.saturating_sub(6).min(30));
    frame.render_widget(Clear, rect);
    let results = notes::NoteQuery::parse(query).run(&v.index);
    let block = pane(pane_title("Search notes", true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(format!(" {} found · enter opens · esc ", results.len()), theme.dim).right_aligned());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let [input, hint, rule, list] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    // Colour terms by kind, like the compendium search.
    let mut spans = vec![Span::styled("❯ ", Style::new().fg(theme.accent))];
    for (i, tok) in query.split(' ').enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let body = tok.trim_start_matches('!');
        let style = match body.chars().next() {
            Some('#') => Style::new().fg(theme.facet_tag),
            Some('"') => Style::new().fg(theme.facet_text),
            Some('@') => Style::new().fg(theme.facet_source),
            _ if body.contains(':') => Style::new().fg(theme.facet_level),
            _ => Style::new().add_modifier(Modifier::BOLD),
        };
        spans.push(Span::styled(tok.to_string(), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), input);
    frame.set_cursor_position((input.x + 2 + query.width() as u16, input.y));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("words", theme.dim),
            Span::styled("  #", Style::new().fg(theme.facet_tag)),
            Span::styled("tag", theme.dim),
            Span::styled("  \"", Style::new().fg(theme.facet_text)),
            Span::styled("full text\"", theme.dim),
            Span::styled("  key:", Style::new().fg(theme.facet_level)),
            Span::styled("value", theme.dim),
            Span::styled("  @", Style::new().fg(theme.facet_source)),
            Span::styled("Folder", theme.dim),
            Span::styled("  !", Style::new().fg(theme.accent)),
            Span::styled("not   sort:-date  limit:5", theme.dim),
        ])),
        hint,
    );
    frame.render_widget(Paragraph::new(Line::styled("─".repeat(rule.width as usize), theme.border)), rule);

    // A text term shows the matching line as context.
    let phrase = query.split('"').nth(1).filter(|p| !p.is_empty()).map(str::to_lowercase);
    let width = list.width as usize;
    let height = list.height as usize;
    let offset = selected.saturating_sub(height.saturating_sub(1));
    let lines: Vec<Line> = results
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(i, n)| {
            let base = if i == selected { theme.selected } else { Style::new() };
            let faint = if i == selected { theme.muted } else { theme.dim };
            let context = phrase
                .as_ref()
                .and_then(|p| n.text.lines().find(|l| l.contains(p.as_str())))
                .map(|l| format!("  “{}”", truncate(l.trim(), 40)))
                .unwrap_or_default();
            let tags = if n.tags.is_empty() { String::new() } else { format!("#{} ", n.tags.join(" #")) };
            spread(
                vec![
                    Span::styled(" ", base),
                    Span::styled(n.title.clone(), base),
                    Span::styled(context, base.patch(faint).remove_modifier(Modifier::BOLD)),
                ],
                vec![
                    Span::styled(truncate(&tags, 24), base.fg(theme.facet_tag).remove_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {} ", n.folder()), base.patch(faint).remove_modifier(Modifier::BOLD)),
                ],
                width,
            )
            .style(base)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(h.min(area.height))]).flex(Flex::Center).areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(w.min(area.width))]).flex(Flex::Center).areas(row);
    rect
}

fn draw_popup(frame: &mut Frame, area: Rect, lib: &Library, filter: &SourceFilter, v: &NotesView, theme: &Theme) {
    let Some(popup) = &v.popup else { return };
    match popup {
        Popup::Prompt(kind, text) => {
            let (title, hint) = match kind {
                PromptKind::NewNote { folder } if folder.is_empty() => ("New note".to_string(), "name"),
                PromptKind::NewNote { folder } => (format!("New note in {folder}"), "name"),
                PromptKind::NewFolder { parent } if parent.is_empty() => ("New folder".to_string(), "name"),
                PromptKind::NewFolder { parent } => (format!("New folder in {parent}"), "name"),
                PromptKind::Rename { path } => (format!("Rename {path}"), "new name"),
                PromptKind::Delete { path } => (format!("Delete {path}?"), "enter or y deletes · esc keeps it"),
            };
            let rect = centered(area, 64, 6);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(format!(" {hint} "), theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])).block(block),
                rect,
            );
            frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
        }
        Popup::Search(query, selected) => draw_search(frame, area, v, query, *selected, theme),
        Popup::Command(cmd) => {
            // A vim-style command line over the editor's bottom border.
            let [_, mid, _, _, _] = Layout::horizontal([
                Constraint::Length(34),
                Constraint::Length(1),
                Constraint::Min(40),
                Constraint::Length(1),
                Constraint::Length(36),
            ])
            .areas(area);
            let line = Rect { x: mid.x + 1, y: mid.bottom().saturating_sub(1), width: mid.width.saturating_sub(2), height: 1 };
            frame.render_widget(Clear, line);
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(":", Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    Span::raw(cmd.clone()),
                ])),
                line,
            );
            frame.set_cursor_position((line.x + 1 + cmd.width() as u16, line.y));
        }
        Popup::CreateFromLink(p, _) => {
            let rect = centered(area, 72, area.height.saturating_sub(6).min(24));
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&p.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" type to filter · enter · esc ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, p, theme);
        }
        Popup::Preview(title, entity, lines) => {
            let rect = centered(area, 90, area.height.saturating_sub(6).min(30));
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" any key closes ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let body: Vec<Line> = match entity {
                Some(e) => render::render(&view::document(lib, *e, filter), inner.width, None, theme).lines,
                None => lines.iter().map(|l| Line::raw(l.clone())).collect(),
            };
            frame.render_widget(Paragraph::new(body), inner);
        }
    }
}
