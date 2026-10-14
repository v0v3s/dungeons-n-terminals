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
    let warn = if sheet.build.warnings.is_empty() {
        vec![]
    } else {
        vec![Span::styled(format!("⚠ {} warning(s) ", sheet.build.warnings.len()), Style::new().fg(Color::Yellow))]
    };
    frame.render_widget(
        Paragraph::new(vec![
            spread(
                vec![Span::styled(" ◆ ", Style::new().fg(theme.accent)), Span::styled(ch.name.clone(), theme.title)],
                warn,
                header.width as usize,
            ),
            Line::styled(format!("   {}", identity(lib, ch, &sheet.build.class_summary())), theme.muted),
        ]),
        header,
    );

    let mut spans = vec![Span::raw(" ")];
    for t in Tab::ALL {
        let style = if t == sheet.tab { theme.category_active } else { theme.category_inactive };
        spans.push(Span::styled(format!(" {} ", t.label()), style));
        spans.push(Span::raw(" "));
    }
    for soon in ["Spells", "Inventory", "Notes"] {
        spans.push(Span::styled(format!(" {soon} "), theme.dim));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), tabs);

    match sheet.tab {
        Tab::Overview => draw_overview(frame, body, sheet, theme),
        Tab::Features => draw_features(frame, body, lib, overlays, sheet, theme),
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
        None => {}
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
        Span::styled("score  mod  save", theme.label),
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
                Span::styled(format!("{:>6} ", signed(ev, &format!("save.{a}"))), base),
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
