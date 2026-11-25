//! Conditions: the library's, and what their rules (and any feature's)
//! do to the character's rolls. Their effects on the numbers are the
//! rules' effects, applied by the builder while the character has them.

use std::collections::HashMap;

use dnt_data::{EntityId, Kind, Library};

use crate::overlay::{OverlaySet, RollMode, RollRule};
use crate::roll::Advantage;
use crate::standard::ABILITIES;
use crate::value::Value;

/// A condition the character can have, as its rules say.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub name: String,
    pub entity: EntityId,
    /// Its book.
    pub source: String,
    /// What it does, in a line.
    pub summary: String,
    /// Had at a level (Exhaustion): what each level does, in a line.
    pub levels: Vec<String>,
}

impl Condition {
    /// What the condition does at a level (0 for one without levels).
    pub fn at(&self, level: i64) -> &str {
        match level {
            l if l < 1 => &self.summary,
            l => self.levels.get(l as usize - 1).or(self.levels.last()).map_or("", String::as_str),
        }
    }
}

/// Every condition in the library, one per name: of those with a name,
/// the one from the book of `class` (the starting class), else of its
/// edition, else the first.
pub fn available(lib: &Library, rules: &OverlaySet, class: Option<EntityId>) -> Vec<Condition> {
    let mut by_name: Vec<(String, Vec<EntityId>)> = Vec::new();
    for (id, e) in lib.entities().iter().enumerate().filter(|(_, e)| e.kind == Kind::Condition) {
        match by_name.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&e.name)) {
            Some((_, ids)) => ids.push(id),
            None => by_name.push((e.name.clone(), vec![id])),
        }
    }
    let rank = |id: EntityId| match class {
        Some(c) if lib.entity(c).source == lib.entity(id).source => 0,
        Some(c) if crate::builder::is_2024(lib, c) == crate::builder::is_2024(lib, id) => 1,
        _ => 2,
    };
    let mut out: Vec<Condition> = by_name
        .into_iter()
        .map(|(name, ids)| {
            let id = *ids.iter().min_by_key(|id| rank(**id)).expect("a name has an entity");
            let e = lib.entity(id);
            let own = rules.for_entity(&e.key);
            Condition {
                name,
                entity: id,
                source: e.source.clone(),
                summary: own.iter().find_map(|o| o.summary.clone()).unwrap_or_default(),
                levels: own.iter().find(|o| !o.levels.is_empty()).map(|o| o.levels.clone()).unwrap_or_default(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The condition had at a level (Exhaustion), if the rules have one.
pub fn leveled(conditions: &[Condition]) -> Option<&Condition> {
    conditions.iter().find(|c| !c.levels.is_empty())
}

/// What is being rolled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollKind<'a> {
    Attack,
    /// An ability check (skills included), by ability key.
    Check(&'a str),
    Save(&'a str),
    /// A Dexterity check.
    Initiative,
}

/// What a roll rule's filter knows of a roll: `roll.attack`, `roll.check`,
/// `roll.save`, `roll.initiative`, `roll.str` … `roll.cha`, and the
/// exhaustion level.
fn roll_vars(kind: RollKind, exhaustion: i64) -> HashMap<String, Value> {
    let (what, ability) = match kind {
        RollKind::Attack => ("attack", None),
        RollKind::Check(ab) => ("check", Some(ab)),
        RollKind::Save(ab) => ("save", Some(ab)),
        RollKind::Initiative => ("initiative", Some("dex")),
    };
    let mut vars = HashMap::new();
    for w in ["attack", "check", "save", "initiative"] {
        let on = w == what || (w == "check" && what == "initiative");
        vars.insert(format!("roll.{w}"), Value::Bool(on));
    }
    for (ab, _) in ABILITIES {
        vars.insert(format!("roll.{ab}"), Value::Bool(ability == Some(ab)));
    }
    vars.insert("exhaustion".into(), Value::int(exhaustion));
    vars
}

fn picked<'r>(rules: &'r [RollRule], kind: RollKind, exhaustion: i64) -> impl Iterator<Item = &'r RollRule> {
    let vars = roll_vars(kind, exhaustion);
    rules.iter().filter(move |r| matches!(r.rolls.eval(&vars), Ok(Value::Bool(true))))
}

/// Advantage and disadvantage the rules give a roll, with the reason for
/// each.
pub fn advantage(rules: &[RollRule], kind: RollKind, exhaustion: i64) -> Vec<(Advantage, String)> {
    picked(rules, kind, exhaustion)
        .filter_map(|r| match r.mode {
            RollMode::Advantage => Some((Advantage::Advantage, r.origin.clone())),
            RollMode::Disadvantage => Some((Advantage::Disadvantage, r.origin.clone())),
            RollMode::Fail => None,
        })
        .collect()
}

/// A roll the rules make fail without rolling, and why.
pub fn auto_fail(rules: &[RollRule], kind: RollKind, exhaustion: i64) -> Option<String> {
    picked(rules, kind, exhaustion).find(|r| r.mode == RollMode::Fail).map(|r| match &r.note {
        Some(note) => format!("{}: {note}", r.origin),
        None => format!("{}: fails", r.origin),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::Overlay;

    const RULES: &str = r#"
[[feature]]
id = "condition:Tangled|T"
summary = "Speed 0; Disadvantage on attacks"
[[feature.effect]]
target = "speed"
min = "0"
[[feature.roll]]
rolls = "roll.attack or (roll.save and roll.dex)"
mode = "disadvantage"

[[feature]]
id = "condition:Hidden|T"
[[feature.roll]]
rolls = "roll.attack or roll.initiative"
mode = "advantage"

[[feature]]
id = "condition:Frozen|T"
[[feature.roll]]
rolls = "roll.save and (roll.str or roll.dex)"
mode = "fail"
note = "Strength and Dexterity saves fail"

[[feature]]
id = "condition:Weary|T"
levels = ["slow", "slower", "gone"]
[[feature.effect]]
target = "d20.bonus"
add = "-2 * exhaustion"
[[feature.roll]]
rolls = "exhaustion >= 2 and roll.check"
mode = "disadvantage"
"#;

    fn setup() -> (Library, OverlaySet) {
        let mut lib = Library::new();
        lib.load_json(&serde_json::json!({"condition": [
            {"name": "Tangled", "source": "T"}, {"name": "Hidden", "source": "T"},
            {"name": "Frozen", "source": "T"}, {"name": "Weary", "source": "T"},
            {"name": "Tangled", "source": "U"}
        ]}));
        let mut rules = OverlaySet::default();
        let (o, d) = Overlay::parse(RULES).unwrap();
        assert!(d.is_empty(), "{d:?}");
        rules.add(o);
        (lib, rules)
    }

    fn rolls_of(lib: &Library, rules: &OverlaySet, names: &[&str]) -> Vec<RollRule> {
        let all = available(lib, rules, None);
        all.iter()
            .filter(|c| names.contains(&c.name.as_str()))
            .flat_map(|c| rules.for_entity(&lib.entity(c.entity).key))
            .flat_map(|o| o.rolls.clone())
            .collect()
    }

    #[test]
    fn conditions_come_from_the_library_with_their_rules() {
        let (lib, rules) = setup();
        let all = available(&lib, &rules, None);
        assert_eq!(all.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Frozen", "Hidden", "Tangled", "Weary"], "one per name");
        let tangled = all.iter().find(|c| c.name == "Tangled").unwrap();
        assert_eq!(tangled.summary, "Speed 0; Disadvantage on attacks");
        let weary = leveled(&all).unwrap();
        assert_eq!(weary.name, "Weary");
        assert_eq!(weary.at(2), "slower");
        assert_eq!(weary.at(9), "gone", "past the last level: the last");
    }

    #[test]
    fn rules_change_rolls() {
        let (lib, rules) = setup();
        let r = rolls_of(&lib, &rules, &["Tangled", "Hidden"]);
        let modes = |k| advantage(&r, k, 0).into_iter().map(|(a, _)| a).collect::<Vec<_>>();
        assert_eq!(Advantage::combine(&modes(RollKind::Attack)), Advantage::Normal, "they cancel");
        assert_eq!(modes(RollKind::Save("dex")), [Advantage::Disadvantage]);
        assert!(modes(RollKind::Save("con")).is_empty());
        assert_eq!(modes(RollKind::Initiative), [Advantage::Advantage]);

        let frozen = rolls_of(&lib, &rules, &["Frozen"]);
        assert_eq!(auto_fail(&frozen, RollKind::Save("str"), 0).as_deref(), Some("Frozen: Strength and Dexterity saves fail"));
        assert!(auto_fail(&frozen, RollKind::Save("wis"), 0).is_none());

        let weary = rolls_of(&lib, &rules, &["Weary"]);
        assert!(advantage(&weary, RollKind::Check("wis"), 1).is_empty());
        assert_eq!(advantage(&weary, RollKind::Initiative, 2).len(), 1, "initiative is a check");
    }

    #[test]
    fn built_in_rules_on_the_sheet() {
        use crate::builder::build;
        use crate::character::Character;
        let rules = crate::builtin::overlays().0;
        let lib = Library::srd();
        let mut ch = Character::default();
        ch.state.exhaustion = 2;
        let b = build(&ch, &lib, &rules);
        let ev = b.evaluate();
        assert_eq!(ev.int("speed"), Some(20), "2024: -5 ft. per level");
        assert_eq!(ev.int("save.str"), Some(-4));
        assert_eq!(ev.int("skill.stealth"), Some(-4));
        assert_eq!(ev.int("initiative"), Some(-4));
        assert_eq!(ev.int("passive.perception"), Some(10), "passive scores roll nothing");
        assert_eq!(ev.int("attack.unarmed.bonus"), Some(ev.int("prof").unwrap() - 4), "proficiency, -4");
        assert_eq!(leveled(&b.conditions).unwrap().at(2), "−4 to d20 tests, −10 ft. speed");
        assert_eq!(b.conditions.len(), 15, "the SRD's");

        ch.state.exhaustion = 0;
        ch.state.conditions = vec!["Grappled".into(), "Stunned".into()];
        let b = build(&ch, &lib, &rules);
        assert_eq!(b.evaluate().int("speed"), Some(0));
        assert!(auto_fail(&b.roll_rules, RollKind::Save("dex"), 0).is_some());
        assert_eq!(advantage(&b.roll_rules, RollKind::Initiative, 0).len(), 1, "2024: Incapacitated");

        // 2014: a book's conditions have the 2014 rules.
        let mut lib = Library::new();
        lib.load_json(&serde_json::json!({"condition": [{"name": "Exhaustion", "source": "PHB"}, {"name": "Stunned", "source": "PHB"}]}));
        let mut ch = Character::default();
        ch.state.exhaustion = 3;
        ch.state.conditions = vec!["Stunned".into()];
        let b = build(&ch, &lib, &rules);
        assert_eq!(b.evaluate().int("speed"), Some(15), "30 halved");
        let e = b.evaluate().int("exhaustion").unwrap();
        assert_eq!(advantage(&b.roll_rules, RollKind::Save("con"), e).len(), 1, "level 3: saves");
        assert!(advantage(&b.roll_rules, RollKind::Initiative, e).iter().all(|(_, w)| w == "Exhaustion"), "2014: no Incapacitated");
    }
}
