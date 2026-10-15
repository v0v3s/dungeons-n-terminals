//! Drawing for the homebrew entry editor and the clone choice.

use dnt_data::{homebrew, view, Library, SourceFilter};
use edtui::{EditorMode, EditorTheme, EditorView};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph, Widget};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::CloneFlow;
use crate::entry_editor::{EntryEditor, Mode, Widget as FieldWidget};
use crate::render;
use crate::theme::Theme;
use crate::ui::{pane, pane_title, spread, truncate};

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(h.min(area.height))]).flex(Flex::Center).areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(w.min(area.width))]).flex(Flex::Center).areas(row);
    rect
}

/// A short one-line rendering of a JSON value.
fn preview(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) if a.iter().all(|x| x.is_string()) => {
            a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · ")
        }
        other => other.to_string(),
    }
}

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    lib: &Library,
    filter: &SourceFilter,
    homebrew_doc: Option<&serde_json::Value>,
    ed: &mut EntryEditor,
    theme: &Theme,
) {
    let id = ed.entity(lib);
    let entry = id
        .and_then(|id| homebrew_doc.and_then(|d| Some((d, homebrew::find_entry(d, lib, id)?))))
        .and_then(|(d, at)| homebrew::entry(d, &at).cloned());
    let fields = ed.fields(lib, entry.as_ref());
    ed.cursor = ed.cursor.min(fields.len().saturating_sub(1));
    let big = matches!(ed.mode, Mode::Big { .. });
    let left = if big { Constraint::Percentage(50) } else { Constraint::Length(70) };
    let [list, _, detail] = Layout::horizontal([left, Constraint::Length(1), Constraint::Min(30)]).areas(area);

    let name = id.map(|i| lib.entity(i).name.clone()).unwrap_or_else(|| "(reloading)".into());
    let what = if ed.replaces { "campaign version" } else { "campaign entry" };
    let block = pane(pane_title(&format!("Editing {name} · {what}"), true, theme), true, theme)
        .padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(list);
    frame.render_widget(block, list);
    let width = inner.width as usize;
    let mut lines = vec![Line::from(vec![
        Span::styled("● ", Style::new().fg(theme.accent)),
        Span::styled(
            if ed.replaces { "changed in this campaign (x reverts to the book)" } else { "stored in the campaign homebrew" },
            theme.dim,
        ),
    ])];
    lines.push(Line::default());
    for (i, f) in fields.iter().enumerate() {
        let base = if i == ed.cursor { theme.selected } else { Style::new() };
        let faint = if i == ed.cursor { theme.muted } else { theme.dim };
        let mark = if f.changed { Span::styled("● ", base.fg(theme.accent)) } else { Span::styled("  ", base) };
        let kind = match f.widget {
            FieldWidget::Entries => "text",
            FieldWidget::Json => "json",
            FieldWidget::Bool => "yes/no",
            FieldWidget::Number => "number",
            FieldWidget::Text => "",
        };
        let value = match (&f.value, f.widget) {
            (None, _) => "—".to_string(),
            (Some(v), FieldWidget::Entries) => homebrew::entries_to_markdown(v).split_whitespace().collect::<Vec<_>>().join(" "),
            (Some(v), _) => preview(v),
        };
        let value_w = width.saturating_sub(28);
        lines.push(
            spread(
                vec![
                    mark,
                    Span::styled(format!("{:<18}", truncate(&f.name, 17)), if f.changed || i == ed.cursor { base } else { base.patch(theme.label) }),
                    Span::styled(truncate(&value, value_w), base),
                ],
                vec![Span::styled(format!("{kind} "), base.patch(faint).remove_modifier(Modifier::BOLD))],
                width,
            )
            .style(base),
        );
    }
    if let Some(e) = &ed.error {
        lines.push(Line::default());
        lines.push(Line::styled(format!("⚠ {e}"), Style::new().fg(Color::Red)));
    }
    let height = inner.height as usize;
    let scroll = (ed.cursor + 3).saturating_sub(height.saturating_sub(1));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);

    // Live preview of the entity as the campaign sees it.
    let block = pane(pane_title("Preview", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(detail);
    frame.render_widget(block, detail);
    if let Some(id) = id {
        let mut lines = vec![Line::styled(lib.entity(id).name.clone(), theme.title), Line::default()];
        lines.extend(render::render(&view::document(lib, id, filter), inner.width.min(96), None, theme).lines);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    match &mut ed.mode {
        Mode::List => {}
        Mode::ConfirmDelete => {
            let rect = centered(area, 60, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title("Delete", true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            let what = if ed.replaces { "the campaign version (the book version returns)" } else { "this campaign entry" };
            frame.render_widget(Paragraph::new(Line::raw(format!("Delete {what}?  y / n"))).block(block), rect);
        }
        Mode::Prompt { .. } | Mode::AddField(_) => {
            let (title, text) = match &ed.mode {
                Mode::Prompt { field, text } => (field.clone(), text.clone()),
                Mode::AddField(text) => ("New field name".to_string(), text.clone()),
                _ => unreachable!(),
            };
            let rect = centered(area, 72, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&title, true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])).block(block),
                rect,
            );
            frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
        }
        Mode::Big { field, editor, json } => {
            let rect = list;
            frame.render_widget(Clear, rect);
            let mode = match editor.mode {
                EditorMode::Insert => " INSERT ",
                EditorMode::Visual => " VISUAL ",
                EditorMode::Search => " SEARCH ",
                EditorMode::Normal => " NORMAL ",
            };
            let hint = if *json { "JSON" } else { "Markdown: **bold** *italic* [[spell:Fireball]] ## Section - list" };
            let mut block = pane(pane_title(&format!("{field} · {hint}"), true, theme), true, theme)
                .padding(Padding::new(1, 1, 1, 0))
                .title_bottom(Line::from(vec![Span::styled(" esc saves ", theme.dim), Span::styled(mode, theme.keycap)]).right_aligned());
            if let Some(e) = &ed.error {
                block = block.title_bottom(Line::styled(format!(" ⚠ {} ", truncate(e, rect.width.saturating_sub(24) as usize)), Style::new().fg(Color::Red)));
            }
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            let theme_ed = EditorTheme::default()
                .base(Style::new())
                .cursor_style(Style::new().add_modifier(Modifier::REVERSED))
                .selection_style(Style::new().bg(Color::DarkGray))
                .hide_status_line();
            EditorView::new(editor).theme(theme_ed).wrap(true).render(inner, frame.buffer_mut());
        }
    }
}

pub fn draw_clone(frame: &mut Frame, area: Rect, lib: &Library, flow: &CloneFlow, theme: &Theme) {
    let e = lib.entity(flow.entity);
    let naming = flow.name.is_some();
    let rect = centered(area, 78, if naming { 6 } else { 9 });
    frame.render_widget(Clear, rect);
    let hint = if naming { " enter creates · esc cancels " } else { " j/k choose · enter · esc cancel " };
    let block = pane(pane_title(&format!("Clone {} into the campaign", e.name), true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(hint, theme.dim).right_aligned());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if let Some(name) = &flow.name {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Name of the copy", theme.label),
                Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(name.clone())]),
            ]),
            inner,
        );
        frame.set_cursor_position((inner.x + 2 + name.width() as u16, inner.y + 1));
        return;
    }
    let option = |i: usize, title: &str, desc: &str| {
        let base = if flow.cursor == i { theme.selected } else { Style::new() };
        vec![
            Line::from(vec![Span::styled(if flow.cursor == i { "▸ " } else { "  " }, base.fg(theme.accent)), Span::styled(title.to_string(), base)]).style(base),
            Line::styled(format!("    {desc}"), theme.dim),
            Line::default(),
        ]
    };
    let mut lines = option(0, "Replace the original in this campaign", "Characters, links and pickers use the campaign version everywhere.");
    lines.extend(option(1, "Separate copy…", "A new entry under its own name; the original stays."));
    lines.pop();
    frame.render_widget(Paragraph::new(lines), inner);
}
