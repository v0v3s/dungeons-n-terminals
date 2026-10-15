//! The Spells tab: rows per casting class and per granting feature, adding
//! and preparing spells, and casting from slots.

use dnt_data::{EntityId, Library, SourceFilter};
use dnt_rules::character::Character;
use dnt_rules::overlay::resolve_id;
use dnt_rules::play;
use dnt_rules::spells::{self, Caster, Grant, Granted};
use dnt_rules::Evaluation;

use crate::picker::{PickItem, Picker};

/// Where a class spell sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Cantrip,
    Prepared,
    /// In the spellbook but not prepared.
    Book,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// A casting class: index into the casters list.
    ClassHeader(usize),
    ClassSpell { caster: usize, id: String, slot: Slot },
    GrantHeader(usize),
    GrantSpell { grant: usize, id: String, how: Grant, chosen_key: Option<String> },
    /// A grant choice with picks still missing.
    GrantChoice { grant: usize, key: String, filter: String, missing: usize, how: Grant },
}

pub struct Rows {
    pub casters: Vec<Caster>,
    pub rows: Vec<Row>,
}

fn sorted(lib: &Library, ids: &[String]) -> Vec<String> {
    let mut v: Vec<(i64, String, String)> = ids
        .iter()
        .map(|id| {
            let (lvl, name) = resolve_id(lib, id).map_or((0, id.clone()), |e| (spells::spell_level(lib, e), lib.entity(e).name.clone()));
            (lvl, name, id.clone())
        })
        .collect();
    v.sort();
    v.into_iter().map(|(_, _, id)| id).collect()
}

pub fn rows(lib: &Library, ch: &Character, build: &dnt_rules::builder::Build, ev: &Evaluation) -> Rows {
    let casters = spells::casters(lib, build, ev);
    let mut rows = Vec::new();
    for (ci, c) in casters.iter().enumerate() {
        rows.push(Row::ClassHeader(ci));
        let mine = ch.spells.classes.get(&c.id).cloned().unwrap_or_default();
        for id in sorted(lib, &mine.cantrips) {
            rows.push(Row::ClassSpell { caster: ci, id, slot: Slot::Cantrip });
        }
        for id in sorted(lib, &mine.prepared) {
            rows.push(Row::ClassSpell { caster: ci, id, slot: Slot::Prepared });
        }
        for id in sorted(lib, &mine.spellbook) {
            if !mine.prepared.contains(&id) {
                rows.push(Row::ClassSpell { caster: ci, id, slot: Slot::Book });
            }
        }
    }
    for (gi, g) in build.granted.iter().enumerate() {
        rows.push(Row::GrantHeader(gi));
        for (i, grant) in g.grants.iter().enumerate() {
            match grant {
                Granted::Spell { id, grant } => rows.push(Row::GrantSpell { grant: gi, id: id.clone(), how: grant.clone(), chosen_key: None }),
                Granted::Choice { filter, count, grant } => {
                    let key = format!("{}#{i}", g.key);
                    let chosen = ch.spells.chosen.get(&key).cloned().unwrap_or_default();
                    for id in sorted(lib, &chosen) {
                        rows.push(Row::GrantSpell { grant: gi, id, how: grant.clone(), chosen_key: Some(key.clone()) });
                    }
                    if chosen.len() < *count {
                        rows.push(Row::GrantChoice {
                            grant: gi,
                            key,
                            filter: filter.clone(),
                            missing: count - chosen.len(),
                            how: grant.clone(),
                        });
                    }
                }
            }
        }
    }
    Rows { casters, rows }
}

/// What a spell picker adds to.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// A casting class; cantrips and leveled spells sort themselves (to the
    /// spellbook for Wizards).
    Class { caster: usize, class_id: String, wizard: bool },
    Grant { key: String },
}

fn spell_item(lib: &Library, e: EntityId) -> PickItem {
    let lvl = spells::spell_level(lib, e);
    let school = lib.entity(e).data.get("school").and_then(|s| s.as_str()).map(dnt_data::view::spell_school).unwrap_or("");
    let level = if lvl == 0 { "cantrip".to_string() } else { format!("level {lvl}") };
    let mut tags = Vec::new();
    if spells::concentration(lib, e) {
        tags.push("C");
    }
    if spells::ritual(lib, e) {
        tags.push("R");
    }
    let tags = if tags.is_empty() { String::new() } else { format!(" · {}", tags.join(" ")) };
    PickItem::new(lib.entity(e).name.clone(), format!("{level} · {school}{tags} · {}", lib.entity(e).source), spells::spell_id(lib, e))
}

/// A picker of spells a class can add (not already chosen).
pub fn class_picker(lib: &Library, ch: &Character, c: &Caster, filter: &SourceFilter) -> (Picker, Target) {
    let mine = ch.spells.classes.get(&c.id).cloned().unwrap_or_default();
    let have: Vec<&String> = mine.cantrips.iter().chain(&mine.prepared).chain(&mine.spellbook).collect();
    let items: Vec<PickItem> = spells::class_spell_list(lib, &c.name, c.limits.max_level, filter)
        .into_iter()
        .filter(|&e| !have.contains(&&spells::spell_id(lib, e)))
        .filter(|&e| spells::spell_level(lib, e) > 0 || c.limits.cantrips > 0)
        .map(|e| spell_item(lib, e))
        .collect();
    let wizard = c.limits.spellbook.is_some();
    let what = if wizard { "cantrip or spellbook spell" } else { "spell" };
    (
        Picker::new(format!("{} {}: add a {what}", c.name, c.level), items),
        Target::Class {
            caster: c.class_index,
            class_id: c.id.clone(),
            wizard,
        },
    )
}

pub fn grant_picker(lib: &Library, ch: &Character, key: &str, filter_text: &str, missing: usize, filter: &SourceFilter) -> (Picker, Target) {
    let chosen = ch.spells.chosen.get(key).cloned().unwrap_or_default();
    let items: Vec<PickItem> = spells::filter_spells(lib, filter_text, filter)
        .into_iter()
        .filter(|&e| !chosen.contains(&spells::spell_id(lib, e)))
        .map(|e| spell_item(lib, e))
        .collect();
    let title = format!("Choose {} {}", missing, spells::describe_filter(filter_text));
    let picker = if missing > 1 { Picker::multi(title, items, missing) } else { Picker::new(title, items) };
    (picker, Target::Grant { key: key.to_string() })
}

/// Add picked spells to the character.
pub fn add(lib: &Library, ch: &mut Character, target: &Target, ids: Vec<String>) {
    match target {
        Target::Class { class_id, wizard, .. } => {
            let entry = ch.spells.classes.entry(class_id.clone()).or_default();
            for id in ids {
                let cantrip = resolve_id(lib, &id).is_some_and(|e| spells::spell_level(lib, e) == 0);
                let list = match (cantrip, wizard) {
                    (true, _) => &mut entry.cantrips,
                    (false, true) => &mut entry.spellbook,
                    (false, false) => &mut entry.prepared,
                };
                if !list.contains(&id) {
                    list.push(id);
                }
            }
        }
        Target::Grant { key } => ch.spells.chosen.entry(key.clone()).or_default().extend(ids),
    }
}

/// Remove the spell on a row.
pub fn remove(ch: &mut Character, casters: &[Caster], row: &Row) -> bool {
    match row {
        Row::ClassSpell { caster, id, slot } => {
            let Some(entry) = ch.spells.classes.get_mut(&casters[*caster].id) else { return false };
            // For Wizards, removing a prepared spell keeps it in the spellbook.
            let list = match slot {
                Slot::Cantrip => &mut entry.cantrips,
                Slot::Prepared => &mut entry.prepared,
                Slot::Book => &mut entry.spellbook,
            };
            list.retain(|s| s != id);
            true
        }
        Row::GrantSpell { id, chosen_key: Some(key), .. } => {
            if let Some(list) = ch.spells.chosen.get_mut(key) {
                list.retain(|s| s != id);
            }
            true
        }
        _ => false,
    }
}

/// Toggle preparing a spellbook spell (Wizards).
pub fn toggle_prepared(ch: &mut Character, casters: &[Caster], row: &Row) -> Option<bool> {
    let Row::ClassSpell { caster, id, slot } = row else { return None };
    casters[*caster].limits.spellbook?;
    let entry = ch.spells.classes.entry(casters[*caster].id.clone()).or_default();
    match slot {
        Slot::Book => {
            entry.prepared.push(id.clone());
            Some(true)
        }
        Slot::Prepared => {
            entry.prepared.retain(|s| s != id);
            if !entry.spellbook.contains(id) {
                entry.spellbook.push(id.clone());
            }
            Some(false)
        }
        Slot::Cantrip => None,
    }
}

/// Slot choices for casting a spell of `level`: values `slot:N` or `pact`.
pub fn cast_picker(lib: &Library, ch: &Character, ev: &Evaluation, build: &dnt_rules::builder::Build, spell: EntityId) -> Option<Picker> {
    let level = spells::spell_level(lib, spell) as usize;
    let mut items = Vec::new();
    for l in level.max(1)..=9 {
        let left = play::slots_left(ch, ev, l);
        if play::slot_max(ev, l) > 0 {
            let mut item = PickItem::new(format!("Level {l} slot"), format!("{left} left"), format!("slot:{l}"));
            if left == 0 {
                item.warning = Some("none left".into());
            }
            items.push(item);
        }
    }
    let pact_level = match build.model.inputs.get("pact.level") {
        Some(dnt_rules::Value::Num(n)) => n.floor() as usize,
        _ => 0,
    };
    let pact_max = ev.int("pact.slots.max").unwrap_or(0);
    if pact_max > 0 && pact_level >= level {
        let left = pact_max - ch.state.pact_slots_used;
        let mut item = PickItem::new(format!("Pact slot (level {pact_level})"), format!("{left} left"), "pact");
        if left == 0 {
            item.warning = Some("none left".into());
        }
        items.push(item);
    }
    if spells::ritual(lib, spell) {
        items.push(PickItem::new("As a ritual", "+10 minutes, no slot", "ritual"));
    }
    (!items.is_empty()).then(|| Picker::new(format!("Cast {}", lib.entity(spell).name), items))
}

/// Spend what a cast needs and track concentration. Returns a status message.
pub fn cast(lib: &Library, ch: &mut Character, ev: &Evaluation, spell: EntityId, how: &str) -> Result<String, String> {
    let name = lib.entity(spell).name.clone();
    match how {
        "pact" => {
            if !play::spend_pact_slot(ch, ev, 1) {
                return Err("No pact slots left".into());
            }
        }
        "ritual" => {}
        "free" => {
            let key = format!("innate_{}", dnt_rules::class_table::slug(&name));
            if !play::spend_resource(ch, ev, &key, 1) {
                return Err(format!("No free casts of {name} left"));
            }
        }
        slot => {
            let l: usize = slot.trim_start_matches("slot:").parse().unwrap_or(1);
            if !play::spend_slot(ch, ev, l, 1) {
                return Err(format!("No level {l} slots left"));
            }
        }
    }
    let mut msg = format!("Cast {name}");
    if spells::concentration(lib, spell) {
        if let Some(prev) = ch.state.concentration.replace(name.clone()) {
            if prev != name {
                msg.push_str(&format!(" (concentration on {prev} ends)"));
            }
        }
    }
    Ok(msg)
}
