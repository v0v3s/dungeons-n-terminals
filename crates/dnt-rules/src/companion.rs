//! Companions: creatures that serve a character. A companion's sheet is
//! its stat block's numbers as stats, changed by the rules of what gives
//! it (`[[feature.companion]]`), whose formulas know the owner's stats
//! (`owner.<stat>`) and the level a spell was cast at (`spell_level`).
//!
//! Its attacks and saves come from the stat block's actions (`{@hit 4}`,
//! `{@damage 1d6 + 2}`, `{@dc 13}`) as action rules, so they roll like a
//! character's: stats `action.<name>.attack`, `.dc`, `.damage`.

use std::collections::HashMap;

use dnt_data::creature;
use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::Companion;
use crate::class_table::slug;
use crate::engine::{Evaluation, Model, StatDef};
use crate::formula::Formula;
use crate::overlay::{resolve_id, ActionRule, CompanionRule, OverlaySet};
use crate::standard::{ABILITIES, SKILLS};
use crate::value::Value;

/// What a filter (`creatures`) knows of a creature: `creature.cr`,
/// `creature.<type>` (`creature.beast`), `creature.familiar`,
/// `creature.summon`, `creature.size` (`M`).
pub fn creature_vars(d: &Json) -> HashMap<String, Value> {
    let mut v = HashMap::new();
    let cr = creature::challenge(d).unwrap_or_default();
    let cr = match cr.split_once('/') {
        Some((n, d)) => n.parse().ok().zip(d.parse().ok()).and_then(|(n, d)| crate::value::Num::ratio(n, d)),
        None => cr.parse().ok().map(crate::value::Num::int),
    };
    v.insert("creature.cr".into(), Value::Num(cr.unwrap_or(crate::value::Num::int(0))));
    for t in [
        "aberration",
        "beast",
        "celestial",
        "construct",
        "dragon",
        "elemental",
        "fey",
        "fiend",
        "giant",
        "humanoid",
        "monstrosity",
        "ooze",
        "plant",
        "undead",
    ] {
        v.insert(format!("creature.{t}"), Value::Bool(false));
    }
    for t in creature::creature_type(d).to_lowercase().split(" or ") {
        v.insert(format!("creature.{}", t.split(" (").next().unwrap_or(t)), Value::Bool(true));
    }
    let flag = |k: &str| d.get(k).is_some_and(|x| x.as_bool() != Some(false));
    v.insert("creature.familiar".into(), Value::Bool(flag("familiar")));
    v.insert("creature.summon".into(), Value::Bool(flag("summonedBySpell") || flag("summonedByClass")));
    let size = d.get("size").and_then(Json::as_array).and_then(|s| s.first()).and_then(Json::as_str).unwrap_or("M");
    v.insert("creature.size".into(), Value::Str(size.into()));
    v
}

/// Whether a creature may be a rule's companion.
pub fn may_be(rule: &CompanionRule, lib: &Library, id: EntityId) -> bool {
    let e = lib.entity(id);
    if e.kind != Kind::Creature {
        return false;
    }
    match (&rule.creature, &rule.creatures) {
        (Some(c), _) => resolve_id(lib, c) == Some(id),
        (None, Some(f)) => matches!(f.eval(&creature_vars(&e.data)), Ok(Value::Bool(true))),
        (None, None) => false,
    }
}

/// The creatures a rule's companion may be, by name.
pub fn choices(rule: &CompanionRule, lib: &Library, filter: &SourceFilter) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = (0..lib.entities().len()).filter(|&id| filter.allows(lib.entity(id)) && may_be(rule, lib, id)).collect();
    out.sort_by(|a, b| lib.entity(*a).name.cmp(&lib.entity(*b).name));
    out
}

/// Whether a rule's companion depends on the level its spell is cast at:
/// its rules' formulas, or its stat block (`summonSpellLevel`).
pub fn uses_spell_level(rule: &CompanionRule, lib: &Library) -> bool {
    let formulas = rule
        .effects
        .iter()
        .flat_map(|e| [Some(&e.value), e.when.as_ref()])
        .chain(rule.actions.iter().flat_map(|a| [&a.attack, &a.dc, &a.damage, &a.heal, &a.temp_hp, &a.when].map(Option::as_ref)));
    let mut formulas = formulas.flatten();
    formulas.any(|f| f.vars().iter().any(|v| v == "spell_level"))
        || rule
            .creature
            .as_deref()
            .and_then(|c| resolve_id(lib, c))
            .is_some_and(|id| lib.entity(id).data.to_string().contains("summonSpellLevel"))
}

/// The rules of what gives a companion (its `from`).
pub fn rules_for<'a>(lib: &Library, overlays: &'a OverlaySet, c: &Companion) -> Vec<&'a CompanionRule> {
    let Some(from) = c.from.as_deref().and_then(|f| resolve_id(lib, f)) else { return Vec::new() };
    overlays.for_entity(&lib.entity(from).key).into_iter().flat_map(|o| o.companions.iter()).collect()
}

/// The owner's stats and inputs as formula variables, with its spell numbers
/// (`spell.attack`, `spell.dc`, `spell.mod`): its best casting class's.
/// A rule's `count` reads these; a companion's formulas, as `owner.<stat>`.
pub fn owner_vars(owner: &Evaluation, inputs: &HashMap<String, Value>) -> HashMap<String, Value> {
    // Its inputs too (`class.ranger.level`), under its stats.
    let mut vars = inputs.clone();
    vars.extend(owner.stats.iter().filter_map(|(k, r)| Some((k.clone(), r.value.clone().ok()?))));
    for part in ["attack", "dc", "mod"] {
        let best = owner
            .stats
            .keys()
            .filter(|k| k.starts_with("spell.") && k.ends_with(&format!(".{part}")) && k.matches('.').count() == 2)
            .filter_map(|k| owner.int(k))
            .max();
        vars.entry(format!("spell.{part}")).or_insert(Value::int(best.unwrap_or(0)));
    }
    vars
}

/// A companion's sheet.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub creature: EntityId,
    /// The creature's name.
    pub name: String,
    pub model: Model,
    /// Its actions with numbers: the stat block's attacks and saves, then
    /// the rules', with where each comes from.
    pub actions: Vec<(ActionRule, String)>,
    pub warnings: Vec<String>,
}

impl Sheet {
    pub fn evaluate(&self) -> Evaluation {
        self.model.evaluate()
    }
}

/// Build a companion's sheet; `None` when its creature is not in the library.
/// `owner`: the owner's [`owner_vars`].
pub fn build(c: &Companion, lib: &Library, overlays: &OverlaySet, owner: &HashMap<String, Value>) -> Option<Sheet> {
    let id = resolve_id(lib, &c.creature).filter(|&id| lib.entity(id).kind == Kind::Creature)?;
    let d = &lib.entity(id).data;
    let mut model = Model::default();
    let mut warnings = Vec::new();
    let input = |m: &mut Model, k: &str, v: i64| {
        m.inputs.insert(k.to_string(), Value::int(v));
    };
    let stat = |m: &mut Model, key: &str, base: &str, origin: &str| m.stats.push(StatDef::new(key, base, origin));

    stat(&mut model, "d20.bonus", "0", "d20 tests");
    input(&mut model, "prof.base", creature::proficiency(d));
    stat(&mut model, "prof", "prof.base", "challenge rating");
    let saves = creature::bonuses(d, "save");
    for (ab, name) in ABILITIES {
        let score = creature::score(d, ab);
        input(&mut model, &format!("base.{ab}"), score);
        stat(&mut model, ab, &format!("base.{ab}"), "stat block");
        stat(&mut model, &format!("mod.{ab}"), &format!("mod({ab})"), name);
        // What the stat block adds to the modifier (its proficiency).
        let extra = saves.iter().find(|(k, _)| k == ab).map_or(0, |(_, b)| b - creature::modifier(score));
        input(&mut model, &format!("save.extra.{ab}"), extra);
        stat(&mut model, &format!("save.{ab}"), &format!("mod.{ab} + save.extra.{ab} + d20.bonus"), &format!("{name} + stat block"));
    }
    let skills = creature::bonuses(d, "skill");
    for (key, _, ab) in SKILLS {
        let listed = skills.iter().find(|(k, _)| k.replace(' ', "_") == key).map(|(_, b)| *b);
        let extra = listed.map_or(0, |b| b - creature::modifier(creature::score(d, ab)));
        input(&mut model, &format!("skill.extra.{key}"), extra);
        stat(&mut model, &format!("skill.{key}"), &format!("mod.{ab} + skill.extra.{key} + d20.bonus"), "ability + stat block");
    }
    stat(&mut model, "passive.perception", "10 + skill.perception - d20.bonus", "10 + skill");
    let init = d.get("initiative").and_then(|i| i.get("proficiency")).and_then(Json::as_i64).unwrap_or(0);
    input(&mut model, "initiative.prof", init);
    stat(&mut model, "initiative", "mod.dex + prof * initiative.prof + d20.bonus", "Dexterity");
    let (ac, _) = creature::armor_class(d);
    input(&mut model, "ac.base", ac.unwrap_or(10));
    stat(&mut model, "ac", "ac.base", "stat block");
    let (hp, _, _) = creature::hit_points(d);
    input(&mut model, "hp.base", hp.unwrap_or(1));
    stat(&mut model, "hp.max", "max(1, hp.base)", "stat block");
    let (speeds, _) = creature::speeds(d);
    let walk = speeds.iter().find(|(m, _)| m == "walk").map_or(0, |(_, n)| *n);
    input(&mut model, "speed.base", walk);
    stat(&mut model, "speed", "speed.base", "stat block");
    for (mode, n) in speeds.iter().filter(|(m, _)| m != "walk") {
        input(&mut model, &format!("speed.base.{mode}"), *n);
        stat(&mut model, &format!("speed.{mode}"), &format!("speed.base.{mode}"), "stat block");
    }
    input(&mut model, "spell_level", c.spell_level.unwrap_or(0));
    for (k, v) in owner {
        model.inputs.insert(format!("owner.{k}"), v.clone());
    }
    // What it is (`creature.fey`), its chosen type over its own.
    let mut vars = creature_vars(d);
    if let Some(t) = &c.creature_type {
        for (k, v) in vars.iter_mut() {
            if matches!(v, Value::Bool(_)) && !matches!(k.as_str(), "creature.familiar" | "creature.summon") {
                *v = Value::Bool(k == &format!("creature.{}", t.to_lowercase()));
            }
        }
    }
    model.inputs.extend(vars);

    // The stat block's actions, then the rules'.
    let mut actions: Vec<(ActionRule, String)> = Vec::new();
    for (key, label) in [("action", "Action"), ("bonus", "Bonus Action"), ("reaction", "Reaction"), ("legendary", "Legendary Action")] {
        for a in d.get(key).and_then(Json::as_array).into_iter().flatten() {
            if let Some(rule) = stat_block_action(a, c.spell_level) {
                actions.push((rule, label.to_string()));
            }
        }
    }
    for rule in rules_for(lib, overlays, c) {
        for e in &rule.effects {
            model.effects.push(e.clone());
        }
        // A rule's action replaces the stat block's of that name.
        actions.retain(|(a, _)| !rule.actions.iter().any(|r| r.name == a.name));
        actions.extend(rule.actions.iter().map(|a| (a.clone(), rule.kind.clone())));
    }
    for (a, origin) in &actions {
        let key = |part: &str| format!("action.{}.{part}", slug(&a.name));
        for (part, f) in [("attack", &a.attack), ("dc", &a.dc), ("damage", &a.damage), ("heal", &a.heal), ("on", &a.when)] {
            if let Some(f) = f {
                model.stats.push(StatDef { key: key(part), base: f.clone(), origin: origin.clone() });
            }
        }
    }
    for (k, src) in &c.overrides {
        match Formula::parse(src).and_then(|f| f.eval(&model.inputs)) {
            Ok(v) => {
                model.overrides.insert(k.clone(), v);
            }
            Err(e) => warnings.push(format!("override `{k}`: {}", e.message)),
        }
    }
    Some(Sheet { creature: id, name: lib.entity(id).name.clone(), model, actions, warnings })
}

/// A stat block action with an attack roll, a save or damage, as an
/// action rule; `None` for one with none (Multiattack).
pub fn stat_block_action(a: &Json, spell_level: Option<i64>) -> Option<ActionRule> {
    let name = a.get("name").and_then(Json::as_str)?;
    let text = flatten(a.get("entries")?);
    let tags = inline_tags(&text);
    let tag = |t: &str| tags.iter().find(|(n, _)| n == t).map(|(_, v)| v.as_str());
    let level = spell_level.unwrap_or(0).to_string();
    let attack = if let Some(h) = tag("hit") {
        Formula::parse(h.trim_start_matches('+')).ok()
    } else if let Some(h) = tag("hitYourSpellAttack") {
        // `{@hitYourSpellAttack}`, `{@hitYourSpellAttack +2}`, or with the
        // text to show instead (`Bonus equals your spell attack modifier`).
        let extra: Option<i64> = h.trim().trim_start_matches('+').parse().ok();
        Formula::parse(&match extra {
            Some(n) if n != 0 => format!("owner.spell.attack + {n}"),
            _ => "owner.spell.attack".to_string(),
        })
        .ok()
    } else {
        None
    };
    let dc = tag("dc").and_then(|v| v.split('|').next()).map(|v| {
        if v.contains("summonSpellLevel") {
            v.replace("summonSpellLevel", &level)
        } else {
            v.to_string()
        }
    });
    let dc = dc.and_then(|v| Formula::parse(&v).ok());
    let save = tag("actSave").map(|s| s.to_lowercase()).filter(|s| ABILITIES.iter().any(|(k, _)| k == s)).or_else(|| {
        dc.as_ref().and_then(|_| {
            ABILITIES
                .iter()
                .find(|(_, n)| text.contains(&format!("{n} Saving Throw")) || text.contains(&format!("{n} saving throw")))
                .map(|(k, _)| k.to_string())
        })
    });
    let mut damage = Vec::new();
    let mut damage_type = None;
    for (i, (n, v)) in tags.iter().enumerate() {
        if n != "damage" {
            continue;
        }
        if let Some(f) = dice_formula(&v.replace("summonSpellLevel", "spell_level")) {
            damage.push(f);
        }
        if damage_type.is_none() {
            damage_type = damage_type_in(after_tag(&text, i));
        }
    }
    // Flat damage: `{@h}1 Slashing damage`.
    if damage.is_empty() {
        if let Some(i) = text.find("{@h}") {
            let after = &text[i + 4..];
            let n: String = after.chars().take_while(char::is_ascii_digit).collect();
            let mut words = after[n.len()..].split_whitespace();
            if let (false, Some(kind), Some("damage" | "damage.")) = (n.is_empty(), words.next(), words.next()) {
                damage.push(n);
                damage_type = Some(kind.to_lowercase());
            }
        }
    }
    let damage = match damage.len() {
        0 => None,
        _ => Formula::parse(&damage.join(" + ")).ok(),
    };
    if attack.is_none() && dc.is_none() && damage.is_none() {
        return None;
    }
    Some(ActionRule {
        name: name.to_string(),
        attack,
        save: save.filter(|_| dc.is_some()),
        dc,
        damage,
        damage_type,
        heal: None,
        temp_hp: None,
        resource: None,
        cost: 1,
        note: None,
        when: None,
    })
}

const DAMAGE_TYPES: [&str; 13] = [
    "acid",
    "bludgeoning",
    "cold",
    "fire",
    "force",
    "lightning",
    "necrotic",
    "piercing",
    "poison",
    "psychic",
    "radiant",
    "slashing",
    "thunder",
];

/// The damage type a few words on (`Piercing damage`, `plus the spell's
/// level Piercing damage`).
fn damage_type_in(text: &str) -> Option<String> {
    text.split_whitespace()
        .take(8)
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .find(|w| DAMAGE_TYPES.contains(&w.as_str()))
}

/// An entries tree as one string.
fn flatten(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Array(a) => a.iter().map(flatten).collect::<Vec<_>>().join(" "),
        Json::Object(o) => ["entries", "items", "entry"].iter().filter_map(|k| o.get(*k)).map(flatten).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

/// `{@name value}` tags in order, with where each ends.
fn inline_tags(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{@") {
        let after = &rest[i + 2..];
        let Some(end) = after.find('}') else { break };
        let body = &after[..end];
        let (name, value) = body.split_once(' ').unwrap_or((body, ""));
        out.push((name.to_string(), value.to_string()));
        rest = &after[end + 1..];
    }
    out
}

/// The text after the `n`th tag.
fn after_tag(text: &str, n: usize) -> &str {
    let mut rest = text;
    for _ in 0..=n {
        let Some(i) = rest.find("{@") else { return "" };
        let Some(end) = rest[i..].find('}') else { return "" };
        rest = &rest[i + end + 1..];
    }
    // `7 ({@damage 1d10 + 2}) Piercing`: past the closing parenthesis.
    rest.trim_start_matches(')').trim_start()
}

/// `2d6 + 3` → `dice(2, 6) + 3`.
pub fn dice_formula(s: &str) -> Option<String> {
    let mut out = String::new();
    for word in s.split_whitespace() {
        match word.split_once('d') {
            Some((n, f)) if f.chars().all(|c| c.is_ascii_digit()) && !f.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => {
                out.push_str(&format!("dice({}, {f})", if n.is_empty() { "1" } else { n }));
            }
            _ => out.push_str(word),
        }
        out.push(' ');
    }
    let out = out.trim().to_string();
    Formula::parse(&out).ok().map(|_| out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::Companion;
    use crate::overlay::Overlay;
    use serde_json::json;

    fn library() -> Library {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "monster": [
                {"name": "Owl", "source": "T", "size": ["T"], "type": "beast", "familiar": true, "cr": "0",
                 "ac": [11], "hp": {"average": 1, "formula": "1d4 - 1"}, "speed": {"walk": 5, "fly": 60},
                 "str": 3, "dex": 13, "con": 8, "int": 2, "wis": 12, "cha": 7, "skill": {"perception": "+5", "stealth": "+5"},
                 "action": [{"name": "Talons", "entries": ["{@atkr m} {@hit 3}, reach 5 ft. {@h}1 Slashing damage."]}]},
                {"name": "Wolf", "source": "T", "size": ["M"], "type": "beast", "cr": "1/4", "ac": [12], "hp": {"average": 11},
                 "speed": {"walk": 40}, "str": 14, "dex": 15, "con": 12, "int": 3, "wis": 12, "cha": 6, "save": {"dex": "+4"},
                 "action": [{"name": "Bite", "entries": ["{@atkr m} {@hit 4}, reach 5 ft. {@h}7 ({@damage 2d4 + 2}) Piercing damage."]}]},
                {"name": "Spirit", "source": "T", "size": ["M"], "type": "fey", "cr": "0", "ac": [{"special": "11 + the level of the spell"}],
                 "hp": {"special": "30 + 10 for each spell level above 3"}, "speed": {"walk": 30},
                 "str": 12, "dex": 12, "con": 12, "int": 10, "wis": 10, "cha": 10,
                 "action": [{"name": "Strike", "entries": ["{@atkr m} {@hitYourSpellAttack Bonus equals your spell attack modifier}, reach 5 ft. {@h}{@damage 1d8 + 3 + summonSpellLevel} Force damage."]},
                            {"name": "Glare", "entries": ["{@actSave wis} {@dc 12}. {@actSaveFail} {@damage 2d6} Psychic damage."]}]}
            ],
            "spell": [{"name": "Call Spirit", "source": "T", "level": 3}, {"name": "Find Pet", "source": "T", "level": 1}]
        }));
        lib
    }

    fn rules() -> OverlaySet {
        let (o, d) = Overlay::parse(
            r#"
[[feature]]
id = "spell:Find Pet|T"
[[feature.companion]]
kind = "Familiar"
creatures = "creature.familiar or (creature.beast and creature.cr == 0)"
types = ["celestial", "fey", "fiend"]

[[feature]]
id = "spell:Call Spirit|T"
[[feature.companion]]
creature = "creature:Spirit|T"
[[feature.companion.effect]]
target = "ac"
set = "11 + spell_level"
[[feature.companion.effect]]
target = "hp.max"
set = "30 + 10 * (spell_level - 3)"
[[feature.companion.action]]
name = "Bond"
heal = "owner.prof"
"#,
        )
        .unwrap();
        assert!(d.is_empty(), "{d:?}");
        let mut set = OverlaySet::default();
        set.add(o);
        set
    }

    fn owner() -> HashMap<String, Value> {
        let mut m = Model::default();
        m.stats.push(StatDef::new("prof", "3", "level"));
        m.stats.push(StatDef::new("spell.attack", "7", "ability + proficiency"));
        owner_vars(&m.evaluate(), &HashMap::new())
    }

    #[test]
    fn a_stat_block_becomes_a_sheet() {
        let lib = library();
        let wolf = Companion { creature: "creature:Wolf|T".into(), ..Default::default() };
        let s = build(&wolf, &lib, &OverlaySet::default(), &owner()).unwrap();
        let ev = s.evaluate();
        assert_eq!((ev.int("ac"), ev.int("hp.max"), ev.int("speed")), (Some(12), Some(11), Some(40)));
        assert_eq!((ev.int("mod.dex"), ev.int("save.dex"), ev.int("save.str")), (Some(2), Some(4), Some(2)));
        assert_eq!((ev.int("skill.perception"), ev.int("passive.perception")), (Some(1), Some(11)));
        assert_eq!(ev.int("action.bite.attack"), Some(4));
        assert_eq!(ev.get("action.bite.damage").unwrap().to_string(), "2d4 + 2");
        assert_eq!(s.actions[0].0.damage_type.as_deref(), Some("piercing"));

        let owl = Companion { creature: "creature:Owl|T".into(), ..Default::default() };
        let s = build(&owl, &lib, &OverlaySet::default(), &owner()).unwrap();
        let ev = s.evaluate();
        assert_eq!((ev.int("action.talons.attack"), ev.int("action.talons.damage")), (Some(3), Some(1)), "flat damage");
        assert_eq!(s.actions[0].0.damage_type.as_deref(), Some("slashing"));
    }

    #[test]
    fn a_rule_picks_the_creatures_and_changes_the_sheet() {
        let lib = library();
        let rules = rules();
        let pet = &rules.for_entity(&dnt_data::Key::new(Kind::Spell, &["Find Pet", "T"]))[0].companions[0];
        let names: Vec<&str> = choices(pet, &lib, &SourceFilter::default()).iter().map(|&id| lib.entity(id).name.as_str()).collect();
        assert_eq!(names, ["Owl"], "the wolf is CR 1/4, the spirit no beast");
        assert_eq!(pet.types, ["celestial", "fey", "fiend"]);

        let spirit = Companion {
            creature: "creature:Spirit|T".into(),
            from: Some("spell:Call Spirit|T".into()),
            spell_level: Some(4),
            ..Default::default()
        };
        let s = build(&spirit, &lib, &rules, &owner()).unwrap();
        let ev = s.evaluate();
        assert_eq!((ev.int("ac"), ev.int("hp.max")), (Some(15), Some(40)));
        assert_eq!(ev.int("action.strike.attack"), Some(7), "the owner's spell attack");
        assert_eq!(ev.get("action.strike.damage").unwrap().to_string(), "1d8 + 7", "+ the spell's level");
        assert_eq!((ev.int("action.glare.dc"), s.actions[1].0.save.as_deref()), (Some(12), Some("wis")));
        assert_eq!(ev.int("action.bond.heal"), Some(3), "a rule's action knows the owner");
    }

    #[test]
    fn the_srd_summons_by_its_built_in_rules() {
        let lib = Library::srd();
        let rules = crate::builtin::overlays().0;
        let familiar = &rules.for_entity(&dnt_data::Key::new(Kind::Spell, &["Find Familiar", "SRD52"]))[0].companions[0];
        let names: Vec<String> = choices(familiar, &lib, &SourceFilter::default()).iter().map(|&id| lib.entity(id).name.clone()).collect();
        for want in ["Bat", "Cat", "Owl", "Raven", "Spider"] {
            assert!(names.iter().any(|n| n == want), "{want}: {names:?}");
        }
        assert!(!names.iter().any(|n| n == "Wolf"), "CR 1/4");

        let steed = Companion {
            creature: "creature:Otherworldly Steed|SRD52".into(),
            from: Some("spell:Find Steed|SRD52".into()),
            spell_level: Some(4),
            creature_type: Some("fey".into()),
            ..Default::default()
        };
        let s = build(&steed, &lib, &rules, &owner()).unwrap();
        let ev = s.evaluate();
        assert_eq!((ev.int("ac"), ev.int("hp.max"), ev.int("speed.fly"), ev.int("prof")), (Some(14), Some(45), Some(60), Some(3)));
        let shown: Vec<&str> = s
            .actions
            .iter()
            .filter(|(a, _)| a.when.is_none() || ev.get(&format!("action.{}.on", slug(&a.name))) == Some(&Value::Bool(true)))
            .map(|(a, _)| a.name.as_str())
            .collect();
        assert_eq!(shown, ["Otherworldly Slam (Fey)"], "the fey's slam only");
        assert_eq!(ev.int("action.otherworldly_slam_fey.attack"), Some(7));
        assert_eq!(ev.get("action.otherworldly_slam_fey.damage").unwrap().to_string(), "1d8 + 4");
    }
}
