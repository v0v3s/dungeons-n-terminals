//! The Companions tab: the creatures serving the character (a familiar, a
//! steed, followers), their rows under the cursor, and the keys and popups
//! that summon, roll for, hurt, heal and dismiss them.

use dnt_core::companions::{self, Card, Companions};
use dnt_core::rolls::CompanionRoll;
use dnt_core::Command;
use dnt_data::Library;
use dnt_rules::overlay::OverlaySet;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::characters::{run, Action, Popup, Sheet};
use crate::picker::{PickItem, Picked, Picker};

/// A row of a companion's card the cursor can rest on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    /// Its name: enter opens its menu.
    Name,
    Hp,
    Initiative,
    Ability(String),
    Skill(String),
    Action(String),
}

/// The rows of every card, in cursor order: (card, row).
pub fn rows(list: &[Card]) -> Vec<(usize, Row)> {
    let mut out = Vec::new();
    for (i, c) in list.iter().enumerate() {
        out.push((i, Row::Name));
        let Some(n) = &c.numbers else { continue };
        out.push((i, Row::Hp));
        out.push((i, Row::Initiative));
        out.extend(n.abilities.iter().map(|a| (i, Row::Ability(a.key.clone()))));
        out.extend(n.skills.iter().map(|s| (i, Row::Skill(s.key.clone()))));
        out.extend(n.actions.iter().map(|a| (i, Row::Action(a.name.clone()))));
    }
    out
}

/// The cards and what gives companions, now.
pub fn of(sheet: &Sheet, lib: &Library, overlays: &OverlaySet) -> Companions {
    companions::companions(&sheet.session, lib, overlays)
}

/// A companion being summoned: what gives it (and the types it may be),
/// the level its spell was cast at, the creature and type chosen so far.
#[derive(Debug, Clone, PartialEq)]
pub struct Summoning {
    /// The rule's id; `None` for a follower.
    pub giver: Option<String>,
    pub kind: String,
    pub types: Vec<String>,
    pub spell_level: Option<i64>,
    pub creature: String,
    pub creature_type: Option<String>,
}

/// What a companions list popup chooses.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    /// What to add: a giver (by its rule's id), or a follower.
    Add,
    /// The creature.
    Creature(Summoning),
    /// The creature's type.
    Type(Summoning),
    /// A companion's menu.
    Menu(usize),
}

/// What a companions text popup asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    Damage(usize),
    Heal(usize),
    TempHp(usize),
    Rename(usize),
    Notes(usize),
}

impl Ask {
    /// Only digits are typed.
    pub fn numeric(&self) -> bool {
        !matches!(self, Ask::Rename(_) | Ask::Notes(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub ask: Ask,
    pub title: String,
}

const FOLLOWER: &str = "follower";

/// Keys on the Companions tab.
pub fn key(sheet: &mut Sheet, key: KeyEvent, lib: &Library, overlays: &OverlaySet) -> Action {
    let all = of(sheet, lib, overlays);
    let list = &all.list;
    let rows = rows(list);
    let n = rows.len();
    let at = sheet.companion_cursor.min(n.saturating_sub(1));
    let here = rows.get(at).cloned();
    let card = here.as_ref().map(|(c, _)| &list[*c]);
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if n > 0 => sheet.companion_cursor = (at + 1).min(n - 1),
        KeyCode::Char('k') | KeyCode::Up => sheet.companion_cursor = at.saturating_sub(1),
        KeyCode::Char('g') | KeyCode::Home => sheet.companion_cursor = 0,
        KeyCode::Char('G') | KeyCode::End => sheet.companion_cursor = n.saturating_sub(1),
        // The next or previous companion, on its name.
        KeyCode::Right | KeyCode::Left if n > 0 => {
            let c = here.as_ref().map_or(0, |(c, _)| *c);
            let to = if key.code == KeyCode::Right { (c + 1).min(list.len() - 1) } else { c.saturating_sub(1) };
            sheet.companion_cursor = rows.iter().position(|(i, r)| *i == to && *r == Row::Name).unwrap_or(at);
        }
        KeyCode::Char('a') => return add(sheet, lib, overlays, &all),
        KeyCode::Char('v') => {
            let m = sheet.session.roller.cycle();
            return Action::Status(match m {
                dnt_rules::roll::Advantage::Normal => "Next roll: normal".into(),
                m => format!("Next roll: {}", m.label()),
            });
        }
        _ if card.is_none() => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('r' | 'R' | 'm' | '-' | '+' | '=' | 'x' | 'C' | 'i')) {
                return Action::Status("No companions: a adds one".into());
            }
        }
        KeyCode::Enter if matches!(here, Some((_, Row::Name))) => {
            sheet.popup = Some(menu(card.unwrap()));
        }
        KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Enter => {
            let (_, row) = here.unwrap();
            let card = card.unwrap();
            let what = match row {
                Row::Ability(k) if key.code == KeyCode::Char('R') => CompanionRoll::Save { key: k },
                Row::Ability(k) => CompanionRoll::Ability { key: k },
                Row::Skill(k) => CompanionRoll::Skill { key: k },
                Row::Initiative => CompanionRoll::Initiative,
                Row::Action(name) => CompanionRoll::Action { name },
                Row::Hp => return Action::Status(format!("{}: - damage · + heal", card.name)),
                Row::Name => return Action::Status("Roll an ability (R its save), a skill, initiative or an action".into()),
            };
            return match dnt_core::rolls::companion(sheet, card.index, &what) {
                Ok(r) => Action::Rolled { line: r.line(), to_notes: sheet.session.roller.to_notes, save: false },
                Err(e) => Action::Status(e),
            };
        }
        KeyCode::Char('m') => sheet.popup = Some(menu(card.unwrap())),
        KeyCode::Char('i') => return stat_block(sheet, card.unwrap()),
        KeyCode::Char('C') => sheet.popup = Some(Popup::CompanionConditions(card.unwrap().index, 0)),
        KeyCode::Char('x') => sheet.popup = Some(Popup::RemoveCompanion(card.unwrap().index)),
        KeyCode::Char('-') => {
            let c = card.unwrap();
            sheet.popup = Some(ask(Ask::Damage(c.index), format!("Damage to {}", c.name), String::new()));
        }
        KeyCode::Char('+' | '=') => {
            let c = card.unwrap();
            sheet.popup = Some(ask(Ask::Heal(c.index), format!("Heal {}", c.name), String::new()));
        }
        _ => {}
    }
    Action::None
}

fn ask(ask: Ask, title: String, text: String) -> Popup {
    Popup::CompanionInput(Input { ask, title }, text)
}

fn stat_block(sheet: &mut Sheet, card: &Card) -> Action {
    match card.entity {
        Some(e) => {
            sheet.popup = Some(Popup::StatBlock(e, 0));
            Action::None
        }
        None => Action::Status(format!("{} is not in the data", card.creature_id)),
    }
}

/// What can be added: the features that give companions (spells give
/// theirs when cast), then a follower; straight to the follower's
/// creatures when no feature gives any.
fn add(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, all: &Companions) -> Action {
    let features: Vec<&companions::Giver> = all.givers.iter().filter(|g| !g.id.starts_with("spell:")).collect();
    if features.is_empty() {
        return follower(sheet, lib, overlays);
    }
    let mut items: Vec<PickItem> = features
        .iter()
        .map(|g| {
            let mut item = PickItem::new(format!("{} · {}", g.kind, g.name), format!("{}/{}", g.have, g.most), g.id.clone());
            if g.have >= g.most {
                item.warning = Some(format!("gives {} at once", g.most));
            }
            item
        })
        .collect();
    items.push(PickItem::new("A follower…", "any creature", FOLLOWER));
    sheet.popup = Some(Popup::CompanionPick(Box::new(Picker::new("Add a companion", items)), Pick::Add));
    Action::None
}

fn follower(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet) -> Action {
    let items = companions::creature_choices(&sheet.session, lib, overlays, None);
    let s = Summoning {
        giver: None,
        kind: "Follower".into(),
        types: Vec::new(),
        spell_level: None,
        creature: String::new(),
        creature_type: None,
    };
    sheet.popup = Some(Popup::CompanionPick(Box::new(Picker::new("A follower: which creature?", items)), Pick::Creature(s)));
    Action::None
}

/// A cast spell gives a companion: ask its creature and type. `Some`
/// when it joined at once (one creature, no type to choose).
pub fn from_cast(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, summon: dnt_core::commands::Summon) -> Option<Action> {
    match begin(sheet, lib, overlays, &summon.giver, Some(summon.spell_level)) {
        Action::None => None,
        a => Some(a),
    }
}

/// Summon what `g` gives: its creature (unless it has one), then the rest.
fn begin(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, g: &companions::Giver, spell_level: Option<i64>) -> Action {
    let s = Summoning {
        giver: Some(g.id.clone()),
        kind: g.kind.clone(),
        types: g.types.clone(),
        spell_level,
        creature: String::new(),
        creature_type: None,
    };
    if let [(only, _, _)] = g.creatures.as_slice() {
        return with_creature(sheet, lib, overlays, s, only.clone());
    }
    let items = companions::creature_choices(&sheet.session, lib, overlays, Some(&g.id));
    let picker = Picker::new(format!("{}: which creature?", g.kind), items);
    sheet.popup = Some(Popup::CompanionPick(Box::new(picker), Pick::Creature(s)));
    Action::None
}

fn menu(card: &Card) -> Popup {
    let mut items = Vec::new();
    if card.entity.is_some() {
        items.push(PickItem::new("Stat block", "i", "stat"));
    }
    items.push(PickItem::new("Rename…", "", "rename"));
    items.push(PickItem::new("Temporary hit points…", "", "temp"));
    items.push(PickItem::new("Conditions…", "C", "conditions"));
    items.push(PickItem::new(if card.away { "Back" } else { "Away (dismissed)" }, "", "away"));
    items.push(PickItem::new("Notes…", "", "notes"));
    items.push(PickItem::new("Remove…", "x", "remove"));
    Popup::CompanionPick(Box::new(Picker::new(card.name.clone(), items)), Pick::Menu(card.index))
}

fn finish(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, s: Summoning) -> Action {
    let command = Command::AddCompanion {
        creature: s.creature,
        name: String::new(),
        from: s.giver,
        spell_level: s.spell_level,
        creature_type: s.creature_type,
    };
    match run(sheet, lib, overlays, command) {
        Action::Save(m) => {
            // On the newcomer's name.
            let list = of(sheet, lib, overlays).list;
            let last = list.len().saturating_sub(1);
            sheet.companion_cursor = rows(&list).iter().position(|(c, r)| *c == last && *r == Row::Name).unwrap_or(0);
            Action::Save(if sheet.tab == crate::characters::Tab::Companions { m } else { format!("{m} (Companions tab)") })
        }
        a => a,
    }
}

/// After the creature: its type, if there are types to choose.
fn with_creature(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, mut s: Summoning, creature: String) -> Action {
    s.creature = creature;
    if s.types.is_empty() {
        return finish(sheet, lib, overlays, s);
    }
    let items = s.types.iter().map(|t| PickItem::new(dnt_data::view::title_case(t), "", t.clone())).collect();
    let picker = Picker::new(format!("{}: which type?", s.kind), items);
    sheet.popup = Some(Popup::CompanionPick(Box::new(picker), Pick::Type(s)));
    Action::None
}

/// Keys in the Companions popups.
pub fn popup_key(sheet: &mut Sheet, popup: Popup, key: KeyEvent, lib: &Library, overlays: &OverlaySet) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    sheet.popup = None;
    match popup {
        Popup::CompanionPick(mut picker, pick) => {
            let value = match picker.key(key) {
                Some(Picked::One(v)) => v,
                Some(Picked::Many(_)) => return Action::None,
                None => {
                    if key.code != KeyCode::Esc {
                        sheet.popup = Some(Popup::CompanionPick(picker, pick));
                    }
                    return Action::None;
                }
            };
            match pick {
                Pick::Add if value == FOLLOWER => return follower(sheet, lib, overlays),
                Pick::Add => {
                    let all = of(sheet, lib, overlays);
                    let Some(g) = all.givers.iter().find(|g| g.id == value) else { return Action::None };
                    if g.have >= g.most {
                        return Action::Status(format!("{} gives {} at once", g.name, g.most));
                    }
                    return begin(sheet, lib, overlays, g, None);
                }
                Pick::Creature(s) => return with_creature(sheet, lib, overlays, s, value),
                Pick::Type(mut s) => {
                    s.creature_type = Some(value);
                    return finish(sheet, lib, overlays, s);
                }
                Pick::Menu(i) => {
                    let Some(card) = of(sheet, lib, overlays).list.into_iter().nth(i) else { return Action::None };
                    match value.as_str() {
                        "stat" => return stat_block(sheet, &card),
                        "rename" => sheet.popup = Some(ask(Ask::Rename(i), format!("Rename {}", card.name), card.name.clone())),
                        "temp" => sheet.popup = Some(ask(Ask::TempHp(i), format!("{}: temporary hit points", card.name), String::new())),
                        "conditions" => sheet.popup = Some(Popup::CompanionConditions(i, 0)),
                        "away" => return run(sheet, lib, overlays, Command::SetCompanionAway { index: i, away: !card.away }),
                        "notes" => sheet.popup = Some(ask(Ask::Notes(i), format!("{}: notes", card.name), card.notes.clone())),
                        "remove" => sheet.popup = Some(Popup::RemoveCompanion(i)),
                        _ => {}
                    }
                }
            }
            Action::None
        }
        Popup::CompanionInput(input, mut text) => {
            match key.code {
                KeyCode::Esc => return Action::None,
                KeyCode::Enter => {}
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char('u') if ctrl => text.clear(),
                KeyCode::Char(c) if !ctrl && (!input.ask.numeric() || c.is_ascii_digit()) => text.push(c),
                _ => {}
            }
            if key.code != KeyCode::Enter {
                sheet.popup = Some(Popup::CompanionInput(input, text));
                return Action::None;
            }
            let number = text.trim().parse::<i64>().ok();
            let command = match input.ask {
                Ask::Damage(index) => Command::CompanionHp { index, damage: number.unwrap_or(0) },
                Ask::Heal(index) => Command::CompanionHp { index, damage: -number.unwrap_or(0) },
                Ask::TempHp(index) => Command::CompanionTempHp { index, amount: number.unwrap_or(0) },
                Ask::Rename(index) => Command::RenameCompanion { index, name: text },
                Ask::Notes(index) => Command::SetCompanionNotes { index, notes: text },
            };
            if matches!(command, Command::CompanionHp { damage: 0, .. }) {
                return Action::None;
            }
            run(sheet, lib, overlays, command)
        }
        Popup::CompanionConditions(index, cursor) => {
            let names: Vec<String> = dnt_core::sheet::conditions(&sheet.session).into_iter().map(|(c, _)| c.name).collect();
            let n = names.len();
            let keep = |sheet: &mut Sheet, cursor: usize| sheet.popup = Some(Popup::CompanionConditions(index, cursor));
            match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'C') => {}
                KeyCode::Char('j') | KeyCode::Down => keep(sheet, (cursor + 1).min(n.saturating_sub(1))),
                KeyCode::Char('k') | KeyCode::Up => keep(sheet, cursor.saturating_sub(1)),
                KeyCode::Char(' ') | KeyCode::Enter if cursor < n => {
                    let c = names[cursor].clone();
                    let has = sheet
                        .session
                        .character()
                        .companions
                        .get(index)
                        .is_some_and(|x| x.conditions.iter().any(|h| h.eq_ignore_ascii_case(&c)));
                    let action = run(sheet, lib, overlays, Command::SetCompanionCondition { index, condition: c, on: !has });
                    keep(sheet, cursor);
                    return action;
                }
                _ => keep(sheet, cursor),
            }
            Action::None
        }
        Popup::RemoveCompanion(index) => {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                let action = run(sheet, lib, overlays, Command::RemoveCompanion { index });
                let n = rows(&of(sheet, lib, overlays).list).len();
                sheet.companion_cursor = sheet.companion_cursor.min(n.saturating_sub(1));
                return action;
            }
            Action::None
        }
        Popup::StatBlock(e, scroll) => {
            let scroll = match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'i') => return Action::None,
                KeyCode::Char('j') | KeyCode::Down => scroll.saturating_add(1),
                KeyCode::Char('k') | KeyCode::Up => scroll.saturating_sub(1),
                KeyCode::Char(' ') | KeyCode::PageDown => scroll.saturating_add(10),
                KeyCode::PageUp => scroll.saturating_sub(10),
                KeyCode::Char('g') => 0,
                _ => scroll,
            };
            sheet.popup = Some(Popup::StatBlock(e, scroll));
            Action::None
        }
        other => {
            sheet.popup = Some(other);
            Action::None
        }
    }
}
