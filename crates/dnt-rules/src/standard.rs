//! The standard 5e stats every character has, and the inputs they read.
//!
//! Inputs (set by the character builder; see [`default_inputs`]):
//!
//! | Input                  | Meaning                                         |
//! |------------------------|-------------------------------------------------|
//! | `base.str` … `base.cha`| ability scores before bonuses                   |
//! | `level`                | total character level                           |
//! | `prof.save.<ability>`  | save proficiency multiplier (0 or 1)            |
//! | `prof.skill.<skill>`   | 0, 1/2 (half), 1 (proficient), 2 (expertise)    |
//! | `hp.base`              | hit points from hit dice, before Constitution   |
//! | `speed.base`           | walking speed                                   |
//! | `armor.none/light/medium/heavy` | worn armor category (booleans)         |
//! | `armor.base`           | the worn armor's base AC                        |
//! | `armor.shield`         | AC bonus from a shield (0 if none)              |

use std::collections::HashMap;

use crate::engine::StatDef;
use crate::value::{Num, Value};

pub const ABILITIES: [(&str, &str); 6] = [
    ("str", "Strength"),
    ("dex", "Dexterity"),
    ("con", "Constitution"),
    ("int", "Intelligence"),
    ("wis", "Wisdom"),
    ("cha", "Charisma"),
];

/// Skills and their abilities, keyed as in `skill.<key>`.
pub const SKILLS: [(&str, &str, &str); 18] = [
    ("acrobatics", "Acrobatics", "dex"),
    ("animal_handling", "Animal Handling", "wis"),
    ("arcana", "Arcana", "int"),
    ("athletics", "Athletics", "str"),
    ("deception", "Deception", "cha"),
    ("history", "History", "int"),
    ("insight", "Insight", "wis"),
    ("intimidation", "Intimidation", "cha"),
    ("investigation", "Investigation", "int"),
    ("medicine", "Medicine", "wis"),
    ("nature", "Nature", "int"),
    ("perception", "Perception", "wis"),
    ("performance", "Performance", "cha"),
    ("persuasion", "Persuasion", "cha"),
    ("religion", "Religion", "int"),
    ("sleight_of_hand", "Sleight of Hand", "dex"),
    ("stealth", "Stealth", "dex"),
    ("survival", "Survival", "wis"),
];

/// Definitions of the standard stats.
pub fn stats() -> Vec<StatDef> {
    let mut out = Vec::new();
    for (ab, name) in ABILITIES {
        out.push(StatDef::new(ab, &format!("base.{ab}"), "base score"));
        out.push(StatDef::new(&format!("mod.{ab}"), &format!("mod({ab})"), name));
        out.push(StatDef::new(
            &format!("save.{ab}"),
            &format!("mod.{ab} + prof * prof.save.{ab}"),
            &format!("{name} + proficiency"),
        ));
    }
    out.push(StatDef::new("prof", "2 + floor((level - 1) / 4)", "level"));
    for (key, _, ab) in SKILLS {
        let ability = ABILITIES.iter().find(|(a, _)| *a == ab).unwrap().1;
        out.push(StatDef::new(
            &format!("skill.{key}"),
            &format!("mod.{ab} + prof * prof.skill.{key}"),
            &format!("{ability} + proficiency"),
        ));
    }
    for skill in ["perception", "investigation", "insight"] {
        out.push(StatDef::new(&format!("passive.{skill}"), &format!("10 + skill.{skill}"), "10 + skill"));
    }
    out.push(StatDef::new("initiative", "mod.dex", "Dexterity"));
    out.push(StatDef::new(
        "ac",
        "if(armor.none, 10 + mod.dex, armor.base + if(armor.heavy, 0, if(armor.medium, min(mod.dex, 2), mod.dex))) + armor.shield",
        "armor",
    ));
    out.push(StatDef::new("hp.max", "max(1, hp.base + mod.con * level)", "hit dice + Constitution"));
    out.push(StatDef::new("speed", "speed.base", "base speed"));
    out
}

/// Spellcasting stats for a casting ability (`int`, `wis`, `cha`).
pub fn spellcasting(ability: &str) -> Vec<StatDef> {
    vec![
        StatDef::new("spell.mod", &format!("mod.{ability}"), "spellcasting ability"),
        StatDef::new("spell.attack", "spell.mod + prof", "ability + proficiency"),
        StatDef::new("spell.dc", "8 + spell.mod + prof", "8 + ability + proficiency"),
    ]
}

/// Defaults for every input the standard stats read: scores of 10, level 1,
/// no proficiencies, no armor.
pub fn default_inputs() -> HashMap<String, Value> {
    let mut m = HashMap::new();
    for (ab, _) in ABILITIES {
        m.insert(format!("base.{ab}"), Value::int(10));
        m.insert(format!("prof.save.{ab}"), Value::int(0));
    }
    for (key, _, _) in SKILLS {
        m.insert(format!("prof.skill.{key}"), Value::int(0));
    }
    m.insert("level".into(), Value::int(1));
    m.insert("hp.base".into(), Value::int(0));
    m.insert("speed.base".into(), Value::int(30));
    m.insert("armor.none".into(), Value::Bool(true));
    for cat in ["light", "medium", "heavy"] {
        m.insert(format!("armor.{cat}"), Value::Bool(false));
    }
    m.insert("armor.base".into(), Value::int(10));
    m.insert("armor.shield".into(), Value::int(0));
    m
}

/// Half proficiency (e.g. Jack of All Trades), as a skill multiplier.
pub fn half() -> Value {
    Value::Num(Num::ratio(1, 2).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{effect, Model, Op};

    fn rogue() -> Model {
        let mut m = Model {
            stats: stats(),
            inputs: default_inputs(),
            ..Model::default()
        };
        let set = |m: &mut Model, k: &str, v: Value| {
            m.inputs.insert(k.into(), v);
        };
        set(&mut m, "level", Value::int(5));
        set(&mut m, "base.dex", Value::int(16));
        set(&mut m, "base.con", Value::int(14));
        set(&mut m, "base.wis", Value::int(12));
        set(&mut m, "prof.save.dex", Value::int(1));
        set(&mut m, "prof.skill.stealth", Value::int(2));
        set(&mut m, "prof.skill.perception", Value::int(1));
        set(&mut m, "prof.skill.athletics", half());
        set(&mut m, "hp.base", Value::int(8 + 4 * 5));
        set(&mut m, "armor.none", Value::Bool(false));
        set(&mut m, "armor.light", Value::Bool(true));
        set(&mut m, "armor.base", Value::int(12));
        m
    }

    #[test]
    fn core_stats() {
        let m = rogue();
        let ev = m.evaluate();
        assert_eq!(ev.errors().count(), 0, "{:?}", ev.errors().collect::<Vec<_>>());
        assert_eq!(ev.int("prof"), Some(3));
        assert_eq!(ev.int("mod.dex"), Some(3));
        assert_eq!(ev.int("save.dex"), Some(6));
        assert_eq!(ev.int("save.str"), Some(0));
        assert_eq!(ev.int("skill.stealth"), Some(9));
        assert_eq!(ev.int("skill.athletics"), Some(1), "half of 3 rounds down");
        assert_eq!(ev.int("passive.perception"), Some(14));
        assert_eq!(ev.int("ac"), Some(15));
        assert_eq!(ev.int("hp.max"), Some(38));
        assert_eq!(ev.int("initiative"), Some(3));
    }

    #[test]
    fn effects_flow_through_dependents() {
        let mut m = rogue();
        m.effects.push(effect("dex", Op::Add, "2", None, "Ability Score Improvement"));
        m.stats.extend(spellcasting("wis"));
        let ev = m.evaluate();
        assert_eq!(ev.int("dex"), Some(18));
        assert_eq!(ev.int("skill.stealth"), Some(10));
        assert_eq!(ev.int("ac"), Some(16));
        assert_eq!(ev.int("spell.dc"), Some(12));
    }

    #[test]
    fn medium_armor_caps_dex() {
        let mut m = rogue();
        m.inputs.insert("armor.light".into(), Value::Bool(false));
        m.inputs.insert("armor.medium".into(), Value::Bool(true));
        m.inputs.insert("armor.base".into(), Value::int(14));
        m.inputs.insert("armor.shield".into(), Value::int(2));
        assert_eq!(m.evaluate().int("ac"), Some(18));
    }
}
