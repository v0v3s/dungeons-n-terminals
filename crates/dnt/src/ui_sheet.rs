//! Drawing for the Characters section: roster, character sheet and popups.

use dnt_data::{view, Library};
use dnt_rules::builder::{build, FeatureSource};
use dnt_rules::character::{Hand, Slot};
use dnt_rules::engine::PartKind;
use dnt_rules::overlay::resolve_id;
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

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    lib: &Library,
    overlays: &OverlaySet,
    c: &mut Characters,
    theme: &Theme,
    campaign: Option<&str>,
) {
    if let Some(w) = &c.wizard {
        return draw_wizard(frame, area, lib, overlays, w, &c.filter, theme);
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
    let [left, _, right] = Layout::horizontal([Constraint::Length(40), Constraint::Length(1), Constraint::Min(30)]).areas(area);
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
    let mut lines =
        vec![Line::styled(ch.name.clone(), theme.title), Line::styled(identity(lib, ch, &b.class_summary()), theme.muted), Line::default()];
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
    let name_of =
        |id: &Option<String>| id.as_deref().and_then(|id| dnt_rules::overlay::resolve_id(lib, id)).map(|e| lib.entity(e).name.clone());
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
    let [header, tabs, _, body] =
        Layout::vertical([Constraint::Length(2), Constraint::Length(1), Constraint::Length(1), Constraint::Min(5)]).areas(area);

    let ch = sheet.character();
    let mut warn = Vec::new();
    if !sheet.session.pending.is_empty() {
        warn.push(Span::styled(format!(" {} choice(s) to make ", sheet.session.pending.len()), theme.category_active));
        warn.push(Span::styled("  press ", theme.dim));
        warn.push(Span::styled(" c ", theme.keycap));
        warn.push(Span::raw("   "));
    }
    let gaps = crate::spells_tab::spell_gaps(ch, &dnt_rules::spells::casters(lib, &sheet.session.build, &sheet.session.ev));
    let open: i64 = gaps.iter().map(|(_, c, s)| c + s).sum();
    if open > 0 {
        warn.push(Span::styled(format!(" {open} spell(s) to choose "), theme.category_active));
        warn.push(Span::styled("  Spells tab ", theme.dim));
        warn.push(Span::raw("   "));
    }
    if !sheet.session.build.warnings.is_empty() {
        warn.push(Span::styled(format!("⚠ {} warning(s) ", sheet.session.build.warnings.len()), Style::new().fg(Color::Yellow)));
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
                Line::styled(format!("   {}", identity(lib, ch, &sheet.session.build.class_summary())), theme.muted)
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

    frame.render_widget(Paragraph::new(Line::from(spans)), tabs);

    match sheet.tab {
        Tab::Overview => draw_overview(frame, body, lib, sheet, theme),
        Tab::Features => draw_features(frame, body, lib, overlays, sheet, theme),
        Tab::Inventory => draw_inventory(frame, body, lib, sheet, theme),
        Tab::Spells => draw_spells(frame, body, lib, sheet, theme),
        Tab::Notes => draw_character_note(frame, body, sheet, theme),
    }

    // The rules editor keeps its scroll state, so it draws from the sheet.
    if let Some(Popup::Overlay(ed)) = sheet.popup.as_mut() {
        draw_overlay_editor(frame, body, ed, theme);
        return;
    }
    match sheet.popup.clone() {
        Some(Popup::Overlay(_)) => {}
        Some(Popup::Conditions(cursor)) => {
            let names = dnt_rules::conditions::CONDITIONS;
            let st = &sheet.session.file.character.state;
            let rect = centered(area, 96.min(area.width), names.len() as u16 + 7);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title("Conditions", true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" space toggle · + - exhaustion · esc done ", theme.dim).right_aligned());
            let mut lines = Vec::new();
            for (i, c) in names.iter().enumerate() {
                let on = st.conditions.iter().any(|x| x.eq_ignore_ascii_case(c));
                let base = if i == cursor { theme.selected } else { Style::new() };
                lines.push(
                    Line::from(vec![
                        Span::styled(if on { "◉ " } else { "○ " }, if on { base.fg(Color::Magenta) } else { base.patch(theme.dim) }),
                        Span::styled(format!("{c:<15}"), if on { base.add_modifier(Modifier::BOLD) } else { base }),
                        Span::styled(dnt_rules::conditions::summary(c), base.patch(theme.dim)),
                    ])
                    .style(base),
                );
            }
            let base = if cursor == names.len() { theme.selected } else { Style::new() };
            let lvl = st.exhaustion;
            let what = dnt_rules::conditions::exhaustion_summary(lvl, sheet.session.edition == dnt_rules::play::Edition::Modern);
            lines.push(Line::default());
            lines.push(
                Line::from(vec![
                    Span::styled(format!("  {:<15}", "Exhaustion"), base),
                    Span::styled(
                        format!("{lvl}  "),
                        base.fg(if lvl > 0 { Color::Magenta } else { Color::Reset }).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(what, base.patch(theme.dim)),
                ])
                .style(base),
            );
            frame.render_widget(Paragraph::new(lines).block(block), rect);
        }
        Some(Popup::Breakdown(key)) => draw_breakdown(frame, area, sheet, &key, theme),
        Some(Popup::Input(prompt, text)) => {
            let rect = centered(area, 34, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(prompt.label(), true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])).block(block),
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
        Some(Popup::SpellPick(picker, _)) | Some(Popup::Cast(picker, _)) => {
            let rows = picker.items.len() as u16 + 7;
            let rect = centered(area, 84.min(area.width), area.height.saturating_sub(4).min(28).min(rows.max(10)));
            frame.render_widget(Clear, rect);
            let hint = if picker.multi.is_some() {
                " space toggle · enter add · esc cancel "
            } else {
                " type to filter · enter choose · esc cancel "
            };
            let block = pane(pane_title(&picker.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(hint, theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, &picker, theme);
        }
        Some(Popup::Wear(picker, _)) | Some(Popup::MoveInto(picker, _)) | Some(Popup::SwapMastery(picker, _)) => {
            let rect = centered(area, 64.min(area.width), (picker.items.len() as u16 + 7).min(area.height));
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&picker.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" enter choose · esc cancel ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, &picker, theme);
        }
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
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])).block(block),
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
                .title_bottom(Line::styled(format!(" {remaining} choice(s) left · enter choose · esc stop "), theme.dim).right_aligned());
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
    let shown = if p.query.is_empty() { Span::styled("type to filter", theme.dim) } else { Span::raw(p.query.clone()) };
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
/// Attack bonus, damage and range of each weapon and the unarmed strike.
fn draw_attacks(frame: &mut Frame, area: Rect, lib: &Library, sheet: &Sheet, theme: &Theme) {
    let block = pane(pane_title("Attacks", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let ev = &sheet.session.ev;
    let damage_of = |a: &dnt_rules::attacks::Attack| {
        let dmg = |k: &str| ev.get(&format!("attack.{}.{k}", a.slug)).map(|v| v.to_string());
        let mut d = format!("{} {}", dmg("damage").unwrap_or_else(|| "?".into()), a.damage_type);
        if let Some(two) = dmg("versatile") {
            d.push_str(&format!(" ({two})"));
        }
        d
    };
    let attacks = &sheet.session.build.attacks;
    let col = |f: &dyn Fn(&dnt_rules::attacks::Attack) -> usize, min: usize| attacks.iter().map(f).max().unwrap_or(0).max(min) + 3;
    let name_w = col(&|a| a.name.width(), 6).min(28);
    let dmg_w = col(&|a| damage_of(a).width(), 6);
    let range_w = col(&|a| a.range.width(), 5);
    // What is in hand right now.
    let ch = sheet.character();
    let held: Vec<String> = ch
        .inventory
        .iter()
        .filter_map(|l| {
            let h = l.hand?;
            let name = crate::characters::line_name(lib, l);
            Some(match h {
                Hand::Pair => format!("{name} ×2"),
                Hand::Both => format!("{name} (two hands)"),
                _ => name,
            })
        })
        .collect();
    let mut lines = vec![
        spread(
            vec![
                Span::styled("In hand  ", theme.label),
                if held.is_empty() {
                    Span::styled("nothing", theme.dim)
                } else {
                    Span::styled(held.join(" · "), Style::new().add_modifier(Modifier::BOLD))
                },
            ],
            match sheet.session.roller.next {
                dnt_rules::roll::Advantage::Normal => vec![Span::styled("r roll · space main · o off · b both · S shield ", theme.dim)],
                m => vec![Span::styled(format!(" next roll: {} ", m.label()), theme.category_active), Span::raw(" ")],
            },
            width,
        ),
        Line::default(),
    ];
    lines.push(Line::from(vec![
        Span::styled(format!("{:<6}", "hand"), theme.label),
        Span::styled(format!("{:<name_w$}", "weapon"), theme.label),
        Span::styled(format!("{:>4}   ", "hit"), theme.label),
        Span::styled(format!("{:<dmg_w$}", "damage"), theme.label),
        Span::styled(format!("{:<range_w$}", "range"), theme.label),
        Span::styled("notes", theme.label),
    ]));
    let mut selected = 0;
    for a in attacks {
        let item = Item::Attack(a.slug.clone());
        let base = row_style(sheet, &item, theme);
        if base != Style::new() {
            selected = lines.len();
        }
        let faint = if base == Style::new() { theme.dim } else { theme.muted };
        let hit = ev.int(&format!("attack.{}.bonus", a.slug)).map(|n| format!("{n:+}")).unwrap_or_else(|| "?".into());
        let mut notes: Vec<Span> = Vec::new();
        for n in &a.notes {
            notes.push(Span::styled(format!("{n}  "), base.fg(Color::Red)));
        }
        if a.monk_weapon {
            notes.push(Span::styled("monk weapon  ", base.fg(theme.accent)));
        }
        if let Some(m) = &a.mastery {
            notes.push(Span::styled(format!("mastery: {m}  "), base.fg(theme.accent)));
        }
        if !a.properties.is_empty() {
            notes.push(Span::styled(a.properties.join(", "), base.patch(faint)));
        }
        let in_hand = a.hand.is_some();
        let name_style = if in_hand {
            base.add_modifier(Modifier::BOLD)
        } else if a.item.is_some() {
            base.patch(faint)
        } else {
            base
        };
        let hit_style = if a.proficient { base.fg(theme.accent).add_modifier(Modifier::BOLD) } else { base.fg(Color::Red) };
        let hand = match a.hand {
            Some(Hand::Main) => "main",
            Some(Hand::Off) => "off",
            Some(Hand::Both) => "both",
            Some(Hand::Pair) => "pair",
            None => "",
        };
        // An off-hand weapon shows its off-hand attack.
        let damage = match (a.hand, ev.get(&format!("attack.{}.offhand", a.slug))) {
            (Some(Hand::Off), Some(off)) => format!("{off} {}", a.damage_type),
            _ => damage_of(a),
        };
        if let (Some(Hand::Pair), Some(off)) = (a.hand, ev.get(&format!("attack.{}.offhand", a.slug))) {
            notes.insert(0, Span::styled(format!("off hand {off}  "), base.fg(theme.accent)));
        }
        if a.hand == Some(Hand::Off) {
            notes.insert(0, Span::styled("off-hand attack  ", base.fg(theme.accent)));
        }
        let mut spans = vec![
            Span::styled(format!("{hand:<6}"), base.fg(theme.accent)),
            Span::styled(format!("{:<name_w$}", truncate(&a.name, name_w - 1)), name_style),
            Span::styled(format!("{hit:>4}   "), hit_style),
            Span::styled(format!("{:<dmg_w$}", damage), base),
            Span::styled(format!("{:<range_w$}", a.range), base.patch(faint)),
        ];
        spans.extend(notes);
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        if used > width {
            // Trim the notes to the pane.
            let mut out = Vec::new();
            let mut left = width;
            for sp in spans {
                let w = sp.content.width();
                if w <= left {
                    left -= w;
                    out.push(sp);
                } else {
                    out.push(Span::styled(truncate(&sp.content, left), sp.style));
                    break;
                }
            }
            spans = out;
        }
        lines.push(Line::from(spans).style(base));
    }
    // Extra damage on some hits (Sneak Attack, Radiant Strikes): the value
    // and, from the feature, when it applies.
    let riders: Vec<(&String, &dnt_rules::StatResult)> = ev.stats.iter().filter(|(k, _)| k.starts_with("rider.")).collect();
    if !riders.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("On a hit", theme.label));
        for (_, r) in riders {
            let Ok(v) = &r.value else { continue };
            let why = r.parts.iter().rev().find(|p| p.applied).map(|p| p.origin.clone()).unwrap_or_default();
            let (name, rest) = why.split_once(": ").unwrap_or((why.as_str(), ""));
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled(format!("+{v} "), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(name.to_string(), Style::new().add_modifier(Modifier::BOLD)),
                Span::styled(
                    if rest.is_empty() { String::new() } else { format!("  {}", truncate(rest, width.saturating_sub(name.width() + 20))) },
                    theme.dim,
                ),
            ]));
        }
    }
    let scroll = (selected + 1).saturating_sub(inner.height as usize);
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

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

fn draw_overview(frame: &mut Frame, area: Rect, lib: &Library, sheet: &Sheet, theme: &Theme) {
    let [abil, _, mid, _, skills] = Layout::horizontal([
        Constraint::Length(34),
        Constraint::Length(1),
        Constraint::Min(40),
        Constraint::Length(1),
        Constraint::Length(34),
    ])
    .areas(area);
    let ev = &sheet.session.ev;

    // Abilities and saves.
    let block = pane(pane_title("Abilities", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(abil);
    frame.render_widget(block, abil);
    let mut lines = vec![Line::from(vec![Span::styled(format!("{:<13}", ""), theme.label), Span::styled("score  mod save", theme.label)])];
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
    let casters: Vec<_> = sheet.session.build.classes.iter().filter(|c| c.casting_ability.is_some()).collect();
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
            let mult = sheet.session.build.model.inputs.get(&format!("prof.skill.{k}")).cloned().unwrap_or(Value::int(0));
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
    let riders = sheet.session.ev.stats.keys().filter(|k| k.starts_with("rider.")).count() as u16;
    let attacks_h = (sheet.session.build.attacks.len() as u16 + 7 + if riders > 0 { riders + 2 } else { 0 }).min(22);
    let [tiles, _, attacks, _, rest] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(1),
        Constraint::Length(attacks_h),
        Constraint::Length(1),
        Constraint::Min(3),
    ])
    .areas(mid);
    draw_attacks(frame, attacks, lib, sheet, theme);
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
    // Conditions (C to change) and what they do.
    let st = &ch.state;
    if !st.conditions.is_empty() || st.exhaustion > 0 {
        let mut names: Vec<String> = st.conditions.clone();
        if st.exhaustion > 0 {
            names.push(format!("Exhaustion {}", st.exhaustion));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "Conditions"), theme.label),
            Span::styled(names.join(" · "), Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled("   C", theme.dim),
        ]));
        for c in &st.conditions {
            lines.push(Line::styled(
                format!("{:<12}{}", "", truncate(dnt_rules::conditions::summary(c), width.saturating_sub(12))),
                theme.dim,
            ));
        }
        if st.exhaustion > 0 {
            let what = dnt_rules::conditions::exhaustion_summary(st.exhaustion, sheet.session.edition == dnt_rules::play::Edition::Modern);
            let style = if st.exhaustion >= 6 { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { theme.dim };
            lines.push(Line::styled(format!("{:<12}Exhaustion: {}", "", truncate(&what, width.saturating_sub(24))), style));
        }
    }
    // The last few rolls, newest first.
    if !sheet.session.roller.log.is_empty() {
        lines.push(Line::default());
        for (n, r) in sheet.session.roller.log.iter().take(3).enumerate() {
            let head = if n == 0 { Style::new().add_modifier(Modifier::BOLD) } else { theme.muted };
            let colour = match r.crit {
                Some(true) => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
                Some(false) => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                None if n == 0 => Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                None => theme.muted,
            };
            let label = if n == 0 { "Rolled" } else { "" };
            lines.push(Line::from(vec![
                Span::styled(format!("{label:<12}"), theme.label),
                Span::styled(format!("{}  ", r.title), head),
                Span::styled(r.result.clone(), colour),
                Span::styled(
                    format!("  {}", truncate(&r.detail, width.saturating_sub(r.title.width() + r.result.width() + 18))),
                    theme.dim,
                ),
            ]));
            for e in &r.extra {
                lines.push(Line::styled(
                    format!("{:<12}{}", "", truncate(e, width.saturating_sub(12))),
                    if n == 0 { Style::new() } else { theme.dim },
                ));
            }
        }
    }
    lines.push(Line::default());

    let section = |lines: &mut Vec<Line>, title: &str| {
        if lines.last().is_some_and(|l| !l.spans.is_empty()) {
            lines.push(Line::default());
        }
        lines.push(Line::styled(title.to_string(), theme.heading));
    };

    if !sheet.session.build.resources.is_empty() {
        section(&mut lines, "Resources");
        for (r, _) in &sheet.session.build.resources {
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
            let level = sheet.session.build.model.inputs.get("pact.level").map(|v| v.to_string()).unwrap_or_default();
            let mut spans = vec![Span::styled(format!("{:<22}", format!("Pact (level {level})")), base)];
            spans.extend(pips(left, pact, theme, base));
            lines.push(
                spread(
                    spans,
                    vec![Span::styled(format!("{left}/{pact}  "), base), Span::styled("short rest", base.patch(theme.dim))],
                    width,
                )
                .style(base),
            );
        }
    }

    let dice: Vec<i64> = sheet.items().into_iter().filter_map(|i| if let Item::HitDie(f) = i { Some(f) } else { None }).collect();
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

    if !sheet.session.build.toggles.is_empty() {
        section(&mut lines, "Toggles");
        for (t, _) in &sheet.session.build.toggles {
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

    if !sheet.session.build.warnings.is_empty() {
        section(&mut lines, "Warnings");
        for w in &sheet.session.build.warnings {
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
    let [list, _, detail] = Layout::horizontal([Constraint::Length(42), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let block = pane(pane_title("Features", true, theme), true, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(list);
    frame.render_widget(block, list);

    let width = inner.width as usize;
    let mut lines: Vec<Line> = Vec::new();
    let mut last_group = String::new();
    let mut cursor_line = 0;
    for (i, f) in sheet.session.build.features.iter().enumerate() {
        let group = match f.source {
            FeatureSource::Class(ci) => {
                let c = &sheet.session.build.classes[ci];
                format!("{} {}", c.name, c.level)
            }
            FeatureSource::Subclass(ci) => {
                sheet.session.build.classes[ci].subclass.map(|s| lib.entity(s).name.clone()).unwrap_or_else(|| "Subclass".into())
            }
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

    let Some(f) = sheet.session.build.features.get(sheet.feature_cursor) else { return };
    let e = lib.entity(f.entity);
    let block = pane(pane_title(&e.name, false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(detail);
    frame.render_widget(block, detail);
    let doc = view::document(lib, f.entity, &sheet.session.filter);
    let rendered = render::render(&doc, inner.width.min(96), None, theme);
    let max_scroll = rendered.lines.len().saturating_sub(inner.height as usize);
    sheet.feature_scroll = sheet.feature_scroll.min(max_scroll);
    frame.render_widget(Paragraph::new(rendered.lines).scroll((sheet.feature_scroll as u16, 0)), inner);
}

fn draw_spells(frame: &mut Frame, area: Rect, lib: &Library, sheet: &mut Sheet, theme: &Theme) {
    use crate::spells_tab::{self, Row, Slot};
    use dnt_rules::spells::{self, Grant};
    let [bar, _, main] = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3)]).areas(area);
    let ch = &sheet.session.file.character;
    let ev = &sheet.session.ev;

    // Slots and concentration.
    let mut spans = vec![Span::styled(" Slots  ", theme.label)];
    let mut any = false;
    for l in 1..=9 {
        let max = play::slot_max(ev, l);
        if max > 0 {
            any = true;
            spans.push(Span::styled(format!("{l} "), theme.muted));
            spans.extend(pips(play::slots_left(ch, ev, l), max, theme, Style::new()));
            spans.push(Span::raw("   "));
        }
    }
    let pact = ev.int("pact.slots.max").unwrap_or(0);
    if pact > 0 {
        any = true;
        let level = sheet.session.build.model.inputs.get("pact.level").map(|v| v.to_string()).unwrap_or_default();
        spans.push(Span::styled(format!("pact {level} "), theme.muted));
        spans.extend(pips(pact - ch.state.pact_slots_used, pact, theme, Style::new()));
        spans.push(Span::raw("   "));
    }
    if !any {
        spans.push(Span::styled("none", theme.dim));
    }
    let right = match &ch.state.concentration {
        Some(c) => vec![
            Span::styled(" ◎ concentrating on ", Style::new().fg(Color::Magenta)),
            Span::styled(c.clone(), Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled("   C to end ", theme.dim),
        ],
        None => vec![],
    };
    frame.render_widget(Paragraph::new(spread(spans, right, bar.width as usize)), bar);

    let [list, _, detail] = Layout::horizontal([Constraint::Length(64), Constraint::Length(1), Constraint::Min(30)]).areas(main);
    let block = pane(pane_title("Spells", true, theme), true, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(list);
    frame.render_widget(block, list);
    let data = spells_tab::rows(lib, ch, &sheet.session.build, ev);
    let width = inner.width as usize;
    sheet.spell_cursor = sheet.spell_cursor.min(data.rows.len().saturating_sub(1));

    let count = |have: usize, max: i64, label: &str| {
        let over = have as i64 > max;
        vec![
            Span::styled(format!("{label} "), theme.label),
            Span::styled(
                format!("{have}/{max}  "),
                if over { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::new() },
            ),
        ]
    };
    let mut lines: Vec<Line> = Vec::new();
    if data.rows.is_empty() {
        lines.push(Line::styled("No spellcasting yet.", theme.muted));
        lines.push(Line::styled("Spellcasting classes, subclasses and feats like Magic Initiate add spells here.", theme.dim));
    }
    for (i, row) in data.rows.iter().enumerate() {
        let selected = i == sheet.spell_cursor;
        let base = if selected { theme.selected } else { Style::new() };
        let faint = if selected { theme.muted } else { theme.dim };
        let (row, favorite_row) = match row {
            Row::Favorite(inner) => (&**inner, true),
            r => (r, false),
        };
        // In the Favorites group, say where each spell comes from.
        let origin = match row {
            Row::ClassSpell { caster, .. } if favorite_row => Some(data.casters[*caster].name.clone()),
            Row::GrantSpell { grant, .. } if favorite_row => Some(sheet.session.build.granted[*grant].origin.clone()),
            _ => None,
        };
        let spell_spans = |id: &str, mark: Span<'static>, how: String| {
            let e = resolve_id(lib, id);
            let name = e.map(|e| lib.entity(e).name.clone()).unwrap_or_else(|| id.to_string());
            let star = ch.spells.favorites.iter().any(|f| f == id);

            let mut right = String::new();
            if let Some(e) = e {
                let lvl = spells::spell_level(lib, e);
                // What it deals (or heals) at its own level.
                let r = spells::spell_roll(lib, e, ch.total_level() as i64, lvl);
                let caster = match row {
                    Row::ClassSpell { caster, .. } => data.casters.get(*caster).map(|c| c.slug.as_str()),
                    _ => None,
                };
                let modifier = {
                    let (_, dc) = dnt_core::rolls::spell_numbers(sheet, caster);
                    dc - 8 - ev.int("prof").unwrap_or(2)
                };
                let dice = r.dice.as_ref().map(|d| if r.add_modifier { d.plus_flat(modifier) } else { d.clone() });
                if let Some(d) = &dice {
                    right.push_str(&format!(
                        "{d} {} · ",
                        if r.kind.is_empty() { "damage" } else { r.kind.split(" or ").next().unwrap_or("") }
                    ));
                }
                right.push_str(&if lvl == 0 { "cantrip".into() } else { format!("level {lvl}") });
                if spells::concentration(lib, e) {
                    right.push_str(" · C");
                }
                if spells::ritual(lib, e) {
                    right.push_str(" · R");
                }
            }
            if !how.is_empty() {
                right = format!("{how} · {right}");
            }
            if let Some(o) = &origin {
                right = format!("{right} · {o}");
            }
            // The name gets what the details leave (at least 16 columns).
            let right = truncate(&right, width.saturating_sub(24));
            let star = star && !favorite_row;
            let name_w = width.saturating_sub(right.width() + 8 + if star { 2 } else { 0 }).max(16);
            let mut left = vec![Span::styled("  ", base), mark, Span::styled(truncate(&name, name_w), base)];
            if star {
                left.push(Span::styled(" ★", base.fg(theme.accent)));
            }
            spread(left, vec![Span::styled(format!("{right} "), base.patch(faint).remove_modifier(Modifier::BOLD))], width).style(base)
        };
        let line = match row {
            Row::ClassHeader(c) => {
                let caster = &data.casters[*c];
                let mine = ch.spells.classes.get(&caster.id).cloned().unwrap_or_default();
                let l = &caster.limits;
                let mut right = Vec::new();
                if l.cantrips > 0 {
                    right.extend(count(mine.cantrips.len(), l.cantrips, "cantrips"));
                }
                let label = match l.list {
                    spells::SpellList::Prepared => "prepared",
                    spells::SpellList::Known => "known",
                };
                right.extend(count(mine.prepared.len(), l.spells, label));
                if let Some(book) = l.spellbook {
                    let in_book = mine.spellbook.iter().chain(&mine.prepared).collect::<std::collections::BTreeSet<_>>().len();
                    right.extend(count(in_book, book, "book"));
                }
                if !lines.is_empty() {
                    lines.push(Line::default());
                }
                let dc = stat(ev, &format!("spell.{}.dc", caster.slug));
                let atk = signed(ev, &format!("spell.{}.attack", caster.slug));
                lines.push(
                    spread(
                        vec![Span::styled(format!("{} {}", caster.name, caster.level), base.patch(theme.heading))],
                        vec![Span::styled(
                            format!("DC {dc} · {atk} · up to level {} ", l.max_level),
                            base.patch(faint).remove_modifier(Modifier::BOLD),
                        )],
                        width,
                    )
                    .style(base),
                );
                // Counts on their own (non-selectable) line.
                Line::from([vec![Span::raw("  ")], right].concat())
            }
            Row::ClassSpell { id, slot, .. } => {
                let (mark, how) = match slot {
                    Slot::Cantrip => (Span::styled("◦ ", base.fg(theme.accent)), String::new()),
                    Slot::Prepared => (Span::styled("● ", base.fg(theme.accent)), String::new()),
                    Slot::Book => (Span::styled("○ ", base.patch(faint)), "spellbook".to_string()),
                };
                spell_spans(id, mark, how)
            }
            Row::GrantHeader(g) => {
                if !lines.is_empty() {
                    lines.push(Line::default());
                }
                Line::styled(sheet.session.build.granted[*g].origin.clone(), base.patch(theme.heading)).style(base)
            }
            Row::GrantSpell { id, how, .. } => {
                let text = match how {
                    Grant::Prepared => "always prepared".to_string(),
                    Grant::Known => "known".to_string(),
                    Grant::Innate(None) => "at will".to_string(),
                    Grant::Innate(Some(n)) => {
                        let name = resolve_id(lib, id).map(|e| lib.entity(e).name.clone()).unwrap_or_default();
                        let key = format!("innate_{}", dnt_rules::class_table::slug(&name));
                        format!("{}/{n} free", play::resource_left(ch, ev, &key))
                    }
                };
                spell_spans(id, Span::styled("◆ ", base.fg(theme.accent)), text)
            }
            Row::FavoritesHeader => Line::styled("★ Favorites", base.patch(theme.heading)).style(base),
            Row::Favorite(_) => unreachable!("unwrapped above"),
            Row::GrantChoice { filter, missing, .. } => Line::from(vec![
                Span::styled("  + ", base.fg(theme.accent)),
                Span::styled(format!("choose {missing} {}", spells::describe_filter(filter)), base.fg(theme.accent)),
                Span::styled("   enter", base.patch(faint)),
            ])
            .style(base),
        };
        lines.push(line);
    }
    let height = inner.height as usize;
    let cursor_line = lines.iter().position(|l| l.style == theme.selected).unwrap_or(0);
    let scroll = cursor_line.saturating_sub(height.saturating_sub(3));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);

    // Detail: the selected spell, or how the class prepares spells.
    let block = pane(pane_title("Spell", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(detail);
    frame.render_widget(block, detail);
    let id = data.rows.get(sheet.spell_cursor).and_then(|r| r.spell_id()).map(str::to_string);
    if let Some(e) = id.and_then(|id| resolve_id(lib, &id)) {
        let mut lines = vec![Line::styled(lib.entity(e).name.clone(), theme.title), Line::default()];
        let doc = view::document(lib, e, &sheet.session.filter);
        lines.extend(render::render(&doc, inner.width.min(96), None, theme).lines);
        frame.render_widget(Paragraph::new(lines), inner);
    } else {
        let help = vec![
            Line::styled("◦ cantrip   ● prepared/known   ○ in spellbook   ◆ from a feature", theme.dim),
            Line::default(),
            Line::from(vec![Span::styled(" a ", theme.keycap), Span::styled(" add a spell to the class under the cursor", theme.dim)]),
            Line::from(vec![
                Span::styled(" enter ", theme.keycap),
                Span::styled(" cast (choose a slot) · pick spells for a feature", theme.dim),
            ]),
            Line::from(vec![Span::styled(" p ", theme.keycap), Span::styled(" prepare / unprepare a spellbook spell", theme.dim)]),
            Line::from(vec![Span::styled(" x ", theme.keycap), Span::styled(" remove", theme.dim)]),
        ];
        frame.render_widget(Paragraph::new(help), inner);
    }
}

fn draw_character_note(frame: &mut Frame, area: Rect, sheet: &Sheet, theme: &Theme) {
    let block = pane(pane_title(&format!("Party/{}", sheet.character().name), true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(" enter edits in Notes ", theme.dim).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = sheet.note_path().and_then(|p| std::fs::read_to_string(p).ok());
    let Some(text) = text else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("No notes for this character yet.", theme.muted),
                Line::default(),
                Line::from(vec![
                    Span::styled(" enter ", theme.keycap),
                    Span::styled("  create one from the character template (backstory, bonds, goals)", theme.dim),
                ]),
            ]),
            inner,
        );
        return;
    };
    // Show the body without frontmatter, with headings styled.
    let mut lines = Vec::new();
    let mut in_front = text.starts_with("---");
    for (i, line) in text.lines().enumerate() {
        if in_front {
            if i > 0 && line.trim() == "---" {
                in_front = false;
            }
            continue;
        }
        let style = if line.starts_with('#') { theme.heading } else { Style::new() };
        lines.push(Line::styled(line.to_string(), style));
    }
    frame.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), inner);
}

fn draw_inventory(frame: &mut Frame, area: Rect, lib: &Library, sheet: &mut Sheet, theme: &Theme) {
    use crate::characters::{item_entity, line_name};
    use dnt_rules::equipment;
    let [list, _, gear, _, detail] = Layout::horizontal([
        Constraint::Length(58),
        Constraint::Length(1),
        Constraint::Length(44),
        Constraint::Length(1),
        Constraint::Min(30),
    ])
    .areas(area);
    draw_equipment(frame, gear, lib, sheet, theme);
    let ch = &sheet.session.file.character;
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
    let total_weight = equipment::carried_weight(lib, ch);
    let order = equipment::tree(ch, &sheet.folded);
    let mut cursor_row = 0;
    for &(i, depth) in &order {
        let line = &ch.inventory[i];
        if i == sheet.inventory_cursor {
            cursor_row = lines.len();
        }
        let w = equipment::weight(lib, line) * line.quantity as f64;
        let base = if i == sheet.inventory_cursor { theme.selected } else { Style::new() };
        let faint = if i == sheet.inventory_cursor { theme.muted } else { theme.dim };
        let entity = item_entity(lib, line);
        let needs_attune = entity.is_some_and(|e| equipment::requires_attunement(&lib.entity(e).data));
        let mark = match (line.equipped, line.hand.is_some(), line.attuned) {
            (_, _, true) => Span::styled("★ ", base.fg(theme.accent)),
            (true, _, _) => Span::styled("◉ ", base.fg(theme.accent)),
            (_, true, _) => Span::styled("◈ ", base.fg(theme.accent)),
            _ if needs_attune => Span::styled("☆ ", base.patch(faint)),
            _ => Span::styled("  ", base),
        };
        let mut kind = match (line.slot.filter(|_| line.equipped), line.hand) {
            (Some(slot), _) => format!(" {}", slot.label().to_lowercase()),
            (_, Some(h)) => format!(" {}", h.label()),
            _ => entity.and_then(|e| equipment::armor_kind(&lib.entity(e).data)).map(|k| format!(" {k}")).unwrap_or_default(),
        };
        if entity.is_some() && sheet.session.build.attacks.iter().any(|a| a.item == entity && a.monk_weapon) {
            kind.push_str(" monk weapon");
        }
        let weight = if w > 0.0 { format!("{w:.1}") } else { "—".into() };
        // Containers: a fold marker and how full they are.
        let container = equipment::container(lib, line);
        let fold = match (&container, &line.id) {
            (Some(_), Some(id)) if sheet.folded.contains(id) && !equipment::contents(ch, i).is_empty() => "▸ ",
            (Some(_), _) => "▾ ",
            _ => "",
        };
        let mut fill = Vec::new();
        if let Some(k) = container {
            let held = equipment::contents_weight(lib, ch, i);
            let over = k.capacity.is_some_and(|cap| held > cap);
            let text = match k.capacity {
                Some(cap) => format!(" {held:.0}/{cap:.0} lb"),
                None => format!(" {held:.0} lb"),
            };
            fill.push(Span::styled(text, if over { base.fg(Color::Red) } else { base.fg(theme.accent) }));
            if k.weightless {
                fill.push(Span::styled(" weightless", base.patch(faint)));
            }
        }
        let indent = "  ".repeat(depth);
        let fill_w: usize = fill.iter().map(|f| f.content.width()).sum();
        let name_w = width.saturating_sub(20 + indent.len() + 2 + fill_w + kind.width()).max(8);
        let mut left = vec![
            Span::raw(" "),
            mark,
            Span::raw(indent),
            Span::styled(fold, base.fg(theme.accent)),
            Span::styled(
                truncate(&line_name(lib, line), name_w),
                if container.is_some() { base.add_modifier(Modifier::BOLD) } else { base },
            ),
            Span::styled(kind, base.patch(faint).remove_modifier(Modifier::BOLD)),
        ];
        left.extend(fill);
        lines.push(spread(left, vec![Span::styled(format!("{:>5}{weight:>10} ", line.quantity), base)], width).style(base));
    }
    let height = items_area.height as usize;
    let scroll = (cursor_row + 1).saturating_sub(height);
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), items_area);

    // Totals: weight, attunement, coins.
    let capacity = sheet.session.ev.int("str").unwrap_or(10) * 15;
    let attuned = ch.inventory.iter().filter(|l| l.attuned).count();
    let c = &ch.currency;
    let coin = |n: i64, label: &str, color: Color| {
        vec![
            Span::styled(format!("{n}"), Style::new().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {label}   "), theme.label),
        ]
    };
    let mut coins = Vec::new();
    for (n, label, color) in [
        (c.pp, "pp", Color::White),
        (c.gp, "gp", Color::Yellow),
        (c.ep, "ep", Color::Gray),
        (c.sp, "sp", Color::Gray),
        (c.cp, "cp", Color::Red),
    ] {
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
                Span::styled(
                    format!("{total_weight:.1}"),
                    if over { Style::new().fg(Color::Red).add_modifier(Modifier::BOLD) } else { Style::new().add_modifier(Modifier::BOLD) },
                ),
                Span::styled(format!(" / {capacity} lb"), theme.muted),
            ]),
            Line::from(vec![
                Span::styled("Attunement ", theme.label),
                Span::styled(
                    format!("{attuned}"),
                    if attuned > 3 {
                        Style::new().fg(Color::Red).add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(" / 3", theme.muted),
            ]),
            Line::from([vec![Span::styled("Coins      ", theme.label)], coins].concat()),
            Line::from(vec![Span::styled("◉ worn  ◈ in hand  ★ attuned  ☆ needs attunement", theme.dim)]),
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
            let doc = view::document(lib, e, &sheet.session.filter);
            frame.render_widget(Paragraph::new(render::render(&doc, inner.width.min(96), None, theme).lines), inner);
        }
        None if line.is_some() => {
            frame.render_widget(Paragraph::new(Line::styled("A custom item.", theme.dim)), inner);
        }
        None => {}
    }
}

/// The rules editor: TOML on the left, the check and what it changes on
/// the right.
fn draw_overlay_editor(frame: &mut Frame, area: Rect, ed: &mut crate::overlay_editor::OverlayEditor, theme: &Theme) {
    use crate::overlay_editor::Source;
    use edtui::{EditorMode, EditorTheme, EditorView};
    use ratatui::widgets::Widget;
    frame.render_widget(Clear, area);
    let [left, _, right] = Layout::horizontal([Constraint::Percentage(58), Constraint::Length(1), Constraint::Min(30)]).areas(area);
    let mode = match ed.editor.mode {
        EditorMode::Insert => " INSERT ",
        EditorMode::Visual => " VISUAL ",
        EditorMode::Search => " SEARCH ",
        EditorMode::Normal => " NORMAL ",
    };
    let block =
        pane(pane_title(&format!("Rules for {}", ed.name), true, theme), true, theme).padding(Padding::new(1, 1, 1, 0)).title_bottom(
            Line::from(vec![Span::styled(" esc saves · ctrl-q discards ", theme.dim), Span::styled(mode, theme.keycap)]).right_aligned(),
        );
    let inner = block.inner(left);
    frame.render_widget(block, left);
    let editor_theme = EditorTheme::default()
        .base(Style::new())
        .cursor_style(Style::new().add_modifier(Modifier::REVERSED))
        .selection_style(Style::new().bg(Color::DarkGray))
        .hide_status_line();
    EditorView::new(&mut ed.editor).theme(editor_theme).wrap(true).render(inner, frame.buffer_mut());

    let block = pane(pane_title("Check", false, theme), false, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(right);
    frame.render_widget(block, right);
    let width = inner.width as usize;
    let mut lines = vec![Line::styled(
        match ed.source {
            Source::User => "Your rules for this feature.",
            Source::Builtin => "dnt's built-in rules. Saving a change makes your own version, which replaces them.",
            Source::New => "Nothing automates this feature yet: fill in the template.",
        },
        theme.muted,
    )];
    lines.push(Line::styled(format!("id = {}", ed.id), theme.dim));
    lines.push(Line::default());
    if ed.errors.is_empty() {
        lines.push(Line::styled("✓ valid", Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)));
    } else {
        for e in &ed.errors {
            lines.push(Line::styled(format!("⚠ {}", truncate(e, width.saturating_sub(2) * 3)), Style::new().fg(Color::Red)));
        }
    }
    lines.push(Line::default());
    lines.push(Line::styled("On this character", theme.heading));
    if ed.changes.is_empty() && ed.errors.is_empty() {
        lines.push(Line::styled("no change", theme.dim));
    }
    for c in &ed.changes {
        lines.push(Line::raw(truncate(c, width)));
    }
    lines.push(Line::default());
    lines.push(Line::styled("Empty text removes your rules (the built-in ones come back).", theme.dim));
    frame.render_widget(Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }), inner);
}

/// What is worn in each slot and what is in hand.
fn draw_equipment(frame: &mut Frame, area: Rect, lib: &Library, sheet: &Sheet, theme: &Theme) {
    use crate::characters::line_name;
    let ch = &sheet.session.file.character;
    let block = pane(pane_title("Equipment", false, theme), false, theme).padding(Padding::new(1, 1, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let cursor = sheet.inventory_cursor;
    let row = |label: &str, value: Option<(usize, String)>| {
        let selected = value.as_ref().is_some_and(|(i, _)| *i == cursor);
        let base = if selected { theme.selected } else { Style::new() };
        let (text, style) = match &value {
            Some((i, name)) => {
                let star = if ch.inventory[*i].attuned { " ★" } else { "" };
                (format!("{name}{star}"), base)
            }
            None => ("—".to_string(), base.patch(theme.dim)),
        };
        Line::from(vec![
            Span::styled(format!("{label:<11}"), if selected { base } else { theme.label }),
            Span::styled(truncate(&text, width.saturating_sub(12)), style),
        ])
        .style(base)
    };
    let mut lines = vec![Line::styled("Worn", theme.heading)];
    for slot in Slot::ALL {
        let worn: Vec<(usize, String)> = ch
            .inventory
            .iter()
            .enumerate()
            .filter(|(_, l)| l.equipped && l.slot == Some(slot))
            .map(|(i, l)| (i, line_name(lib, l)))
            .collect();
        if worn.is_empty() {
            lines.push(row(slot.label(), None));
        }
        for (n, w) in worn.into_iter().enumerate() {
            lines.push(row(if n == 0 { slot.label() } else { "" }, Some(w)));
        }
    }
    // Worn before slots existed (or edited by hand).
    for (i, l) in ch.inventory.iter().enumerate().filter(|(_, l)| l.equipped && l.slot.is_none()) {
        lines.push(row("(no slot)", Some((i, line_name(lib, l)))));
    }
    lines.push(Line::default());
    lines.push(Line::styled("In hand", theme.heading));
    let in_hand = |main: bool| {
        ch.inventory
            .iter()
            .enumerate()
            .find(|(_, l)| l.hand.is_some_and(|h| if main { h.uses_main() } else { h.uses_off() }))
            .map(|(i, l)| (i, line_name(lib, l)))
    };
    lines.push(row("Main hand", in_hand(true)));
    lines.push(row("Off hand", in_hand(false)));
    lines.push(Line::default());
    lines.push(Line::styled("space wears, or takes in hand", theme.dim));
    lines.push(Line::styled("o off hand; switch weapons on", theme.dim));
    lines.push(Line::styled("the Overview's attack list", theme.dim));
    frame.render_widget(Paragraph::new(lines), inner);
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
        if let Some((r, _)) = sheet.session.build.resources.iter().find(|(r, _)| r.key == res) {
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
    let attack = key.strip_prefix("attack.").and_then(|slug| sheet.session.build.attacks.iter().find(|a| a.slug == slug));
    let (title, lines) = match attack {
        Some(a) => {
            let mut lines = Vec::new();
            for (part, label) in [("bonus", "Attack bonus"), ("damage", "Damage"), ("versatile", "Damage, two-handed")] {
                let k = format!("attack.{}.{part}", a.slug);
                if sheet.session.ev.stats.contains_key(&k) {
                    if !lines.is_empty() {
                        lines.push(Line::default());
                    }
                    lines.push(Line::styled(label, theme.heading));
                    lines.extend(breakdown_lines(sheet, &k, theme));
                }
            }
            lines.push(Line::default());
            let prof = &sheet.session.build.weapon_proficiency;
            let status = if a.item.is_none() {
                "Everyone is proficient with unarmed strikes.".to_string()
            } else if a.proficient {
                "Proficient.".to_string()
            } else {
                "Not proficient: no proficiency bonus to hit.".to_string()
            };
            lines.push(Line::styled(status, if a.proficient { theme.muted } else { Style::new().fg(Color::Red) }));
            for o in &prof.origins {
                lines.push(Line::styled(format!("  {o}"), theme.dim));
            }
            if let Some(m) = &a.mastery {
                lines.push(Line::default());
                lines.push(Line::from(vec![
                    Span::styled("Mastery  ", theme.label),
                    Span::styled(m.clone(), Style::new().fg(theme.accent)),
                    Span::styled("  (see the compendium for what it does)", theme.dim),
                ]));
            }
            for n in &a.notes {
                lines.push(Line::styled(format!("⚠ {n}"), Style::new().fg(Color::Red)));
            }
            (a.name.clone(), lines)
        }
        None => {
            if !sheet.session.ev.stats.contains_key(key) {
                return;
            }
            (stat_label(sheet, key), breakdown_lines(sheet, key, theme))
        }
    };
    let width = lines.iter().map(|l| l.width()).max().unwrap_or(30).clamp(36, 90) as u16 + 6;
    let rect = centered(area, width, lines.len() as u16 + 4);
    frame.render_widget(Clear, rect);
    let block = pane(pane_title(&title, true, theme), true, theme)
        .padding(Padding::new(2, 2, 1, 0))
        .title_bottom(Line::styled(" esc ", theme.dim).right_aligned());
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

/// The value of a stat and how it is made up.
fn breakdown_lines(sheet: &Sheet, key: &str, theme: &Theme) -> Vec<Line<'static>> {
    let Some(result) = sheet.session.ev.stats.get(key) else { return Vec::new() };
    let mut lines: Vec<Line> = Vec::new();
    match &result.value {
        Ok(v) => lines.push(Line::styled(v.to_string(), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD))),
        Err(e) => lines.push(Line::styled(format!("error: {e}"), Style::new().fg(Color::Red))),
    }
    lines.push(Line::default());
    for p in &result.parts {
        let (mark, style) = if p.applied { ("✓", Style::new().fg(theme.accent)) } else { ("·", theme.dim) };
        let kind = match p.kind {
            PartKind::Base => "base",
            PartKind::Add => "add",
            PartKind::Set => "set",
            PartKind::Max => "best of",
            PartKind::Min => "cap",
            PartKind::Mul => "times",
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
    lines
}

// ---- creation wizard ----

pub(crate) fn draw_wizard(
    frame: &mut Frame,
    area: Rect,
    lib: &Library,
    overlays: &OverlaySet,
    w: &crate::wizard::Wizard,
    filter: &dnt_data::SourceFilter,
    theme: &Theme,
) {
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
            Span::styled(
                label,
                if current {
                    style
                } else if done {
                    Style::new()
                } else {
                    theme.dim
                },
            ),
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
            lines.push(Line::from(vec![Span::styled(" enter ", theme.keycap), Span::styled(" create the character", theme.dim)]));
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
        let doc = view::document(lib, e, filter);
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
