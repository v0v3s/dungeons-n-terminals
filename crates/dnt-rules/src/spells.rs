//! Spellcasting: per-class limits (cantrips, prepared/known spells,
//! spellbook), class spell lists, and spells granted by features
//! (`additionalSpells`: domain spells, lineage spells, Magic Initiate).

use std::collections::BTreeMap;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::formula::Formula;
use crate::value::Value;

/// How a class's chosen leveled spells are called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellList {
    /// Chosen daily from the class list (or spellbook).
    Prepared,
    /// Learned on level up.
    Known,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassLimits {
    pub cantrips: i64,
    /// Prepared or known leveled spells.
    pub spells: i64,
    pub list: SpellList,
    /// Spells in a Wizard's spellbook (`None` for other classes).
    pub spellbook: Option<i64>,
    /// Highest spell level the class can cast.
    pub max_level: i64,
}

fn at_level(v: Option<&Json>, level: i64) -> Option<i64> {
    v?.as_array()?.get((level - 1).max(0) as usize)?.as_i64()
}

/// Evaluate a 2014 prepared-spells formula such as `<$level$> + <$int_mod$>`.
fn prepared_formula(text: &str, level: i64, mods: &dyn Fn(&str) -> i64) -> Option<i64> {
    let mut src = text.replace("<$level$>", &level.to_string());
    for ab in ["str", "dex", "con", "int", "wis", "cha"] {
        src = src.replace(&format!("<${ab}_mod$>"), &format!("({})", mods(ab)));
    }
    match Formula::parse(&src).ok()?.eval(&std::collections::HashMap::<String, Value>::new()).ok()? {
        Value::Num(n) => Some(n.floor().max(1)),
        _ => None,
    }
}

/// Spellcasting limits of a class at a class level. `max_level` is the
/// highest spell level the class's own slots (or pact slots) allow.
pub fn class_limits(class: &Json, level: i64, max_level: i64, mods: &dyn Fn(&str) -> i64) -> ClassLimits {
    let cantrips = at_level(class.get("cantripProgression"), level).unwrap_or(0);
    let (spells, list) = if let Some(n) = at_level(class.get("preparedSpellsProgression"), level) {
        (n, SpellList::Prepared)
    } else if let Some(f) = class.get("preparedSpells").and_then(Json::as_str) {
        (prepared_formula(f, level, mods).unwrap_or(1), SpellList::Prepared)
    } else if let Some(n) = at_level(class.get("spellsKnownProgression"), level) {
        (n, SpellList::Known)
    } else {
        (0, SpellList::Known)
    };
    let spellbook = class
        .get("spellsKnownProgressionFixed")
        .and_then(Json::as_array)
        .map(|a| a.iter().take(level.max(0) as usize).filter_map(Json::as_i64).sum());
    ClassLimits { cantrips, spells, list, spellbook, max_level }
}

/// A spellcasting class of a character, with its limits.
#[derive(Debug, Clone)]
pub struct Caster {
    /// Index into `Build::classes`.
    pub class_index: usize,
    pub id: String,
    pub name: String,
    pub slug: String,
    pub level: i64,
    pub limits: ClassLimits,
    /// Spells the subclass adds to the class's list (`expanded`): spell
    /// refs (`spell:faerie fire|phb`) and filters (`level=1|class=Wizard`).
    pub expanded: Vec<Expanded>,
}

/// One entry of an expanded spell list.
#[derive(Debug, Clone, PartialEq)]
pub enum Expanded {
    Spell(String),
    Filter(String),
}

/// A subclass's expanded spell list at a class level: keys are class
/// levels (`"3"`), or spell levels (`"s1"`, 2014 Warlock patrons) that
/// apply once the caster can cast them.
pub fn expanded_list(subclass: &Json, class_level: i64, max_spell_level: i64) -> Vec<Expanded> {
    let mut out = Vec::new();
    for alt in subclass.get("additionalSpells").and_then(Json::as_array).into_iter().flatten() {
        for (k, list) in alt.get("expanded").and_then(Json::as_object).into_iter().flatten() {
            let open = match k.strip_prefix('s') {
                Some(n) => n.parse::<i64>().is_ok_and(|n| n <= max_spell_level),
                None => k.parse::<i64>().is_ok_and(|n| n <= class_level),
            };
            if !open {
                continue;
            }
            for entry in list.as_array().into_iter().flatten() {
                let e = match entry {
                    Json::String(r) => Expanded::Spell(format!("spell:{}", r.split('#').next().unwrap_or(r))),
                    Json::Object(o) => match o.get("all").and_then(Json::as_str) {
                        Some(f) => Expanded::Filter(f.to_string()),
                        None => continue,
                    },
                    _ => continue,
                };
                if !out.contains(&e) {
                    out.push(e);
                }
            }
        }
    }
    out
}

/// The character's spellcasting classes (including subclass casters such
/// as Eldritch Knights), with what each can prepare.
pub fn casters(lib: &Library, build: &crate::builder::Build, ev: &crate::engine::Evaluation) -> Vec<Caster> {
    let input = |k: String| match build.model.inputs.get(&k) {
        Some(Value::Num(n)) => n.floor(),
        _ => 0,
    };
    build
        .classes
        .iter()
        .enumerate()
        .filter(|(_, c)| c.casting_ability.is_some())
        .map(|(i, c)| {
            let own = (1..=9).rev().find(|n| input(format!("class.{}.slots.{n}", c.slug)) > 0).unwrap_or(0);
            let pact = input(format!("class.{}.slot_level", c.slug));
            let max_level = own.max(pact);
            // Subclass casters (Eldritch Knight) keep their progressions on the subclass.
            let data = match c.subclass {
                Some(s) if lib.entity(c.entity).data.get("cantripProgression").is_none() => &lib.entity(s).data,
                _ => &lib.entity(c.entity).data,
            };
            let mods = |ab: &str| ev.int(&format!("mod.{ab}")).unwrap_or(0);
            let expanded = c.subclass.map(|s| expanded_list(&lib.entity(s).data, c.level, max_level)).unwrap_or_default();
            Caster {
                class_index: i,
                id: c.id.clone(),
                name: c.name.clone(),
                slug: c.slug.clone(),
                level: c.level,
                limits: class_limits(data, c.level, max_level, &mods),
                expanded,
            }
        })
        .collect()
}

/// Whether a spell needs concentration.
pub fn concentration(lib: &Library, spell: EntityId) -> bool {
    lib.entity(spell)
        .data
        .get("duration")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .any(|d| d.get("concentration").and_then(Json::as_bool) == Some(true))
}

pub fn ritual(lib: &Library, spell: EntityId) -> bool {
    lib.entity(spell).data.get("meta").and_then(|m| m.get("ritual")).and_then(Json::as_bool) == Some(true)
}

pub fn spell_level(lib: &Library, spell: EntityId) -> i64 {
    lib.entity(spell).data.get("level").and_then(Json::as_i64).unwrap_or(0)
}

pub fn spell_id(lib: &Library, spell: EntityId) -> String {
    let e = lib.entity(spell);
    format!("spell:{}|{}", e.name, e.source)
}

/// What a caster can choose from: the class's list plus the subclass's
/// expanded spells, up to the highest spell level it can cast.
pub fn caster_spell_list(lib: &Library, c: &Caster, filter: &SourceFilter) -> Vec<EntityId> {
    let mut out = class_spell_list(lib, &c.name, c.limits.max_level, filter);
    for e in &c.expanded {
        let found = match e {
            Expanded::Spell(id) => crate::overlay::resolve_id(lib, id).into_iter().collect(),
            Expanded::Filter(f) => filter_spells(lib, f, filter),
        };
        for s in found {
            if spell_level(lib, s) <= c.limits.max_level && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out.sort_by(|&a, &b| spell_level(lib, a).cmp(&spell_level(lib, b)).then(lib.entity(a).name.cmp(&lib.entity(b).name)));
    out
}

/// Spells on a class's list (by class name, so 2014 classes see 2024
/// spells and vice versa) up to `max_level`, as allowed by `filter`.
pub fn class_spell_list(lib: &Library, class_name: &str, max_level: i64, filter: &SourceFilter) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            e.kind == Kind::Spell
                && filter.allows(e)
                && spell_level(lib, i) <= max_level
                && lib.spell_classes(i).iter().any(|c| c.name.eq_ignore_ascii_case(class_name))
        })
        .collect();
    out.sort_by(|&a, &b| spell_level(lib, a).cmp(&spell_level(lib, b)).then(lib.entity(a).name.cmp(&lib.entity(b).name)));
    out
}

/// How a granted spell is available.
#[derive(Debug, Clone, PartialEq)]
pub enum Grant {
    /// Always prepared (domain/oath spells).
    Prepared,
    /// Known (counts as a known spell or cantrip).
    Known,
    /// Cast without a slot: `None` = at will, `Some(n)` = n per long rest.
    Innate(Option<i64>),
}

/// A spell or a choice of spells granted by a feature.
#[derive(Debug, Clone, PartialEq)]
pub enum Granted {
    Spell {
        id: String,
        grant: Grant,
    },
    /// Pick `count` spells matching a filter like `level=0|class=Cleric`.
    Choice {
        filter: String,
        count: usize,
        grant: Grant,
    },
}

fn refs(v: &Json, grant: &Grant, out: &mut Vec<Granted>) {
    for r in v.as_array().into_iter().flatten() {
        match r {
            Json::String(s) => {
                // `dancing lights|xphb#c`: `#c` marks a cantrip.
                let s = s.split('#').next().unwrap_or(s);
                let (name, source) = s.split_once('|').unwrap_or((s, "phb"));
                out.push(Granted::Spell { id: format!("spell:{name}|{source}"), grant: grant.clone() });
            }
            Json::Object(o) => {
                if let Some(f) = o.get("choose").and_then(Json::as_str) {
                    out.push(Granted::Choice {
                        filter: f.to_string(),
                        count: o.get("count").and_then(Json::as_u64).unwrap_or(1) as usize,
                        grant: grant.clone(),
                    });
                }
            }
            _ => {}
        }
    }
}

/// Levels in a by-level map that apply at `level` (`_` always applies).
fn applicable(map: &serde_json::Map<String, Json>, level: i64) -> impl Iterator<Item = &Json> {
    map.iter().filter(move |(k, _)| k == &"_" || k.trim_end_matches('e').parse::<i64>().is_ok_and(|l| l <= level)).map(|(_, v)| v)
}

/// Spells granted by an entity's `additionalSpells` at `level` (class level
/// for subclasses, character level otherwise). Only the first alternative
/// is used; variants such as Magic Initiate; Cleric are separate entities.
pub fn granted_spells(data: &Json, level: i64) -> Vec<Granted> {
    let Some(alt) = data.get("additionalSpells").and_then(Json::as_array).and_then(|a| a.first()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, grant) in [("prepared", Grant::Prepared), ("known", Grant::Known)] {
        for v in alt.get(key).and_then(Json::as_object).into_iter().flat_map(|m| applicable(m, level)) {
            match v {
                Json::Object(inner) => inner.values().for_each(|x| refs(x, &grant, &mut out)),
                other => refs(other, &grant, &mut out),
            }
        }
    }
    for v in alt.get("innate").and_then(Json::as_object).into_iter().flat_map(|m| applicable(m, level)) {
        match v {
            Json::Object(kinds) => {
                for (kind, uses) in kinds {
                    match (kind.as_str(), uses) {
                        ("daily" | "rest", Json::Object(per)) => {
                            for (n, list) in per {
                                let count = n.trim_end_matches('e').parse().unwrap_or(1);
                                refs(list, &Grant::Innate(Some(count)), &mut out);
                            }
                        }
                        ("will" | "ritual", list) => refs(list, &Grant::Innate(None), &mut out),
                        _ => {}
                    }
                }
            }
            list => refs(list, &Grant::Innate(None), &mut out),
        }
    }
    out
}

/// Spells matching a data choose filter (`level=0|class=Cleric`,
/// `level=1;2|school=V`).
pub fn filter_spells(lib: &Library, filter_text: &str, filter: &SourceFilter) -> Vec<EntityId> {
    let mut levels: Option<Vec<i64>> = None;
    let mut classes: Option<Vec<String>> = None;
    let mut schools: Option<Vec<String>> = None;
    for part in filter_text.split('|') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let values: Vec<String> = v.split(';').map(|s| s.trim().to_lowercase()).collect();
        match k.trim().to_lowercase().as_str() {
            "level" => levels = Some(values.iter().filter_map(|l| l.parse().ok()).collect()),
            "class" => classes = Some(values),
            "school" => schools = Some(values),
            _ => {}
        }
    }
    let mut out: Vec<EntityId> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            if e.kind != Kind::Spell || !filter.allows(e) {
                return false;
            }
            let lvl = spell_level(lib, i);
            let school = e.data.get("school").and_then(Json::as_str).unwrap_or("").to_lowercase();
            levels.as_ref().is_none_or(|l| l.contains(&lvl))
                && schools.as_ref().is_none_or(|s| s.contains(&school))
                && classes.as_ref().is_none_or(|c| lib.spell_classes(i).iter().any(|sc| c.contains(&sc.name.to_lowercase())))
        })
        .collect();
    out.sort_by(|&a, &b| spell_level(lib, a).cmp(&spell_level(lib, b)).then(lib.entity(a).name.cmp(&lib.entity(b).name)));
    out
}

/// Describe a choose filter: `level=0|class=Cleric` → `Cleric cantrip`.
pub fn describe_filter(filter_text: &str) -> String {
    let mut parts: BTreeMap<String, String> = BTreeMap::new();
    for part in filter_text.split('|') {
        if let Some((k, v)) = part.split_once('=') {
            parts.insert(k.trim().to_lowercase(), v.replace(';', "/"));
        }
    }
    let class = parts.get("class").cloned().unwrap_or_default();
    let school = parts
        .get("school")
        .map(|s| s.split('/').map(|c| dnt_data::view::spell_school(&c.to_uppercase())).collect::<Vec<_>>().join("/"))
        .unwrap_or_default();
    let levels: Vec<i64> = parts.get("level").map(|l| l.split('/').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    let what = match levels.as_slice() {
        [0] => "cantrip".to_string(),
        [] => "spell".to_string(),
        [l] => format!("spell (level {l})"),
        [first, .., last] => format!("spell (level {first}–{last})"),
    };
    [class, school, what].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// How a spell is resolved and what it rolls.
#[derive(Debug, Clone, PartialEq)]
pub struct SpellRoll {
    /// Makes a spell attack roll.
    pub attack: bool,
    /// The saving throw targets make (`dexterity`).
    pub save: Option<String>,
    /// Damage or healing dice at the level asked for.
    pub dice: Option<crate::value::Dice>,
    /// `fire`, `necrotic`, or `healing`.
    pub kind: String,
    /// The text adds the spellcasting ability modifier (Cure Wounds).
    pub add_modifier: bool,
}

/// `{@tag text}` for the given tags, in order: (tag, contents, the text
/// right after it).
fn tags_in(v: &Json, names: &[&str], out: &mut Vec<(String, String, String)>) {
    match v {
        Json::String(s) => {
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{@") {
                let after = &rest[start + 2..];
                let Some(end) = after.find('}') else { break };
                let inner = &after[..end];
                if let Some((tag, body)) = inner.split_once(' ') {
                    if names.contains(&tag) {
                        let follow: String = after[end + 1..].chars().take(60).collect();
                        out.push((tag.to_string(), body.to_string(), follow));
                    }
                }
                rest = &after[end..];
            }
        }
        Json::Array(a) => a.iter().for_each(|x| tags_in(x, names, out)),
        Json::Object(o) => o.values().for_each(|x| tags_in(x, names, out)),
        _ => {}
    }
}

/// What casting `spell` rolls: for cantrips at `character_level`, for
/// leveled spells at `slot_level` (their own level or higher).
pub fn spell_roll(lib: &Library, spell: EntityId, character_level: i64, slot_level: i64) -> SpellRoll {
    let d = &lib.entity(spell).data;
    let level = spell_level(lib, spell);
    let attack = d.get("spellAttack").is_some();
    let save = d.get("savingThrow").and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_str).map(str::to_string);
    let damage_types: Vec<&str> = d.get("damageInflict").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).collect();
    let mut kind = damage_types.join(" or ");
    let mut add_modifier = false;
    // Cantrips: the tier for the character's level.
    let scaling = d.get("scalingLevelDice").map(|s| match s {
        Json::Array(a) => a.first().cloned().unwrap_or(Json::Null),
        other => other.clone(),
    });
    let mut dice = scaling.as_ref().and_then(|s| s.get("scaling")).and_then(Json::as_object).and_then(|tiers| {
        tiers
            .iter()
            .filter_map(|(k, v)| Some((k.parse::<i64>().ok()?, v.as_str()?)))
            .filter(|(l, _)| *l <= character_level.max(1))
            .max_by_key(|(l, _)| *l)
            .and_then(|(_, t)| crate::roll::parse_dice(t))
    });
    if dice.is_none() {
        let mut found = Vec::new();
        tags_in(d.get("entries").unwrap_or(&Json::Null), &["damage", "dice"], &mut found);
        let base = found.iter().find(|(t, _, _)| t == "damage").or_else(|| found.iter().find(|(t, _, _)| t == "dice"));
        if let Some((tag, text, follow)) = base {
            let follow = follow.to_lowercase();
            add_modifier = follow.trim_start().starts_with("plus your spellcasting ability modifier")
                || follow.trim_start().starts_with("+ your spellcasting ability modifier");
            if tag == "dice" && kind.is_empty() {
                kind = "healing".into();
            }
            dice = crate::roll::parse_dice(text.split('|').next().unwrap_or(text));
            // Higher slots: `{@scaledamage 8d6|3-9|1d6}` adds 1d6 per level.
            let mut steps = Vec::new();
            tags_in(d.get("entriesHigherLevel").unwrap_or(&Json::Null), &["scaledamage", "scaledice"], &mut steps);
            if let (Some(base), Some((_, step, _))) = (dice.as_mut(), steps.first()) {
                let per = step.split('|').nth(2).and_then(crate::roll::parse_dice);
                let extra = (slot_level - level).max(0);
                if let Some(per) = per {
                    for _ in 0..extra {
                        *base = base.plus(&per);
                    }
                }
            }
        }
    }
    SpellRoll { attack, save, dice, kind, add_modifier }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn limits_2024_and_2014() {
        let wizard24 = json!({"cantripProgression": [3, 3, 3, 4], "preparedSpellsProgression": [4, 5, 6, 7],
            "spellsKnownProgressionFixed": [6, 2, 2, 2]});
        let l = class_limits(&wizard24, 3, 2, &|_| 3);
        assert_eq!((l.cantrips, l.spells, l.list, l.spellbook), (3, 6, SpellList::Prepared, Some(10)));

        let cleric14 = json!({"cantripProgression": [3, 3, 3], "preparedSpells": "<$level$> + <$wis_mod$>"});
        let l = class_limits(&cleric14, 3, 2, &|a| if a == "wis" { 3 } else { 0 });
        assert_eq!((l.spells, l.list), (6, SpellList::Prepared));
        let paladin14 = json!({"preparedSpells": "<$level$> / 2 + <$cha_mod$>"});
        assert_eq!(class_limits(&paladin14, 2, 1, &|_| -1).spells, 1, "minimum one");

        let sorcerer14 = json!({"cantripProgression": [4], "spellsKnownProgression": [2, 3]});
        let l = class_limits(&sorcerer14, 2, 1, &|_| 0);
        assert_eq!((l.spells, l.list), (3, SpellList::Known));
    }

    #[test]
    fn grants() {
        let life = json!({"additionalSpells": [{"prepared": {"3": ["aid|xphb", "bless|xphb"], "5": ["revivify|xphb"]}}]});
        assert_eq!(granted_spells(&life, 4).len(), 2);
        assert_eq!(granted_spells(&life, 5).len(), 3);

        let mi = json!({"additionalSpells": [{"innate": {"_": {"daily": {"1": [{"choose": "level=1|class=Cleric"}]}}},
            "known": {"_": [{"choose": "level=0|class=Cleric", "count": 2}]}}]});
        let g = granted_spells(&mi, 1);
        assert!(g.contains(&Granted::Choice { filter: "level=0|class=Cleric".into(), count: 2, grant: Grant::Known }));
        assert!(g.contains(&Granted::Choice { filter: "level=1|class=Cleric".into(), count: 1, grant: Grant::Innate(Some(1)) }));

        let drow = json!({"additionalSpells": [{"known": {"1": ["dancing lights|xphb#c"]},
            "innate": {"3": {"daily": {"1": ["faerie fire|xphb"]}}, "5": {"daily": {"1": ["darkness|xphb"]}}}}]});
        let g = granted_spells(&drow, 3);
        assert_eq!(g.len(), 2);
        assert!(g.contains(&Granted::Spell { id: "spell:dancing lights|xphb".into(), grant: Grant::Known }));
        assert_eq!(describe_filter("level=0|class=Cleric"), "Cleric cantrip");
        assert_eq!(describe_filter("level=0;1;2|class=Wizard|school=V"), "Wizard Evocation spell (level 0–2)");
    }

    #[test]
    fn expanded_lists() {
        let fiend = json!({"additionalSpells": [{"expanded": {"s1": ["burning hands", "command"], "s3": ["fireball"]}}]});
        assert_eq!(
            expanded_list(&fiend, 3, 2),
            vec![Expanded::Spell("spell:burning hands".into()), Expanded::Spell("spell:command".into())],
            "level 3 spells once the caster has them"
        );
        assert_eq!(expanded_list(&fiend, 5, 3).len(), 3);
        let ek = json!({"additionalSpells": [{"expanded": {"3": [{"all": "level=0|class=Wizard"}, {"all": "level=1|class=Wizard"}],
                                                             "7": [{"all": "level=2|class=Wizard"}]}}]});
        assert_eq!(expanded_list(&ek, 3, 1).len(), 2);
        assert_eq!(expanded_list(&ek, 7, 2)[2], Expanded::Filter("level=2|class=Wizard".into()));
    }

    #[test]
    fn spell_rolls() {
        let mut lib = Library::new();
        lib.load_json(&json!({"spell": [
            {"name": "Fire Bolt", "source": "T", "level": 0, "spellAttack": ["R"], "damageInflict": ["fire"],
             "scalingLevelDice": {"label": "Fire damage", "scaling": {"1": "1d10", "5": "2d10", "11": "3d10"}}, "entries": ["{@damage 1d10}"]},
            {"name": "Fireball", "source": "T", "level": 3, "savingThrow": ["dexterity"], "damageInflict": ["fire"],
             "entries": ["takes {@damage 8d6} Fire damage"], "entriesHigherLevel": [{"type": "entries", "entries": ["{@scaledamage 8d6|3-9|1d6}"]}]},
            {"name": "Cure Wounds", "source": "T", "level": 1, "entries": ["regains {@dice 2d8} plus your spellcasting ability modifier"], "entriesHigherLevel": ["{@scaledice 2d8|1-9|2d8}"]}
        ]}));
        let id = |n: &str| lib.get(&dnt_data::Key::new(Kind::Spell, &[n, "T"])).unwrap();
        let fb = spell_roll(&lib, id("Fire Bolt"), 6, 0);
        assert!(fb.attack && fb.save.is_none());
        assert_eq!((fb.dice.unwrap().to_string(), fb.kind), ("2d10".to_string(), "fire".to_string()));
        let fireball = spell_roll(&lib, id("Fireball"), 6, 5);
        assert_eq!(fireball.save.as_deref(), Some("dexterity"));
        assert_eq!(fireball.dice.unwrap().to_string(), "10d6");
        let cure = spell_roll(&lib, id("Cure Wounds"), 1, 2);
        assert!(cure.add_modifier && !fb.add_modifier);
        assert_eq!((cure.dice.unwrap().to_string(), cure.kind), ("4d8".to_string(), "healing".to_string()));
    }
}
