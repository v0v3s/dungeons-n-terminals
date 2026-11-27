//! Drawing for the Companions tab: a card per companion (its numbers, each
//! roll on its own row), what gives companions, and the tab's popups.

use dnt_core::companions::{Card, Companions};
use dnt_data::{view, Library};
use dnt_rules::overlay::OverlaySet;
use dnt_rules::standard::ABILITIES;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::characters::{Popup, Sheet};
use crate::companions_tab::{self, Row};
use crate::render;
use crate::theme::Theme;
use crate::ui::{pane, pane_title, spread, truncate};
use crate::ui_sheet::{centered, draw_picker};

/// The narrowest a card is drawn.
const CARD_MIN: u16 = 44;
const CARD_MAX: u16 = 60;

pub fn draw(frame: &mut Frame, area: Rect, lib: &Library, overlays: &OverlaySet, sheet: &mut Sheet, theme: &Theme) {
    let all = companions_tab::of(sheet, lib, overlays);
    let rows = companions_tab::rows(&all.list);
    sheet.companion_cursor = sheet.companion_cursor.min(rows.len().saturating_sub(1));
    let here = rows.get(sheet.companion_cursor).cloned().filter(|_| sheet.popup.is_none());

    let [bar, _, cards] = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3)]).areas(area);
    draw_bar(frame, bar, sheet, &all, theme);

    if all.list.is_empty() {
        let block = pane(pane_title("Companions", true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
        let inner = block.inner(cards);
        frame.render_widget(block, cards);
        let lines = vec![
            Line::styled("No companions yet.", theme.muted),
            Line::default(),
            Line::styled("A spell that gives one asks for it when it is cast (Spells tab).", theme.dim),
            Line::default(),
            Line::from(vec![
                Span::styled(" a ", theme.keycap),
                Span::styled("  add a follower: any creature (or what a feature gives)", theme.dim),
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }

    // As many cards as fit, from the one under the cursor.
    let fit = ((cards.width + 1) / (CARD_MIN + 1)).max(1) as usize;
    let shown = fit.min(all.list.len());
    let width = ((cards.width + 1) / shown as u16).saturating_sub(1).min(CARD_MAX);
    let focus = here.as_ref().map_or(0, |(c, _)| *c);
    let first = focus.saturating_sub(shown - 1).min(all.list.len() - shown);
    let mut constraints = Vec::new();
    for i in 0..shown {
        if i > 0 {
            constraints.push(Constraint::Length(1));
        }
        constraints.push(Constraint::Length(width));
    }
    let areas = Layout::horizontal(constraints).split(cards);
    for (n, card) in all.list.iter().enumerate().skip(first).take(shown) {
        let at = areas[(n - first) * 2];
        let row = here.as_ref().filter(|(c, _)| *c == n).map(|(_, r)| r.clone());
        draw_card(frame, at, card, row.as_ref(), theme);
    }
    // More cards than fit: say so in the last shown one's corner.
    let hidden = all.list.len() - shown;
    if hidden > 0 {
        let at = areas[(shown - 1) * 2];
        let text = format!(" ← → {hidden} more ");
        let x = at.right().saturating_sub(text.width() as u16 + 2);
        frame.render_widget(Paragraph::new(Line::styled(text, theme.dim)), Rect::new(x, at.bottom().saturating_sub(1), 12, 1));
    }
}

/// The next roll's mode and the latest roll.
fn draw_bar(frame: &mut Frame, area: Rect, sheet: &Sheet, all: &Companions, theme: &Theme) {
    let away = all.list.iter().filter(|c| c.away).count();
    let mut left = vec![
        Span::styled(" Companions  ", theme.label),
        Span::styled(format!("{}", all.list.len() - away), Style::new().add_modifier(Modifier::BOLD)),
    ];
    if away > 0 {
        left.push(Span::styled(format!(" · {away} away"), theme.muted));
    }
    let mut right = Vec::new();
    if let Some(r) = sheet.session.roller.log.first() {
        let colour = match r.crit {
            Some(true) => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            Some(false) => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            None => Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        };
        right.push(Span::styled("Rolled  ", theme.label));
        right.push(Span::styled(format!("{}  ", truncate(&r.title, 40)), Style::new().add_modifier(Modifier::BOLD)));
        right.push(Span::styled(r.result.clone(), colour));
        if let Some(e) = r.extra.first() {
            right.push(Span::styled(format!("  {}", truncate(e, 36)), theme.muted));
        }
        right.push(Span::raw("   "));
    }
    match sheet.session.roller.next {
        dnt_rules::roll::Advantage::Normal => {}
        m => {
            right.push(Span::styled(format!(" next roll: {} ", m.label()), theme.category_active));
            right.push(Span::raw(" "));
        }
    }
    frame.render_widget(Paragraph::new(spread(left, right, area.width as usize)), area);
}

fn draw_card(frame: &mut Frame, area: Rect, card: &Card, row: Option<&Row>, theme: &Theme) {
    let focused = row.is_some();
    let title = if card.away { format!("{} · away", card.name) } else { card.name.clone() };
    let block = pane(pane_title(&title, focused, theme), focused, theme).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    // Away: the whole card faint.
    let ink = if card.away { theme.dim } else { Style::new() };
    let on = |r: Row| if row == Some(&r) { theme.selected } else { ink };
    let mut lines: Vec<Line> = Vec::new();
    let mut cursor_line = 0;
    let mut push = |lines: &mut Vec<Line<'static>>, line: Line<'static>, selected: bool| {
        if selected {
            cursor_line = lines.len();
        }
        lines.push(line);
    };

    // Who it is.
    let base = on(Row::Name);
    let mut who = vec![Span::styled(card.kind.clone(), base.fg(theme.accent).add_modifier(Modifier::BOLD))];
    if card.creature != card.name || card.kind == "Follower" {
        who.push(Span::styled(format!(" · {}", card.creature), base));
    }
    let cr = card.numbers.as_ref().and_then(|n| n.cr.clone()).map(|cr| format!("CR {cr} ")).unwrap_or_default();
    push(&mut lines, spread(who, vec![Span::styled(cr, base.patch(theme.muted))], width).style(base), base != ink);
    push(&mut lines, Line::styled(truncate(&card.type_line, width), theme.muted), false);
    lines.push(Line::default());

    let Some(n) = &card.numbers else {
        lines.push(Line::styled(format!("{} is not in the data.", card.creature_id), Style::new().fg(Color::Red)));
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    };

    // Hit points.
    let base = on(Row::Hp);
    let max = n.hp_max.max(1);
    let bar_w = width.saturating_sub(24).clamp(6, 24);
    let filled = ((n.hp as f64 / max as f64) * bar_w as f64).round() as usize;
    let colour = match n.hp * 100 / max {
        0..=25 => Color::Red,
        26..=50 => Color::Yellow,
        _ => Color::Green,
    };
    let mut hp = vec![
        Span::styled(format!("{:<12}", "Hit Points"), base.patch(theme.label)),
        Span::styled("█".repeat(filled.min(bar_w)), base.fg(if card.away { Color::DarkGray } else { colour })),
        Span::styled("░".repeat(bar_w - filled.min(bar_w)), base.patch(theme.dim)),
        Span::styled(format!("  {}", n.hp), base.add_modifier(Modifier::BOLD)),
        Span::styled(format!("/{}", n.hp_max), base.patch(theme.muted)),
    ];
    if n.temp_hp > 0 {
        hp.push(Span::styled(format!("  +{} temp", n.temp_hp), base.fg(Color::Cyan)));
    }
    push(&mut lines, Line::from(hp).style(base), base != ink);

    // Armor Class, initiative (rolls), speed.
    let base = on(Row::Initiative);
    push(
        &mut lines,
        Line::from(vec![
            Span::styled(format!("{:<12}", "Armor Class"), ink.patch(theme.label)),
            Span::styled(format!("{:<6}", n.ac), ink.add_modifier(Modifier::BOLD)),
            Span::styled("Initiative ", base.patch(theme.label)),
            Span::styled(format!("{:+} ", n.initiative), base.fg(theme.accent).add_modifier(Modifier::BOLD)),
        ]),
        base != ink,
    );
    lines.push(Line::from(vec![
        Span::styled(format!("{:<12}", "Speed"), theme.label),
        Span::styled(truncate(&n.speed, width.saturating_sub(12)), ink),
    ]));
    lines.push(Line::default());

    // Abilities: the check and the save roll.
    lines.push(Line::from(vec![Span::styled(format!("{:<14}", ""), theme.label), Span::styled("score  mod save", theme.label)]));
    for a in &n.abilities {
        let base = on(Row::Ability(a.key.clone()));
        let name = ABILITIES.iter().find(|(k, _)| *k == a.key).map_or(a.key.as_str(), |(_, n)| n);
        let proficient = a.save != a.modifier;
        push(
            &mut lines,
            Line::from(vec![
                Span::styled(format!("{name:<14}"), base),
                Span::styled(format!("{:>5}", a.score), base),
                Span::styled(format!("{:>5}", format!("{:+}", a.modifier)), base.fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:>5} ", format!("{:+}", a.save)), base),
                Span::styled(if proficient { "●" } else { "○" }, if proficient { base.fg(theme.accent) } else { base.patch(theme.dim) }),
            ])
            .style(base),
            base != ink,
        );
    }

    if !n.skills.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("Skills", theme.heading));
        for s in &n.skills {
            let base = on(Row::Skill(s.key.clone()));
            push(
                &mut lines,
                spread(
                    vec![Span::styled(s.name.clone(), base)],
                    vec![Span::styled(format!("{:+} ", s.bonus), base.add_modifier(Modifier::BOLD))],
                    width,
                )
                .style(base),
                base != ink,
            );
        }
    }

    lines.push(Line::default());
    let mut senses = format!("Passive Perception {}", n.passive_perception);
    if !n.senses.is_empty() {
        senses.push_str(&format!(" · {}", n.senses));
    }
    for l in wrap(&senses, width) {
        lines.push(Line::styled(l, theme.muted));
    }
    if !n.languages.is_empty() {
        for l in wrap(&format!("Languages: {}", n.languages), width) {
            lines.push(Line::styled(l, theme.muted));
        }
    }

    if !n.actions.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("Actions", theme.heading));
        for a in &n.actions {
            let base = on(Row::Action(a.name.clone()));
            let mut right = Vec::new();
            if let Some(atk) = a.attack {
                right.push(Span::styled(format!("{atk:+}  "), base.fg(theme.accent).add_modifier(Modifier::BOLD)));
            }
            if let Some(dc) = a.dc {
                let save = a.save.as_deref().map(|s| format!(" {}", s.to_uppercase())).unwrap_or_default();
                right.push(Span::styled(format!("DC {dc}{save}  "), base.fg(theme.accent).add_modifier(Modifier::BOLD)));
            }
            if let Some(d) = &a.damage {
                right.push(Span::styled(format!("{d} "), base));
            }
            if let Some(h) = &a.heal {
                right.push(Span::styled(format!("heals {h} "), base));
            }
            let used: usize = right.iter().map(|s| s.content.width()).sum();
            if a.name.width() + used + 2 > width {
                // A long name: the numbers on a line of their own.
                push(&mut lines, Line::styled(truncate(&a.name, width), base).style(base), base != ink);
                lines.push(spread(vec![Span::styled("", base)], right, width).style(base));
            } else {
                push(&mut lines, spread(vec![Span::styled(a.name.clone(), base)], right, width).style(base), base != ink);
            }
        }
    }

    if !card.conditions.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(format!("{:<12}", "Conditions"), theme.label),
            Span::styled(card.conditions.join(" · "), Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]));
    }
    if !card.notes.is_empty() {
        lines.push(Line::default());
        for l in card.notes.lines().flat_map(|l| wrap(l, width)) {
            lines.push(Line::styled(l, theme.muted));
        }
    }

    let height = inner.height as usize;
    let scroll = cursor_line.saturating_sub(height.saturating_sub(3));
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
}

/// Words in lines of at most `width`.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// The Companions popups; false when the popup is another tab's.
pub fn draw_popup(frame: &mut Frame, area: Rect, lib: &Library, sheet: &mut Sheet, theme: &Theme) -> bool {
    match sheet.popup.clone() {
        Some(Popup::CompanionPick(picker, pick)) => {
            let long = matches!(pick, companions_tab::Pick::Creature(_));
            let (w, h) = if long { (84, 28) } else { (64, picker.items.len() as u16 + 7) };
            let rect = centered(area, w.min(area.width), h.min(area.height.saturating_sub(4)).max(8));
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&picker.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" type to filter · enter choose · esc cancel ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            draw_picker(frame, inner, &picker, theme);
        }
        Some(Popup::CompanionInput(input, text)) => {
            let width = (input.title.width() as u16 + 8).max(44);
            let rect = centered(area, width, 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&input.title, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" enter · esc cancel ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled("❯ ", Style::new().fg(theme.accent)), Span::raw(text.clone())])).block(block),
                rect,
            );
            frame.set_cursor_position((inner.x + 2 + text.width() as u16, inner.y));
        }
        Some(Popup::CompanionConditions(index, cursor)) => {
            let list = dnt_core::sheet::conditions(&sheet.session);
            let had = sheet.session.character().companions.get(index).map(|c| c.conditions.clone()).unwrap_or_default();
            let name = sheet.session.character().companions.get(index).map(|c| c.name.clone()).filter(|n| !n.is_empty());
            let name = name.or_else(|| sheet.session.companions.get(index).and_then(|x| x.as_ref()).map(|(s, _)| s.name.clone()));
            let rect = centered(area, 96.min(area.width), list.len() as u16 + 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title(&format!("{}: conditions", name.unwrap_or_default()), true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" space toggle · esc done ", theme.dim).right_aligned());
            let lines: Vec<Line> = list
                .iter()
                .enumerate()
                .map(|(i, (c, _))| {
                    let on = had.iter().any(|h| h.eq_ignore_ascii_case(&c.name));
                    let base = if i == cursor { theme.selected } else { Style::new() };
                    Line::from(vec![
                        Span::styled(if on { "◉ " } else { "○ " }, if on { base.fg(Color::Magenta) } else { base.patch(theme.dim) }),
                        Span::styled(format!("{:<15}", c.name), if on { base.add_modifier(Modifier::BOLD) } else { base }),
                        Span::styled(c.summary.clone(), base.patch(theme.dim)),
                    ])
                    .style(base)
                })
                .collect();
            frame.render_widget(Paragraph::new(lines).block(block), rect);
        }
        Some(Popup::RemoveCompanion(index)) => {
            let card = sheet.session.character().companions.get(index).map(|c| c.name.clone()).filter(|n| !n.is_empty());
            let name = card.or_else(|| sheet.session.companions.get(index).and_then(|x| x.as_ref()).map(|(s, _)| s.name.clone()));
            let name = name.unwrap_or_else(|| "the companion".into());
            let text = format!("{name} leaves the character?  ");
            let rect = centered(area, (text.width() as u16 + 24).max(48), 5);
            frame.render_widget(Clear, rect);
            let block = pane(pane_title("Remove the companion", true, theme), true, theme).padding(Padding::new(2, 2, 1, 0));
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(text),
                    Span::styled(" y ", theme.keycap),
                    Span::styled(" yes  ", theme.dim),
                    Span::styled(" n ", theme.keycap),
                    Span::styled(" no", theme.dim),
                ]))
                .block(block),
                rect,
            );
        }
        Some(Popup::StatBlock(e, scroll)) => {
            let rect = centered(area, 88.min(area.width), area.height.saturating_sub(2));
            frame.render_widget(Clear, rect);
            let entity = lib.entity(e);
            let block = pane(pane_title(&entity.name, true, theme), true, theme)
                .padding(Padding::new(2, 2, 1, 0))
                .title_bottom(Line::styled(" j k scroll · esc close ", theme.dim).right_aligned());
            let inner = block.inner(rect);
            let doc = view::document(lib, e, &sheet.session.filter);
            let mut lines = vec![Line::styled(format!("{}  ·  {}", entity.name, entity.source), theme.title), Line::default()];
            lines.extend(render::render(&doc, inner.width, None, theme).lines);
            let max = lines.len().saturating_sub(inner.height as usize) as u16;
            let scroll = scroll.min(max);
            sheet.popup = Some(Popup::StatBlock(e, scroll));
            frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)).block(block), rect);
        }
        _ => return false,
    }
    true
}
