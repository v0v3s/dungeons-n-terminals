//! Drawing for the Characters section: roster, character sheet and popups.

use dnt_data::{view, Library};
use dnt_rules::builder::{build, FeatureSource};
use dnt_rules::engine::PartKind;
use dnt_rules::overlay::{OverlaySet, Recharge};
use dnt_rules::play;
use dnt_rules::standard::{ABILITIES, SKILLS};
use dnt_rules::{Evaluation, Value};
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::characters::{Characters, Item, Popup, Rest, Sheet, Tab};
use crate::render;
use crate::theme::Theme;
use crate::ui::{pane, pane_title, spread, truncate};

pub fn draw(frame: &mut Frame, area: Rect, lib: &Library, overlays: &OverlaySet, c: &mut Characters, theme: &Theme, campaign: Option<&str>) {
    if let Some(w) = &c.wizard {
        return draw_wizard(frame, area, lib, overlays, w, theme);
    }
    match c.sheet.as_mut() {
        Some(sheet) => draw_sheet(frame, area, lib, overlays, sheet, theme),
        None => draw_roster(frame, area, lib, overlays, c, theme, campaign),
    }
}

// ---- roster ----

fn draw_roster(
    frame: &mut Frame,
    area: Rect,
    lib: &Library,
    overlays: &OverlaySet,
    c: &mut Characters,
    theme: &Theme,
    campaign: Option<&str>,
) {
    let [left, _, right] =
        Layout::horizontal([Constraint::Length(40), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let block = pane(pane_title("Characters", true, theme), true, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(left);
    frame.render_widget(block, left);

    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    if campaign.is_none() {
        lines.push(Line::styled("No active campaign.", theme.muted));
        lines.push(Line::styled("Create one in 4 Campaigns.", theme.dim));
    }
    for (i, f) in c.files.iter().enumerate() {
        let base = if i == c.selected { theme.selected } else { Style::new() };
        let level = f.character.total_level();
        let line = spread(
            vec![Span::styled("◆ ", base.fg(theme.accent)), Span::styled(f.character.name.clone(), base)],
            vec![Span::styled(format!("Lv {level} "), base.patch(theme.muted).remove_modifier(Modifier::BOLD))],
            width,
        );
        lines.push(line.style(base));
    }
    if c.files.is_empty() && campaign.is_some() && c.name_input.is_none() {
        lines.push(Line::styled("No characters yet.", theme.muted));
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Press ", theme.dim),
            Span::styled(" n ", theme.keycap),
            Span::styled(" to create one.", theme.dim),
        ]));
    }
    if let Some(name) = &c.name_input {
        lines.push(Line::default());
        lines.push(Line::styled("New character", theme.label));
        lines.push(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(name.clone())]));
        let y = (inner.y + lines.len() as u16 - 1).min(inner.bottom().saturating_sub(1));
        frame.set_cursor_position((inner.x + 2 + name.width() as u16, y));
    }
    frame.render_widget(Paragraph::new(lines), inner);

    // Preview card.
    let block = pane(pane_title("Preview", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(right);
    frame.render_widget(block, right);
    let Some(file) = c.files.get(c.selected) else { return };
    let b = build(&file.character, lib, overlays);
    let ev = b.evaluate();
    let ch = &file.character;
    let mut lines = vec![
        Line::styled(ch.name.clone(), theme.title),
        Line::styled(identity(lib, ch, &b.class_summary()), theme.muted),
        Line::default(),
    ];
    let hp = format!("{}/{}", play::current_hp(ch, &ev), play::max_hp(&ev));
    for (label, value) in [
        ("Hit Points", hp),
        ("Armor Class", stat(&ev, "ac")),
        ("Initiative", signed(&ev, "initiative")),
        ("Speed", format!("{} ft.", stat(&ev, "speed"))),
        ("Passive Perception", stat(&ev, "passive.perception")),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<20}"), theme.label),
            Span::styled(value, Style::new().add_modifier(Modifier::BOLD)),
        ]));
    }
    lines.push(Line::default());
    let abilities: Vec<Span> = ABILITIES
        .iter()
        .flat_map(|(a, _)| {
            vec![
                Span::styled(format!("{} ", a.to_uppercase()), theme.label),
                Span::styled(format!("{:<4}", stat(&ev, a)), Style::new().add_modifier(Modifier::BOLD)),
            ]
        })
        .collect();
    lines.push(Line::from(abilities));
    if !b.warnings.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(format!("⚠ {} warning(s) — open to see", b.warnings.len()), Style::new().fg(Color::Yellow)));
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(" enter ", theme.keycap),
        Span::styled(" open sheet   ", theme.dim),
        Span::styled(" e ", theme.keycap),
        Span::styled(" edit file", theme.dim),
    ]));
    frame.render_widget(Paragraph::new(lines), inner);
}

fn identity(lib: &Library, ch: &dnt_rules::character::Character, classes: &str) -> String {
    let name_of = |id: &Option<String>| {
        id.as_deref()
            .and_then(|id| dnt_rules::overlay::resolve_id(lib, id))
            .map(|e| lib.entity(e).name.clone())
    };
    let mut parts: Vec<String> = [name_of(&ch.species), name_of(&ch.background)].into_iter().flatten().collect();
    if !classes.is_empty() {
        parts.push(classes.to_string());
    }
    parts.push(format!("Level {}", ch.total_level()));
    parts.join(" · ")
}

fn stat(ev: &Evaluation, key: &str) -> String {
    match ev.get(key) {
        Some(v) => v.to_string(),
        None => "—".into(),
    }
}

fn signed(ev: &Evaluation, key: &str) -> String {
    match ev.get(key) {
        Some(Value::Num(n)) if n.is_int() => format!("{:+}", n.floor()),
        Some(v) => v.to_string(),
        None => "—".into(),
    }
}

// ---- sheet ----

fn draw_sheet(frame: &mut Frame, area: Rect, lib: &Library, overlays: &OverlaySet, sheet: &mut Sheet, theme: &Theme) {
    let [header, tabs, _, body] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
    ])
    .areas(area);

    let ch = sheet.character();
    let mut warn = Vec::new();
    if !sheet.pending.is_empty() {
        warn.push(Span::styled(format!(" {} choice(s) to make ", sheet.pending.len()), theme.category_active));
        warn.push(Span::styled("  press ", theme.dim));
        warn.push(Span::styled(" c ", theme.keycap));
        warn.push(Span::raw("   "));
    }
    if !sheet.build.warnings.is_empty() {
        warn.push(Span::styled(format!("⚠ {} warning(s) ", sheet.build.warnings.len()), Style::new().fg(Color::Yellow)));
    }
    frame.render_widget(
        Paragraph::new(vec![
            spread(
                vec![Span::styled(" ◆ ", Style::new().fg(theme.accent)), Span::styled(ch.name.clone(), theme.title)],
                warn,
                header.width as usize,
            ),
            if ch.levels.is_empty() {
                Line::from(vec![
                    Span::styled("   No class yet — press ", theme.muted),
                    Span::styled(" u ", theme.keycap),
                    Span::styled(" to take your first level, ", theme.muted),
                    Span::styled(" e ", theme.keycap),
                    Span::styled(" to set abilities, species and background", theme.muted),
                ])
            } else {
                Line::styled(format!("   {}", identity(lib, ch, &sheet.build.class_summary())), theme.muted)
            },
        ]),
        header,
    );

    let mut spans = vec![Span::raw(" ")];
    for t in Tab::ALL {
        let style = if t == sheet.tab { theme.category_active } else { theme.category_inactive };
        spans.push(Span::styled(format!(" {} ", t.label()), style));
        spans.push(Span::raw(" "));
    }
    for soon in ["Spells", "Notes"] {
        spans.push(Span::styled(format!(" {soon} "), theme.dim));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs);

    match sheet.tab {
        Tab::Overview => draw_overview(frame, body, sheet, theme),
        Tab::Features => draw_features(frame, body, lib, overlays, sheet, theme),
        Tab::Inventory => draw_inventory(frame, body, lib, sheet, theme),
    }

    match sheet.popup.clone() {
        Some(Popup::Breakdown(key)) => draw_breakdown(frame, area, sheet, &key, theme),
        Some(Popup::Input(prompt, text)) => {
            let rect = centered(area, 34, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(prompt.label(), true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())]))
                    .block(block),
                rect,
            );
            frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
        }
        Some(Popup::Confirm(rest)) => {
            let rect = centered(area, 40, 5);
            frame.render_widget(Clear, rect);
            let what = if rest == Rest::Short { "short" } else { "long" };
            let block = pane(pane_title("Rest", true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(format!("Take a {what} rest?  ")),
                    Span::styled(" y ", theme.keycap),
                    Span::styled(" yes  ", theme.dim),
                    Span::styled(" n ", theme.keycap),
                    Span::styled(" no", theme.dim),
                ]))
                .block(block),
                rect,
            );
        }
        Some(Popup::LevelUp(flow)) => draw_levelup(frame, area, &flow, theme),
        Some(Popup::AddItem(picker)) => {
            let rect = centered(area, 76.min(area.width), area.height.saturating_sub(4).min(26));
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&picker.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" type to filter · enter add · esc cancel ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, &picker, theme);
        }
        Some(Popup::Text(prompt, text)) => {
            let rect = centered(area, 56, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(prompt.label(), true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())]))
                    .block(block),
                rect,
            );
            frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
        }
        Some(Popup::Fill(fill)) => {
            let rect = centered(area, 76.min(area.width), area.height.saturating_sub(4).min(26));
            frame.render_widget(Clear, rect);
            let remaining = fill.queue.len();
            let block = pane(pane_title(&fill.picker.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(
                    Line::styled(
                        format!(" {remaining} choice(s) left · enter choose · esc stop "),
                        theme.dim,
                    )
                    .right_aligned(),
                );
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, &fill.picker, theme);
        }
        Some(Popup::Undo) => {
            let rect = centered(area, 52, 5);
            frame.render_widget(Clear, rect);
            let last = sheet
                .character()
                .levels
                .last()
                .map(|l| l.class.split_once(':').map_or("", |(_, r)| r.split('|').next().unwrap_or(r)).to_string())
                .unwrap_or_default();
            let block = pane(pane_title("Undo level", true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(format!("Remove the latest level ({last})?  ")),
                    Span::styled(" y ", theme.keycap),
                    Span::styled(" yes  ", theme.dim),
                    Span::styled(" n ", theme.keycap),
                    Span::styled(" no", theme.dim),
                ]))
                .block(block),
                rect,
            );
        }
        None => {}
    }
}

fn draw_levelup(frame: &mut Frame, area: Rect, flow: &crate::levelup::LevelUp, theme: &Theme) {
    use crate::levelup::Stage;
    let rect = centered(area, 76.min(area.width), area.height.saturating_sub(4).min(26));
    frame.render_widget(Clear, rect);
    let title = match &flow.stage {
        Stage::Pick(p) => p.title.clone(),
        Stage::Roll(_) => "Hit points for this level".into(),
        Stage::Confirm => "Confirm level up".into(),
    };
    let hints = match &flow.stage {
        Stage::Pick(_) => "type to filter · ↑↓ move · enter choose · esc cancel",
        Stage::Roll(_) => "enter the rolled value · esc cancel",
        Stage::Confirm => "enter apply · esc cancel",
    };
    let block = pane(pane_title(&title, true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(format!(" {hints} "), theme.dim).right_aligned());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let summary: Vec<Line> = flow
        .summary
        .iter()
        .map(|s| Line::from(vec![Span::styled("✓ ", Style::new().fg(theme.accent)), Span::styled(s.clone(), theme.muted)]))
        .collect();
    let summary_h = summary.len() as u16 + u16::from(!summary.is_empty());
    let [top, body] = Layout::vertical([Constraint::Length(summary_h), Constraint::Min(1)]).areas(inner);
    frame.render_widget(Paragraph::new(summary), top);

    match &flow.stage {
        Stage::Confirm => {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(format!("This becomes character level {}.", flow_total_hint(flow)), Style::new()),
                    Line::default(),
                    Line::from(vec![
                        Span::styled(" enter ", theme.keycap),
                        Span::styled(" apply   ", theme.dim),
                        Span::styled(" esc ", theme.keycap),
                        Span::styled(" cancel", theme.dim),
                    ]),
                ]),
                body,
            );
        }
        Stage::Roll(text) => {
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])),
                body,
            );
            frame.set_cursor_position((body.x + 2 + text.width() as u16, body.y));
        }
        Stage::Pick(p) => draw_picker(frame, body, p, theme),
    }
}

/// A filter line, a rule and the (optionally multi-select) item list.
pub(crate) fn draw_picker(frame: &mut Frame, body: Rect, p: &crate::picker::Picker, theme: &Theme) {
    let [input, rule, list] = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)]).areas(body);
    let shown = if p.query.is_empty() {
        Span::styled("type to filter", theme.dim)
    } else {
        Span::raw(p.query.clone())
    };
    let mut input_spans = vec![Span::styled("❯ ", Style::new().fg(theme.accent)), shown];
    if let Some(n) = p.multi {
        input_spans.push(Span::raw("   "));
        input_spans.push(Span::styled(
            format!("{}/{n} chosen · space toggles", p.checked.len()),
            if p.checked.len() == n { Style::new().fg(theme.accent) } else { theme.dim },
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(input_spans)), input);
    frame.set_cursor_position((input.x + 2 + p.query.width() as u16, input.y));
    frame.render_widget(Paragraph::new(Line::styled("─".repeat(rule.width as usize), theme.border)), rule);

    let visible = p.visible();
    let height = list.height as usize;
    let offset = p.selected.saturating_sub(height.saturating_sub(1));
    let width = list.width as usize;
    let lines: Vec<Line> = visible
        .iter()
        .enumerate()
        .skip(offset)
        .take(height)
        .map(|(pos, &i)| {
            let item = &p.items[i];
            let base = if pos == p.selected { theme.selected } else { Style::new() };
            let mut left = Vec::new();
            if p.multi.is_some() {
                let on = p.is_checked(&item.value);
                left.push(Span::styled(if on { " ● " } else { " ○ " }, if on { base.fg(theme.accent) } else { base.patch(theme.muted) }));
            } else {
                left.push(Span::styled(" ", base));
            }
            left.push(Span::styled(item.label.clone(), base));
            let mut right = Vec::new();
            if let Some(w) = &item.warning {
                right.push(Span::styled(format!("⚠ {} ", truncate(w, 36)), base.fg(Color::Yellow).remove_modifier(Modifier::BOLD)));
            }
            right.push(Span::styled(
                format!("{} ", item.hint),
                base.patch(if pos == p.selected { theme.muted } else { theme.dim }).remove_modifier(Modifier::BOLD),
            ));
            spread(left, right, width).style(base)
        })
        .collect();
    if lines.is_empty() {
        frame.render_widget(Paragraph::new(Line::styled("Nothing matches.", theme.dim)), list);
    } else {
        frame.render_widget(Paragraph::new(lines), list);
    }
}

fn flow_total_hint(flow: &crate::levelup::LevelUp) -> String {
    match &flow.class {
        Some(_) => format!("{} in this class", flow.new_level),
        None => "?".into(),
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(h)]).flex(Flex::Center).areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(w.min(area.width))]).flex(Flex::Center).areas(row);
    rect
}

/// Row style for an item: highlighted under the cursor.
fn row_style(sheet: &Sheet, item: &Item, theme: &Theme) -> Style {
    if sheet.current_item().as_ref() == Some(item) && sheet.popup.is_none() {
        theme.selected
    } else {
        Style::new()
    }
}

fn pips(left: i64, max: i64, theme: &Theme, base: Style) -> Vec<Span<'static>> {
    let shown = max.min(12);
    let mut spans = vec![
        Span::styled("●".repeat(left.min(shown) as usize), base.fg(theme.accent)),
        Span::styled("○".repeat((shown - left.min(shown)).max(0) as usize), base.patch(theme.dim)),
    ];
    if max > shown {
        spans.push(Span::styled("…", base.patch(theme.dim)));
    }
    spans
}

fn draw_overview(frame: &mut Frame, area: Rect, sheet: &Sheet, theme: &Theme) {
    let [abil, _, mid, _, skills] = Layout::horizontal([
        Constraint::Length(34),
        Constraint::Length(1),
        Constraint::Min(40),
        Constraint::Length(1),
        Constraint::Length(34),
    ])
    .areas(area);
    let ev = &sheet.ev;

    // Abilities and saves.
    let block = pane(pane_title("Abilities", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(abil);
    frame.render_widget(block, abil);
    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:<13}", ""), theme.label),
        Span::styled("score  mod save", theme.label),
    ])];
    for (a, name) in ABILITIES {
        let item = Item::Stat(a.to_string());
        let base = row_style(sheet, &item, theme);
        let proficient = ev.get(&format!("save.{a}")) != ev.get(&format!("mod.{a}"));
        lines.push(
            Line::from(vec![
                Span::styled(format!("{name:<13}"), base),
                Span::styled(format!("{:>5}", stat(ev, a)), base),
                Span::styled(format!("{:>5}", signed(ev, &format!("mod.{a}"))), base.fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:>5} ", signed(ev, &format!("save.{a}"))), base),
                Span::styled(if proficient { "●" } else { "○" }, if proficient { base.fg(theme.accent) } else { base.patch(theme.dim) }),
            ])
            .style(base),
        );
    }
    lines.push(Line::default());
    // Spellcasting per class.
    let casters: Vec<_> = sheet.build.classes.iter().filter(|c| c.casting_ability.is_some()).collect();
    if !casters.is_empty() {
        lines.push(Line::styled("Spellcasting", theme.heading));
        for c in casters {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<12}", truncate(&c.name, 11)), theme.muted),
                Span::styled("DC ", theme.label),
                Span::styled(format!("{:<4}", stat(ev, &format!("spell.{}.dc", c.slug))), Style::new().add_modifier(Modifier::BOLD)),
                Span::styled("atk ", theme.label),
                Span::styled(signed(ev, &format!("spell.{}.attack", c.slug)), Style::new().add_modifier(Modifier::BOLD)),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);

    // Skills.
    let block = pane(pane_title("Skills", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(skills);
    frame.render_widget(block, skills);
    let width = inner.width as usize;
    let lines: Vec<Line> = SKILLS
        .iter()
        .map(|(k, name, ab)| {
            let item = Item::Stat(format!("skill.{k}"));
            let base = row_style(sheet, &item, theme);
            let mult = sheet.build.model.inputs.get(&format!("prof.skill.{k}")).cloned().unwrap_or(Value::int(0));
            let (mark, style) = match mult.to_string().as_str() {
                "2" => ("◉", base.fg(theme.accent)),
                "1" => ("●", base.fg(theme.accent)),
                "0" => ("○", base.patch(theme.dim)),
                _ => ("◐", base.fg(theme.accent)),
            };
            spread(
                vec![
                    Span::styled(format!("{mark} "), style),
                    Span::styled(name.to_string(), base),
                    Span::styled(format!(" {}", ab.to_uppercase()), base.patch(theme.dim).remove_modifier(Modifier::BOLD)),
                ],
                vec![Span::styled(signed(ev, &format!("skill.{k}")), base.add_modifier(Modifier::BOLD))],
                width,
            )
            .style(base)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);

    // Middle: combat tiles, HP, resources.
    let [tiles, _, rest] = Layout::vertical([Constraint::Length(5), Constraint::Length(1), Constraint::Min(3)]).areas(mid);
    let tile_keys = [
        ("ac", "Armor Class", false),
        ("initiative", "Initiative", true),
        ("speed", "Speed", false),
        ("prof", "Proficiency", true),
        ("passive.perception", "Passive Perc.", false),
    ];
    let cells = Layout::horizontal(tile_keys.iter().map(|_| Constraint::Ratio(1, tile_keys.len() as u32))).split(tiles);
    for ((key, label, sign), cell) in tile_keys.iter().zip(cells.iter()) {
        let item = Item::Stat(key.to_string());
        let focused = sheet.current_item().as_ref() == Some(&item) && sheet.popup.is_none();
        let block = pane(Line::default(), focused, theme).padding(Padding::ZERO);
        let inner = block.inner(*cell);
        frame.render_widget(block, *cell);
        let value = if *sign { signed(ev, key) } else { stat(ev, key) };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(value, Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)).centered(),
                Line::default(),
                Line::styled(label.to_string(), theme.label).centered(),
            ]),
            inner,
        );
    }

    let block = pane(pane_title("Tracking", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(rest);
    frame.render_widget(block, rest);
    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();

    // Hit points bar.
    let ch = sheet.character();
    let (hp, max) = (play::current_hp(ch, ev), play::max_hp(ev));
    let item = Item::Stat("hp.max".into());
    let base = row_style(sheet, &item, theme);
    let bar_w = width.saturating_sub(28).clamp(10, 40);
    let filled = ((hp as f64 / max as f64) * bar_w as f64).round() as usize;
    let hp_color = match hp * 100 / max {
        0..=25 => Color::Red,
        26..=50 => Color::Yellow,
        _ => Color::Green,
    };
    let mut hp_line = vec![
        Span::styled(format!("{:<12}", "Hit Points"), base.patch(theme.label)),
        Span::styled("█".repeat(filled), base.fg(hp_color)),
        Span::styled("░".repeat(bar_w - filled.min(bar_w)), base.patch(theme.dim)),
        Span::styled(format!("  {hp}"), base.add_modifier(Modifier::BOLD)),
        Span::styled(format!("/{max}"), base.patch(theme.muted)),
    ];
    if ch.state.temp_hp > 0 {
        hp_line.push(Span::styled(format!("  +{} temp", ch.state.temp_hp), base.fg(Color::Cyan)));
    }
    lines.push(Line::from(hp_line).style(base));
    if hp == 0 {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "Death saves"), theme.label),
            Span::styled("✓".repeat(ch.state.death_successes as usize), Style::new().fg(Color::Green)),
            Span::styled("  ✗".repeat(ch.state.death_failures as usize), Style::new().fg(Color::Red)),
        ]));
    }
    lines.push(Line::default());

    let section = |lines: &mut Vec<Line>, title: &str| {
        if lines.last().is_some_and(|l| !l.spans.is_empty()) {
            lines.push(Line::default());
        }
        lines.push(Line::styled(title.to_string(), theme.heading));
    };

    if !sheet.build.resources.is_empty() {
        section(&mut lines, "Resources");
        for (r, _) in &sheet.build.resources {
            let item = Item::Resource(r.key.clone());
            let base = row_style(sheet, &item, theme);
            let (left, max) = (play::resource_left(ch, ev, &r.key), play::resource_max(ev, &r.key));
            let recharge = match (r.recharge, r.short_rest_regain.is_some()) {
                (Recharge::ShortRest, _) => "short rest",
                (Recharge::LongRest, true) => "long · 1 on short",
                (Recharge::LongRest, false) => "long rest",
                (Recharge::Dawn, _) => "dawn",
                (Recharge::Manual, _) => "manual",
            };
            let mut left_spans = vec![Span::styled(format!("{:<22}", truncate(&r.name, 21)), base)];
            if max <= 12 {
                left_spans.extend(pips(left, max, theme, base));
            } else {
                left_spans.push(Span::styled(format!("{left}"), base.fg(theme.accent).add_modifier(Modifier::BOLD)));
            }
            lines.push(
                spread(
                    left_spans,
                    vec![
                        Span::styled(format!("{left}/{max}  "), base),
                        Span::styled(recharge, base.patch(theme.dim).remove_modifier(Modifier::BOLD)),
                    ],
                    width,
                )
                .style(base),
            );
        }
    }

    let slot_levels: Vec<usize> = (1..=9).filter(|l| play::slot_max(ev, *l) > 0).collect();
    let pact = ev.int("pact.slots.max").unwrap_or(0);
    if !slot_levels.is_empty() || pact > 0 {
        section(&mut lines, "Spell slots");
        for l in slot_levels {
            let item = Item::Slot(l);
            let base = row_style(sheet, &item, theme);
            let (left, max) = (play::slots_left(ch, ev, l), play::slot_max(ev, l));
            let mut spans = vec![Span::styled(format!("{:<22}", format!("Level {l}")), base)];
            spans.extend(pips(left, max, theme, base));
            lines.push(spread(spans, vec![Span::styled(format!("{left}/{max}"), base)], width).style(base));
        }
        if pact > 0 {
            let base = row_style(sheet, &Item::Pact, theme);
            let left = pact - ch.state.pact_slots_used;
            let level = sheet.build.model.inputs.get("pact.level").map(|v| v.to_string()).unwrap_or_default();
            let mut spans = vec![Span::styled(format!("{:<22}", format!("Pact (level {level})")), base)];
            spans.extend(pips(left, pact, theme, base));
            lines.push(
                spread(spans, vec![Span::styled(format!("{left}/{pact}  "), base), Span::styled("short rest", base.patch(theme.dim))], width)
                    .style(base),
            );
        }
    }

    let dice: Vec<i64> = sheet
        .items()
        .into_iter()
        .filter_map(|i| if let Item::HitDie(f) = i { Some(f) } else { None })
        .collect();
    if !dice.is_empty() {
        section(&mut lines, "Hit dice");
        for f in dice {
            let item = Item::HitDie(f);
            let base = row_style(sheet, &item, theme);
            let max = ev.int(&format!("hit_dice.d{f}.max")).unwrap_or(0);
            let left = max - ch.state.hit_dice_used.get(&format!("d{f}")).copied().unwrap_or(0);
            let mut spans = vec![Span::styled(format!("{:<22}", format!("d{f}")), base)];
            spans.extend(pips(left, max, theme, base));
            lines.push(spread(spans, vec![Span::styled(format!("{left}/{max}"), base)], width).style(base));
        }
    }

    if !sheet.build.toggles.is_empty() {
        section(&mut lines, "Toggles");
        for (t, _) in &sheet.build.toggles {
            let item = Item::Toggle(t.key.clone());
            let base = row_style(sheet, &item, theme);
            let on = ch.state.toggles.get(&t.key).copied().unwrap_or(false);
            lines.push(
                Line::from(vec![
                    Span::styled(if on { "◉ " } else { "○ " }, if on { base.fg(theme.accent) } else { base.patch(theme.dim) }),
                    Span::styled(t.name.clone(), base),
                ])
                .style(base),
            );
        }
    }

    if !sheet.build.warnings.is_empty() {
        section(&mut lines, "Warnings");
        for w in &sheet.build.warnings {
            lines.push(Line::styled(format!("⚠ {w}"), Style::new().fg(Color::Yellow)));
        }
    }

    // Keep the cursor row visible.
    let height = inner.height as usize;
    let cursor_line = lines.iter().position(|l| l.style == theme.selected).unwrap_or(0);
    let scroll = cursor_line.saturating_sub(height.saturating_sub(2));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

fn draw_features(frame: &mut Frame, area: Rect, lib: &Library, overlays: &OverlaySet, sheet: &mut Sheet, theme: &Theme) {
    let [list, _, detail] =
        Layout::horizontal([Constraint::Length(42), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let block = pane(pane_title("Features", true, theme), true, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(list);
    frame.render_widget(block, list);

    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut last_group = String::new();
    let mut cursor_line = 0;
    for (i, f) in sheet.build.features.iter().enumerate() {
        let group = match f.source {
            FeatureSource::Class(ci) => {
                let c = &sheet.build.classes[ci];
                format!("{} {}", c.name, c.level)
            }
            FeatureSource::Subclass(ci) => sheet.build.classes[ci]
                .subclass
                .map(|s| lib.entity(s).name.clone())
                .unwrap_or_else(|| "Subclass".into()),
            FeatureSource::Species => "Species".into(),
            FeatureSource::Background => "Background".into(),
            FeatureSource::Feat => "Feats".into(),
            FeatureSource::Option => "Options".into(),
            FeatureSource::Mastery => "Weapon Mastery".into(),
        };
        if group != last_group {
            if !lines.is_empty() {
                lines.push(Line::default());
            }
            lines.push(Line::styled(group.clone(), theme.heading));
            last_group = group;
        }
        let base = if i == sheet.feature_cursor { theme.selected } else { Style::new() };
        if i == sheet.feature_cursor {
            cursor_line = lines.len();
        }
        let automated = !overlays.for_entity(&lib.entity(f.entity).key).is_empty();
        let level = if f.level > 0 { format!("{:>2} ", f.level) } else { "   ".into() };
        lines.push(
            spread(
                vec![
                    Span::styled(
                        level,
                        base.patch(if i == sheet.feature_cursor { theme.muted } else { theme.dim }).remove_modifier(Modifier::BOLD),
                    ),
                    Span::styled(truncate(&lib.entity(f.entity).name, width.saturating_sub(6)), base),
                ],
                vec![Span::styled(if automated { "auto" } else { "" }, base.fg(theme.accent).remove_modifier(Modifier::BOLD))],
                width,
            )
            .style(base),
        );
    }
    let height = inner.height as usize;
    let scroll = cursor_line.saturating_sub(height.saturating_sub(3));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);

    let Some(f) = sheet.build.features.get(sheet.feature_cursor) else { return };
    let e = lib.entity(f.entity);
    let block = pane(pane_title(&e.name, false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(detail);
    frame.render_widget(block, detail);
    let doc = view::document(lib, f.entity, &Default::default());
    let rendered = render::render(&doc, inner.width.min(96), None, theme);
    let max_scroll = rendered.lines.len().saturating_sub(inner.height as usize);
    sheet.feature_scroll = sheet.feature_scroll.min(max_scroll);
    frame.render_widget(Paragraph::new(rendered.lines).scroll((sheet.feature_scroll as u16, 0)), inner);
}

fn draw_inventory(frame: &mut Frame, area: Rect, lib: &Library, sheet: &mut Sheet, theme: &Theme) {
    use crate::characters::{item_entity, line_name};
    use dnt_rules::equipment;
    let [list, _, detail] =
        Layout::horizontal([Constraint::Length(58), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let ch = &sheet.file.character;
    let block = pane(pane_title("Inventory", true, theme), true, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(list);
    frame.render_widget(block, list);
    let [items_area, totals] = Layout::vertical([Constraint::Min(3), Constraint::Length(6)]).areas(inner);
    let width = items_area.width as usize;

    let n = ch.inventory.len();
    sheet.inventory_cursor = sheet.inventory_cursor.min(n.saturating_sub(1));
    let mut lines: Vec<Line> = vec![Line::from(vec![
        Span::styled(format!("{:<width$}", "   item", width = width.saturating_sub(16)), theme.label),
        Span::styled(format!("{:>5}{:>10} ", "qty", "lb"), theme.label),
    ])];
    if n == 0 {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Empty. Press ", theme.dim),
            Span::styled(" a ", theme.keycap),
            Span::styled(" to add an item.", theme.dim),
        ]));
    }
    let mut total_weight = 0.0;
    for (i, line) in ch.inventory.iter().enumerate() {
        let w = equipment::weight(lib, line) * line.quantity as f64;
        total_weight += w;
        let base = if i == sheet.inventory_cursor { theme.selected } else { Style::new() };
        let faint = if i == sheet.inventory_cursor { theme.muted } else { theme.dim };
        let entity = item_entity(lib, line);
        let needs_attune = entity.is_some_and(|e| equipment::requires_attunement(&lib.entity(e).data));
        let mark = match (line.equipped, line.attuned) {
            (_, true) => Span::styled("★ ", base.fg(theme.accent)),
            (true, _) => Span::styled("◉ ", base.fg(theme.accent)),
            _ if needs_attune => Span::styled("☆ ", base.patch(faint)),
            _ => Span::styled("  ", base),
        };
        let kind = entity.and_then(|e| equipment::armor_kind(&lib.entity(e).data)).map(|k| format!(" {k}")).unwrap_or_default();
        let weight = if w > 0.0 { format!("{w:.1}") } else { "—".into() };
        lines.push(
            spread(
                vec![
                    Span::raw(" "),
                    mark,
                    Span::styled(truncate(&line_name(lib, line), width.saturating_sub(26)), base),
                    Span::styled(kind, base.patch(faint).remove_modifier(Modifier::BOLD)),
                ],
                vec![Span::styled(format!("{:>5}{weight:>10} ", line.quantity), base)],
                width,
            )
            .style(base),
        );
    }
    let height = items_area.height as usize;
    let scroll = (sheet.inventory_cursor + 1).saturating_sub(height.saturating_sub(1));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), items_area);

    // Totals: weight, attunement, coins.
    let capacity = sheet.ev.int("str").unwrap_or(10) * 15;
    let attuned = ch.inventory.iter().filter(|l| l.attuned).count();
    let c = &ch.currency;
    let coin = |n: i64, label: &str, color: Color| {
        vec![Span::styled(format!("{n}"), Style::new().fg(color).add_modifier(Modifier::BOLD)), Span::styled(format!(" {label}   "), theme.label)]
    };
    let mut coins = Vec::new();
    for (n, label, color) in [(c.pp, "pp", Color::White), (c.gp, "gp", Color::Yellow), (c.ep, "ep", Color::Gray), (c.sp, "sp", Color::Gray), (c.cp, "cp", Color::Red)] {
        if n != 0 || label == "gp" {
            coins.extend(coin(n, label, color));
        }
    }
    let over = total_weight > capacity as f64;
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("─".repeat(width), theme.border),
            Line::from(vec![
                Span::styled("Carried    ", theme.label),
                Span::styled(format!("{total_weight:.1}"), if over { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::BOLD) }),
                Span::styled(format!(" / {capacity} lb"), theme.muted),
            ]),
            Line::from(vec![
                Span::styled("Attunement ", theme.label),
                Span::styled(format!("{attuned}"), if attuned > 3 { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::BOLD) }),
                Span::styled(" / 3", theme.muted),
            ]),
            Line::from([vec![Span::styled("Coins      ", theme.label)], coins].concat()),
            Line::from(vec![
                Span::styled("◉ equipped  ★ attuned  ☆ needs attunement", theme.dim),
            ]),
        ]),
        totals,
    );

    // Detail of the selected item.
    let line = ch.inventory.get(sheet.inventory_cursor);
    let title = line.map(|l| line_name(lib, l)).unwrap_or_else(|| "Item".into());
    let block = pane(pane_title(&title, false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(detail);
    frame.render_widget(block, detail);
    match line.and_then(|l| item_entity(lib, l)) {
        Some(e) => {
            let doc = view::document(lib, e, &Default::default());
            frame.render_widget(Paragraph::new(render::render(&doc, inner.width.min(96), None, theme).lines), inner);
        }
        None if line.is_some() => {
            frame.render_widget(Paragraph::new(Line::styled("A custom item.", theme.dim)), inner);
        }
        None => {}
    }
}

fn stat_label(sheet: &Sheet, key: &str) -> String {
    if let Some((_, name)) = ABILITIES.iter().find(|(a, _)| *a == key) {
        return name.to_string();
    }
    if let Some(skill) = key.strip_prefix("skill.") {
        if let Some((_, name, _)) = SKILLS.iter().find(|(k, _, _)| *k == skill) {
            return name.to_string();
        }
    }
    if let Some(res) = key.strip_prefix("resource.").and_then(|k| k.strip_suffix(".max")) {
        if let Some((r, _)) = sheet.build.resources.iter().find(|(r, _)| r.key == res) {
            return r.name.clone();
        }
    }
    match key {
        "ac" => "Armor Class".into(),
        "hp.max" => "Hit Point Maximum".into(),
        "initiative" => "Initiative".into(),
        "speed" => "Speed".into(),
        "prof" => "Proficiency Bonus".into(),
        "passive.perception" => "Passive Perception".into(),
        "pact.slots.max" => "Pact slots".into(),
        k => k.to_string(),
    }
}

fn draw_breakdown(frame: &mut Frame, area: Rect, sheet: &Sheet, key: &str, theme: &Theme) {
    let Some(result) = sheet.ev.stats.get(key) else { return };
    let mut lines: Vec<Line> = Vec::new();
    match &result.value {
        Ok(v) => lines.push(Line::styled(v.to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD))),
        Err(e) => lines.push(Line::styled(format!("error: {e}"), Style::new().fg(Color::Red))),
    }
    lines.push(Line::default());
    for p in &result.parts {
        let (mark, style) = if p.applied {
            ("✓", Style::new().fg(theme.accent))
        } else {
            ("·", theme.dim)
        };
        let kind = match p.kind {
            PartKind::Base => "base",
            PartKind::Add => "add",
            PartKind::Set => "set",
            PartKind::Max => "best of",
            PartKind::Min => "cap",
            PartKind::Override => "override",
        };
        let value = p.value.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "—".into());
        let text_style = if p.applied { Style::new() } else { theme.dim };
        let mut spans = vec![
            Span::styled(format!("{mark} "), style),
            Span::styled(format!("{kind:<9}"), theme.label),
            Span::styled(format!("{value:>8}  "), text_style.add_modifier(Modifier::BOLD)),
            Span::styled(p.origin.clone(), text_style),
        ];
        if let Some(note) = &p.note {
            spans.push(Span::styled(format!("  ({note})"), theme.dim));
        }
        lines.push(Line::from(spans));
    }
    let width = lines.iter().map(|l| l.width()).max().unwrap_or(30).clamp(36, 90) as u16 + 6;
    let rect = centered(area, width, lines.len() as u16 + 4);
    frame.render_widget(Clear, rect);
    let block = pane(pane_title(&stat_label(sheet, key), true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(" esc ", theme.dim).right_aligned());
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

// ---- creation wizard ----

pub(crate) fn draw_wizard(frame: &mut Frame, area: Rect, lib: &Library, overlays: &OverlaySet, w: &crate::wizard::Wizard, theme: &Theme) {
    use crate::wizard::{Method, View, POINT_BUY_BUDGET};
    let [steps, _, main, _, preview] = Layout::horizontal([
        Constraint::Length(30),
        Constraint::Length(1),
        Constraint::Length(62),
        Constraint::Length(1),
        Constraint::Min(20),
    ])
    .areas(area);

    // Progress sidebar with the choices made so far.
    let block = pane(pane_title(&format!("New character · {}", w.name()), false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(steps);
    frame.render_widget(block, steps);
    let mut lines: Vec<Line> = Vec::new();
    for (label, done, current) in w.progress() {
        let (mark, style) = match (done, current) {
            (_, true) => ("▸ ", Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
            (true, _) => ("✓ ", Style::new().fg(theme.accent)),
            _ => ("  ", theme.dim),
        };
        lines.push(Line::from(vec![
            Span::styled(mark, style),
            Span::styled(label, if current { style } else if done { Style::new() } else { theme.dim }),
        ]));
    }
    lines.push(Line::default());
    for (k, v) in w.summary() {
        lines.push(Line::styled(k.clone(), theme.label));
        for part in wrap_words(v, inner.width.saturating_sub(2) as usize) {
            lines.push(Line::styled(format!("  {part}"), theme.muted));
        }
    }
    frame.render_widget(Paragraph::new(lines), inner);

    // Current step.
    let (title, hints) = match w.view() {
        View::Pick(p) => (
            p.title.clone(),
            if p.multi.is_some() { "space toggle · enter next · esc back" } else { "type to filter · enter choose · esc back" },
        ),
        View::Abilities(ed) => (format!("Ability scores · {}", ed.method.label()), "j k move · h l adjust · enter next · esc back"),
        View::Review => ("Review".to_string(), "enter create · esc back"),
    };
    let block = pane(pane_title(&title, true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(format!(" {hints} "), theme.dim).right_aligned());
    let inner = block.inner(main);
    frame.render_widget(block, main);
    match w.view() {
        View::Pick(p) => draw_picker(frame, inner, p, theme),
        View::Abilities(ed) => {
            let mut lines = Vec::new();
            match ed.method {
                Method::PointBuy => {
                    let left = ed.points_left();
                    lines.push(Line::from(vec![
                        Span::styled("Points left  ", theme.label),
                        Span::styled(format!("{left}/{POINT_BUY_BUDGET}"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
                    ]));
                }
                Method::StandardArray => lines.push(Line::styled("h/l swaps the value with another ability.", theme.dim)),
                Method::Manual => lines.push(Line::styled("h/l lowers or raises the score.", theme.dim)),
            }
            lines.push(Line::styled("Species and background increases come next.", theme.dim));
            lines.push(Line::default());
            for (i, (_, name)) in ABILITIES.iter().enumerate() {
                let base = if i == ed.cursor { theme.selected } else { Style::new() };
                let score = ed.scores[i];
                let modifier = (score - 10).div_euclid(2);
                let mut spans = vec![
                    Span::styled(format!(" {name:<14}"), base),
                    Span::styled(if i == ed.cursor { "‹ " } else { "  " }, base.fg(theme.accent)),
                    Span::styled(format!("{score:>2}"), base.add_modifier(Modifier::BOLD)),
                    Span::styled(if i == ed.cursor { " ›" } else { "  " }, base.fg(theme.accent)),
                    Span::styled(format!("   {modifier:+}"), base.fg(theme.accent)),
                ];
                if ed.method == Method::PointBuy {
                    spans.push(Span::styled(format!("   cost {}", crate::wizard::point_cost(score)), base.patch(theme.dim)));
                }
                lines.push(Line::from(spans).style(base));
            }
            frame.render_widget(Paragraph::new(lines), inner);
        }
        View::Review => {
            let mut lines = vec![Line::styled("Everything below is saved to the character file.", theme.dim), Line::default()];
            for (k, v) in w.summary() {
                for (i, part) in wrap_words(v, inner.width.saturating_sub(22) as usize).into_iter().enumerate() {
                    let key = if i == 0 { k.as_str() } else { "" };
                    lines.push(Line::from(vec![Span::styled(format!("{key:<22}"), theme.label), Span::raw(part)]));
                }
            }
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled(" enter ", theme.keycap),
                Span::styled(" create the character", theme.dim),
            ]));
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }

    // Preview of the highlighted entity, or the resulting stats on review.
    let block = pane(pane_title("Preview", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(preview);
    frame.render_widget(block, preview);
    if let Some((title, items)) = w.equipment_preview(lib) {
        let mut lines = vec![Line::styled(title, theme.title), Line::default()];
        lines.extend(items.into_iter().map(|i| Line::from(vec![Span::styled("•  ", theme.bullet), Span::raw(i)])));
        frame.render_widget(Paragraph::new(lines), inner);
    } else if let Some(e) = w.preview(lib) {
        let mut lines = vec![Line::styled(lib.entity(e).name.clone(), theme.title), Line::default()];
        let doc = view::document(lib, e, &Default::default());
        lines.extend(render::render(&doc, inner.width.min(96), None, theme).lines);
        frame.render_widget(Paragraph::new(lines), inner);
    } else {
        let ch = w.preview_character();
        let b = build(&ch, lib, overlays);
        let ev = b.evaluate();
        let mut lines = vec![
            Line::styled(ch.name.clone(), theme.title),
            Line::styled(identity(lib, &ch, &b.class_summary()), theme.muted),
            Line::default(),
        ];
        for (label, key) in [("Hit Points", "hp.max"), ("Armor Class", "ac"), ("Initiative", "initiative"), ("Speed", "speed")] {
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<14}"), theme.label),
                Span::styled(stat(&ev, key), Style::new().add_modifier(Modifier::BOLD)),
            ]));
        }
        lines.push(Line::default());
        for (a, name) in ABILITIES {
            lines.push(Line::from(vec![
                Span::styled(format!("{name:<14}"), theme.label),
                Span::styled(format!("{:>3}", stat(&ev, a)), Style::new().add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", signed(&ev, &format!("mod.{a}"))), Style::new().fg(theme.accent)),
            ]));
        }
        for warn in &b.warnings {
            lines.push(Line::styled(format!("⚠ {warn}"), Style::new().fg(Color::Yellow)));
        }
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// Greedy word wrap for short plain strings.
fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut out = vec![String::new()];
    for word in text.split_whitespace() {
        let line = out.last_mut().unwrap();
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            out.push(word.to_string());
        } else {
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
    }
    out
}
