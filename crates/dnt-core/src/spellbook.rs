//! A character's spells as rows to show: per casting class and per feature
//! that grants spells, favorites first. Also the lists to choose spells and
//! slots from, and the changes behind the spell commands.

use dnt_data::{EntityId, Library, SourceFilter};
use dnt_rules::character::Character;
use dnt_rules::overlay::resolve_id;
use dnt_rules::play;
use dnt_rules::spells::{self, Caster, Grant, Granted};
use dnt_rules::Evaluation;
use serde::{Deserialize, Serialize};

use crate::options::Choice;
use crate::session::CharacterSession;

/// Where a class spell sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    ClassSpell {
        caster: usize,
        id: String,
        slot: Slot,
    },
    GrantHeader(usize),
    GrantSpell {
        grant: usize,
        id: String,
        how: Grant,
        chosen_key: Option<String>,
    },
    /// A grant choice with picks still missing.
    GrantChoice {
        grant: usize,
        key: String,
        filter: String,
        missing: usize,
        how: Grant,
    },
    FavoritesHeader,
    /// A favorite spell, repeated at the top; behaves like its own row.
    Favorite(Box<Row>),
}

impl Row {
    /// The row a favorite stands for (the row itself otherwise).
    pub fn inner(&self) -> &Row {
        match self {
            Row::Favorite(r) => r,
            r => r,
        }
    }

    pub fn spell_id(&self) -> Option<&str> {
        match self.inner() {
            Row::ClassSpell { id, .. } | Row::GrantSpell { id, .. } => Some(id),
            _ => None,
        }
    }

    /// Where the spell on this row was added (to remove it from there).
    pub fn target(&self, casters: &[Caster]) -> Option<Target> {
        match self.inner() {
            Row::ClassSpell { caster, .. } => Some(Target::Class { class_id: casters.get(*caster)?.id.clone() }),
            Row::GrantSpell { chosen_key: Some(key), .. } => Some(Target::Grant { key: key.clone() }),
            _ => None,
        }
    }
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

pub fn rows(lib: &Library, s: &CharacterSession) -> Rows {
    let ch = s.character();
    let casters = spells::casters(lib, &s.build, &s.ev);
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
    for (gi, g) in s.build.granted.iter().enumerate() {
        rows.push(Row::GrantHeader(gi));
        for (i, grant) in g.grants.iter().enumerate() {
            match grant {
                Granted::Spell { id, grant } => {
                    rows.push(Row::GrantSpell { grant: gi, id: id.clone(), how: grant.clone(), chosen_key: None })
                }
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
    // Favorites first, in the order they were marked.
    let mut favorites = Vec::new();
    for fav in &ch.spells.favorites {
        if let Some(r) = rows.iter().find(|r| r.spell_id() == Some(fav.as_str())) {
            favorites.push(Row::Favorite(Box::new(r.clone())));
        }
    }
    if !favorites.is_empty() {
        rows.splice(0..0, std::iter::once(Row::FavoritesHeader).chain(favorites));
    }
    Rows { casters, rows }
}

/// Free places per casting class: (class, cantrips, prepared or known
/// spells) still to choose. Wizards' spellbooks are not counted.
pub fn spell_gaps(ch: &Character, casters: &[Caster]) -> Vec<(String, i64, i64)> {
    casters
        .iter()
        .filter_map(|c| {
            let mine = ch.spells.classes.get(&c.id).cloned().unwrap_or_default();
            let cantrips = (c.limits.cantrips - mine.cantrips.len() as i64).max(0);
            let spells = (c.limits.spells - mine.prepared.len() as i64).max(0);
            (cantrips + spells > 0).then(|| (c.name.clone(), cantrips, spells))
        })
        .collect()
}

/// "Wizard: 1 cantrip, 2 spells" for the status line.
pub fn describe_gaps(gaps: &[(String, i64, i64)]) -> String {
    gaps.iter()
        .map(|(name, cantrips, spells)| {
            let mut parts = Vec::new();
            if *cantrips > 0 {
                parts.push(format!("{cantrips} cantrip{}", if *cantrips == 1 { "" } else { "s" }));
            }
            if *spells > 0 {
                parts.push(format!("{spells} spell{}", if *spells == 1 { "" } else { "s" }));
            }
            format!("{name}: {}", parts.join(", "))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Where spells are added to (and removed from).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// A casting class; cantrips and leveled spells sort themselves (to the
    /// spellbook for Wizards).
    Class { class_id: String },
    /// A feature's spell choice (`<grant key>#<n>`).
    Grant { key: String },
}

/// Spells to choose from: a title, the choices (values are spell ids), how
/// many to pick, and where they go.
#[derive(Debug, Clone, PartialEq)]
pub struct SpellChoices {
    pub title: String,
    pub choices: Vec<Choice>,
    pub pick: usize,
    pub target: Target,
}

fn spell_choice(lib: &Library, e: EntityId) -> Choice {
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
    Choice::new(lib.entity(e).name.clone(), format!("{level} · {school}{tags} · {}", lib.entity(e).source), spells::spell_id(lib, e))
}

/// Spells a class can add (not already chosen).
pub fn class_choices(lib: &Library, ch: &Character, c: &Caster, filter: &SourceFilter) -> SpellChoices {
    let mine = ch.spells.classes.get(&c.id).cloned().unwrap_or_default();
    let have: Vec<&String> = mine.cantrips.iter().chain(&mine.prepared).chain(&mine.spellbook).collect();
    let choices = spells::caster_spell_list(lib, c, filter)
        .into_iter()
        .filter(|&e| !have.contains(&&spells::spell_id(lib, e)))
        .filter(|&e| spells::spell_level(lib, e) > 0 || c.limits.cantrips > 0)
        .map(|e| spell_choice(lib, e))
        .collect();
    let what = if c.limits.spellbook.is_some() { "cantrip or spellbook spell" } else { "spell" };
    SpellChoices {
        title: format!("{} {}: add a {what}", c.name, c.level),
        choices,
        pick: 1,
        target: Target::Class { class_id: c.id.clone() },
    }
}

/// Spells for a feature's choice (`filter_text` as the grant states it).
pub fn grant_choices(lib: &Library, ch: &Character, key: &str, filter_text: &str, missing: usize, filter: &SourceFilter) -> SpellChoices {
    let chosen = ch.spells.chosen.get(key).cloned().unwrap_or_default();
    let choices = spells::filter_spells(lib, filter_text, filter)
        .into_iter()
        .filter(|&e| !chosen.contains(&spells::spell_id(lib, e)))
        .map(|e| spell_choice(lib, e))
        .collect();
    SpellChoices {
        title: format!("Choose {} {}", missing, spells::describe_filter(filter_text)),
        choices,
        pick: missing.max(1),
        target: Target::Grant { key: key.to_string() },
    }
}

/// How a spell can be cast: values `slot:N`, `pact` or `ritual`. `None`
/// when there is no way (no slots at its level).
pub fn cast_choices(lib: &Library, s: &CharacterSession, spell: EntityId) -> Option<(String, Vec<Choice>)> {
    let (ch, ev) = (s.character(), &s.ev);
    let level = spells::spell_level(lib, spell) as usize;
    let mut items = Vec::new();
    for l in level.max(1)..=9 {
        let left = play::slots_left(ch, ev, l);
        if play::slot_max(ev, l) > 0 {
            let mut item = Choice::new(format!("Level {l} slot"), format!("{left} left"), format!("slot:{l}"));
            if left == 0 {
                item.warning = Some("none left".into());
            }
            items.push(item);
        }
    }
    let pact_level = match s.build.model.inputs.get("pact.level") {
        Some(dnt_rules::Value::Num(n)) => n.floor() as usize,
        _ => 0,
    };
    let pact_max = ev.int("pact.slots.max").unwrap_or(0);
    if pact_max > 0 && pact_level >= level {
        let left = pact_max - ch.state.pact_slots_used;
        let mut item = Choice::new(format!("Pact slot (level {pact_level})"), format!("{left} left"), "pact");
        if left == 0 {
            item.warning = Some("none left".into());
        }
        items.push(item);
    }
    if spells::ritual(lib, spell) {
        items.push(Choice::new("As a ritual", "+10 minutes, no slot", "ritual"));
    }
    (!items.is_empty()).then(|| (format!("Cast {}", lib.entity(spell).name), items))
}

/// Add spells, within the class's limits (cantrips, prepared or known
/// spells; a Wizard's spellbook has none).
pub(crate) fn add(lib: &Library, ch: &mut Character, casters: &[Caster], target: &Target, ids: Vec<String>) -> Result<(), String> {
    match target {
        Target::Class { class_id } => {
            let Some(c) = casters.iter().find(|c| &c.id == class_id) else { return Err(format!("{class_id} casts no spells")) };
            let l = &c.limits;
            let wizard = l.spellbook.is_some();
            let entry = ch.spells.classes.entry(class_id.clone()).or_default();
            let new_cantrips = ids.iter().filter(|id| resolve_id(lib, id).is_some_and(|e| spells::spell_level(lib, e) == 0)).count() as i64;
            let new_spells = ids.len() as i64 - new_cantrips;
            if new_cantrips > 0 && entry.cantrips.len() as i64 + new_cantrips > l.cantrips {
                return Err(format!("Cantrips {}/{}: remove one first (x)", entry.cantrips.len(), l.cantrips));
            }
            let what = match l.list {
                spells::SpellList::Prepared => "Prepared",
                spells::SpellList::Known => "Known",
            };
            if !wizard && new_spells > 0 && entry.prepared.len() as i64 + new_spells > l.spells {
                return Err(format!("{what} {}/{}: remove one first (x)", entry.prepared.len(), l.spells));
            }
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
        Target::Grant { key } => {
            let list = ch.spells.chosen.entry(key.clone()).or_default();
            for id in ids {
                if !list.contains(&id) {
                    list.push(id);
                }
            }
        }
    }
    Ok(())
}

/// Remove a spell. A Wizard's prepared spell stays in the spellbook.
pub(crate) fn remove(ch: &mut Character, target: &Target, id: &str) -> bool {
    match target {
        Target::Class { class_id } => {
            let Some(entry) = ch.spells.classes.get_mut(class_id) else { return false };
            if entry.prepared.iter().any(|s| s == id) {
                entry.prepared.retain(|s| s != id);
            } else {
                let before = entry.cantrips.len() + entry.spellbook.len();
                entry.cantrips.retain(|s| s != id);
                entry.spellbook.retain(|s| s != id);
                return entry.cantrips.len() + entry.spellbook.len() < before;
            }
            true
        }
        Target::Grant { key } => {
            let Some(list) = ch.spells.chosen.get_mut(key) else { return false };
            let before = list.len();
            list.retain(|s| s != id);
            list.len() < before
        }
    }
}

/// Whether the character has chosen a spell anywhere (a class or a feature).
pub(crate) fn has(ch: &Character, id: &str) -> bool {
    let classes = ch.spells.classes.values().flat_map(|c| c.cantrips.iter().chain(&c.prepared).chain(&c.spellbook));
    classes.chain(ch.spells.chosen.values().flatten()).any(|s| s == id)
}

/// Prepare a spellbook spell or unprepare a prepared one (Wizards), within
/// the prepared limit. Returns whether it is prepared now.
pub(crate) fn toggle_prepared(ch: &mut Character, casters: &[Caster], class_id: &str, id: &str) -> Result<bool, String> {
    let Some(c) = casters.iter().find(|c| c.id == class_id) else { return Err(format!("{class_id} casts no spells")) };
    if c.limits.spellbook.is_none() {
        return Err(format!("{} prepares no spells from a spellbook", c.name));
    }
    let max = c.limits.spells;
    let entry = ch.spells.classes.entry(class_id.to_string()).or_default();
    if entry.prepared.iter().any(|s| s == id) {
        entry.prepared.retain(|s| s != id);
        if !entry.spellbook.iter().any(|s| s == id) {
            entry.spellbook.push(id.to_string());
        }
        Ok(false)
    } else if !entry.spellbook.iter().any(|s| s == id) {
        Err("Not a spellbook spell".into())
    } else if entry.prepared.len() as i64 >= max {
        Err(format!("Prepared {}/{max}: unprepare one first (p)", entry.prepared.len()))
    } else {
        entry.prepared.push(id.to_string());
        Ok(true)
    }
}

/// Mark or unmark a spell as a favorite; returns whether it is one now.
pub(crate) fn toggle_favorite(ch: &mut Character, id: &str) -> bool {
    let favs = &mut ch.spells.favorites;
    if let Some(i) = favs.iter().position(|f| f == id) {
        favs.remove(i);
        false
    } else {
        favs.push(id.to_string());
        true
    }
}

/// Spend what a cast needs (`slot:N`, `pact`, `ritual` or `free` for an
/// innate cast) and track concentration. Returns a status message.
pub(crate) fn cast(lib: &Library, ch: &mut Character, ev: &Evaluation, spell: EntityId, how: &str) -> Result<String, String> {
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
            let l: usize = slot.trim_start_matches("slot:").parse().map_err(|_| format!("Cast how? `{slot}`"))?;
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
