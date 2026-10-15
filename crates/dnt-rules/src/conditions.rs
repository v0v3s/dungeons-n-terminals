//! Conditions and exhaustion: what they do to the character's numbers and
//! to their rolls.

use crate::engine::{Effect, Op};
use crate::formula::Formula;
use crate::roll::Advantage;
use crate::standard::{ABILITIES, SKILLS};

/// The conditions a character can have (Exhaustion is a level instead).
pub const CONDITIONS: [&str; 14] = [
    "Blinded",
    "Charmed",
    "Deafened",
    "Frightened",
    "Grappled",
    "Incapacitated",
    "Invisible",
    "Paralyzed",
    "Petrified",
    "Poisoned",
    "Prone",
    "Restrained",
    "Stunned",
    "Unconscious",
];

/// What a condition means for the character, in a line.
pub fn summary(condition: &str) -> &'static str {
    match condition.to_lowercase().as_str() {
        "blinded" => "Disadvantage on attacks; attacks against you have Advantage; fail sight checks",
        "charmed" => "Can't attack the charmer; it has Advantage on social checks against you",
        "deafened" => "Fail checks that need hearing",
        "frightened" => "Disadvantage on checks and attacks while you see the source; can't move closer",
        "grappled" => "Speed 0; Disadvantage on attacks against anyone but the grappler",
        "incapacitated" => "No actions, Bonus Actions or Reactions; no Concentration; Disadvantage on Initiative",
        "invisible" => "Advantage on attacks and Initiative; attacks against you have Disadvantage",
        "paralyzed" => "Incapacitated, speed 0, fail Str and Dex saves; hits within 5 ft. are critical",
        "petrified" => "Incapacitated, speed 0, fail Str and Dex saves; resistance to all damage",
        "poisoned" => "Disadvantage on attack rolls and ability checks",
        "prone" => "Disadvantage on attacks; attacks within 5 ft. have Advantage against you",
        "restrained" => "Speed 0; Disadvantage on attacks and Dex saves",
        "stunned" => "Incapacitated, fail Str and Dex saves; attacks against you have Advantage",
        "unconscious" => "Incapacitated, Prone, speed 0, fail Str and Dex saves; hits within 5 ft. are critical",
        _ => "",
    }
}

/// What is being rolled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollKind<'a> {
    Attack,
    /// An ability check (skills included), by ability key.
    Check(&'a str),
    Save(&'a str),
    Initiative,
}

fn has(conditions: &[String], name: &str) -> bool {
    conditions.iter().any(|c| c.eq_ignore_ascii_case(name))
}

/// Advantage and disadvantage the character's conditions give a roll, with
/// the reason for each.
pub fn advantage(conditions: &[String], exhaustion: i64, rules_2024: bool, kind: RollKind) -> Vec<(Advantage, String)> {
    let mut out = Vec::new();
    let mut add = |a: Advantage, why: &str| out.push((a, why.to_string()));
    let incapacitated = ["Incapacitated", "Paralyzed", "Petrified", "Stunned", "Unconscious"].iter().any(|c| has(conditions, c));
    match kind {
        RollKind::Attack => {
            for c in ["Blinded", "Frightened", "Poisoned", "Prone", "Restrained"] {
                if has(conditions, c) {
                    add(Advantage::Disadvantage, c);
                }
            }
            if has(conditions, "Invisible") {
                add(Advantage::Advantage, "Invisible");
            }
        }
        RollKind::Check(_) | RollKind::Initiative => {
            for c in ["Frightened", "Poisoned"] {
                if has(conditions, c) {
                    add(Advantage::Disadvantage, c);
                }
            }
            if kind == RollKind::Initiative {
                if has(conditions, "Invisible") {
                    add(Advantage::Advantage, "Invisible");
                }
                if incapacitated && rules_2024 {
                    add(Advantage::Disadvantage, "Incapacitated");
                }
            }
        }
        RollKind::Save(ab) => {
            if ab == "dex" && has(conditions, "Restrained") {
                add(Advantage::Disadvantage, "Restrained");
            }
        }
    }
    // 2014 exhaustion: disadvantage on checks from level 1, on attacks and
    // saves from level 3.
    if !rules_2024 && exhaustion > 0 {
        match kind {
            RollKind::Check(_) | RollKind::Initiative => add(Advantage::Disadvantage, "Exhaustion"),
            RollKind::Attack | RollKind::Save(_) if exhaustion >= 3 => add(Advantage::Disadvantage, "Exhaustion"),
            _ => {}
        }
    }
    out
}

/// What the exhaustion level means, in a line.
pub fn exhaustion_summary(level: i64, rules_2024: bool) -> String {
    match (level.clamp(0, 6), rules_2024) {
        (0, _) => String::new(),
        (6, _) => "death".into(),
        (l, true) => format!("−{} to d20 tests, −{} ft. speed", 2 * l, 5 * l),
        (l, false) => {
            let mut parts = vec!["disadvantage on ability checks"];
            if l >= 2 {
                parts.push("speed halved");
            }
            if l >= 3 {
                parts.push("disadvantage on attacks and saves");
            }
            if l >= 4 {
                parts.push("hit point maximum halved");
            }
            if l >= 5 {
                parts.push("speed 0");
            }
            parts.join(", ")
        }
    }
}

/// Rolls the character fails without rolling.
pub fn auto_fail(conditions: &[String], kind: RollKind) -> Option<String> {
    let RollKind::Save(ab @ ("str" | "dex")) = kind else { return None };
    ["Paralyzed", "Petrified", "Stunned", "Unconscious"]
        .iter()
        .find(|c| has(conditions, c))
        .map(|c| format!("{c}: Strength and Dexterity saves fail ({})", if ab == "str" { "Strength" } else { "Dexterity" }))
}

fn effect(target: &str, op: Op, value: &str, origin: &str) -> Effect {
    Effect {
        target: target.into(),
        op,
        value: Formula::parse(value).expect("built-in formula"),
        when: None,
        origin: origin.into(),
    }
}

/// Effects on the character's numbers: speed 0 and 2024 exhaustion
/// (−2 per level to d20 tests, −5 ft. speed per level).
pub fn effects(conditions: &[String], exhaustion: i64, rules_2024: bool) -> Vec<Effect> {
    let mut out = Vec::new();
    if let Some(c) = ["Grappled", "Restrained", "Paralyzed", "Petrified", "Unconscious"].iter().find(|c| has(conditions, c)) {
        out.push(effect("speed", Op::Min, "0", c));
    }
    let exhaustion = exhaustion.clamp(0, 6);
    if exhaustion > 0 && rules_2024 {
        let origin = format!("Exhaustion {exhaustion}");
        let penalty = format!("{}", -2 * exhaustion);
        for (ab, _) in ABILITIES {
            out.push(effect(&format!("save.{ab}"), Op::Add, &penalty, &origin));
        }
        for (k, _, _) in SKILLS {
            out.push(effect(&format!("skill.{k}"), Op::Add, &penalty, &origin));
        }
        for t in ["initiative", "attack.bonus", "d20.penalty"] {
            out.push(effect(t, Op::Add, &penalty, &origin));
        }
        out.push(effect("speed", Op::Add, &format!("{}", -5 * exhaustion), &origin));
    }
    // 2014: speed halved from level 2, hit point maximum halved from 4,
    // speed 0 from 5 (disadvantage comes with the rolls).
    if !rules_2024 {
        if exhaustion >= 2 {
            out.push(effect("speed", Op::Mul, "1 / 2", "Exhaustion 2+: speed halved"));
        }
        if exhaustion >= 4 {
            out.push(effect("hp.max", Op::Mul, "1 / 2", "Exhaustion 4+: hit point maximum halved"));
        }
        if exhaustion >= 5 {
            out.push(effect("speed", Op::Min, "0", "Exhaustion 5"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn roll_modes() {
        let p = c(&["poisoned", "Invisible"]);
        let a = advantage(&p, 0, true, RollKind::Attack);
        assert_eq!(Advantage::combine(&a.iter().map(|(x, _)| *x).collect::<Vec<_>>()), Advantage::Normal, "they cancel");
        assert_eq!(advantage(&p, 0, true, RollKind::Check("wis"))[0].0, Advantage::Disadvantage);
        assert!(advantage(&c(&["Restrained"]), 0, true, RollKind::Save("dex")).iter().any(|(a, _)| *a == Advantage::Disadvantage));
        assert!(advantage(&[], 3, false, RollKind::Save("con")).iter().any(|(_, w)| w == "Exhaustion"), "2014 level 3");
        assert!(advantage(&[], 3, true, RollKind::Save("con")).is_empty(), "2024 uses a penalty instead");
        assert!(auto_fail(&c(&["Stunned"]), RollKind::Save("dex")).is_some());
        assert!(auto_fail(&c(&["Stunned"]), RollKind::Save("wis")).is_none());
    }

    #[test]
    fn classic_exhaustion() {
        use crate::engine::Model;
        let mut m = Model { stats: crate::standard::stats(), inputs: crate::standard::default_inputs(), ..Model::default() };
        for (ab, _) in ABILITIES {
            m.inputs.insert(format!("base.{ab}"), crate::value::Value::int(10));
        }
        m.inputs.insert("level".into(), crate::value::Value::int(1));
        m.inputs.insert("hp.base".into(), crate::value::Value::int(11));
        m.effects = effects(&[], 4, false);
        let ev = m.evaluate();
        assert_eq!(ev.int("speed"), Some(15), "30 halved");
        assert_eq!(ev.int("hp.max"), Some(5), "11 halved, rounded down");
        assert!(exhaustion_summary(4, false).contains("hit point maximum halved"));
        assert_eq!(exhaustion_summary(6, true), "death");
    }

    #[test]
    fn numbers_on_the_sheet() {
        use crate::builder::build;
        use crate::character::Character;
        use crate::overlay::OverlaySet;
        let lib = dnt_data::Library::new();
        let mut ch = Character::default();
        ch.state.exhaustion = 2;
        let ev = build(&ch, &lib, &OverlaySet::default()).evaluate();
        assert_eq!(ev.int("speed"), Some(20), "2024: -5 ft. per level");
        assert_eq!(ev.int("save.str"), Some(-4));
        assert_eq!(ev.int("skill.stealth"), Some(-4));
        assert_eq!(ev.int("d20.penalty"), Some(-4));
        assert_eq!(ev.int("passive.perception"), Some(10), "passive scores roll nothing");
        assert_eq!(ev.int("attack.unarmed.bonus"), Some(ev.int("prof").unwrap() - 4), "proficiency, -4");
        ch.state.conditions = vec!["Grappled".into()];
        assert_eq!(build(&ch, &lib, &OverlaySet::default()).evaluate().int("speed"), Some(0));
    }
}