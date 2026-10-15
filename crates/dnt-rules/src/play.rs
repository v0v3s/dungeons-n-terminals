//! Changes to a character's play state: damage, healing, spending
//! resources and slots, and rests. Limits come from the evaluated stats.

use crate::builder::Build;
use crate::character::Character;
use crate::engine::Evaluation;
use crate::overlay::Recharge;

/// Rules edition, which changes rest recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edition {
    /// 2014 rules: a long rest restores half your hit dice.
    Classic,
    /// 2024 rules: a long rest restores all hit dice.
    Modern,
}

impl Edition {
    /// The edition of the starting class (data `edition: "one"` = 2024).
    pub fn of(build: &Build, lib: &dnt_data::Library) -> Edition {
        let modern = build
            .classes
            .first()
            .and_then(|c| lib.entity(c.entity).data.get("edition"))
            .and_then(|e| e.as_str())
            == Some("one");
        if modern {
            Edition::Modern
        } else {
            Edition::Classic
        }
    }
}

pub fn max_hp(ev: &Evaluation) -> i64 {
    ev.int("hp.max").unwrap_or(1).max(1)
}

pub fn current_hp(c: &Character, ev: &Evaluation) -> i64 {
    c.state.hp.unwrap_or(max_hp(ev)).min(max_hp(ev))
}

/// Take damage: temporary hit points absorb it first.
pub fn damage(c: &mut Character, ev: &Evaluation, amount: i64) {
    let amount = amount.max(0);
    let absorbed = amount.min(c.state.temp_hp);
    c.state.temp_hp -= absorbed;
    let hp = (current_hp(c, ev) - (amount - absorbed)).max(0);
    c.state.hp = Some(hp);
}

pub fn heal(c: &mut Character, ev: &Evaluation, amount: i64) {
    let hp = (current_hp(c, ev) + amount.max(0)).min(max_hp(ev));
    c.state.hp = if hp >= max_hp(ev) { None } else { Some(hp) };
    if hp > 0 {
        c.state.death_successes = 0;
        c.state.death_failures = 0;
    }
}

/// Temporary hit points don't stack: keep the higher.
pub fn set_temp_hp(c: &mut Character, amount: i64) {
    c.state.temp_hp = c.state.temp_hp.max(amount.max(0));
}

pub fn resource_max(ev: &Evaluation, key: &str) -> i64 {
    ev.int(&format!("resource.{key}.max")).unwrap_or(0).max(0)
}

pub fn resource_left(c: &Character, ev: &Evaluation, key: &str) -> i64 {
    let used = c.state.resources_used.get(key).copied().unwrap_or(0);
    (resource_max(ev, key) - used).max(0)
}

/// Spend `n` uses (negative restores). Returns false if not enough are left.
pub fn spend_resource(c: &mut Character, ev: &Evaluation, key: &str, n: i64) -> bool {
    let max = resource_max(ev, key);
    let used = c.state.resources_used.get(key).copied().unwrap_or(0);
    let new = used + n;
    if new > max {
        return false;
    }
    set_used(&mut c.state.resources_used, key, new.clamp(0, max));
    true
}

fn set_used(map: &mut std::collections::BTreeMap<String, i64>, key: &str, used: i64) {
    if used <= 0 {
        map.remove(key);
    } else {
        map.insert(key.to_string(), used);
    }
}

pub fn slot_max(ev: &Evaluation, level: usize) -> i64 {
    ev.int(&format!("slots.{level}.max")).unwrap_or(0)
}

pub fn slots_left(c: &Character, ev: &Evaluation, level: usize) -> i64 {
    let used = c.state.slots_used.get(level - 1).copied().unwrap_or(0);
    (slot_max(ev, level) - used).max(0)
}

/// Expend (or with negative `n`, recover) slots of a level (1–9).
pub fn spend_slot(c: &mut Character, ev: &Evaluation, level: usize, n: i64) -> bool {
    if !(1..=9).contains(&level) {
        return false;
    }
    let max = slot_max(ev, level);
    let used = &mut c.state.slots_used;
    if used.len() < level {
        used.resize(level, 0);
    }
    let new = used[level - 1] + n;
    if new > max {
        return false;
    }
    used[level - 1] = new.clamp(0, max);
    while used.last() == Some(&0) {
        used.pop();
    }
    true
}

pub fn spend_pact_slot(c: &mut Character, ev: &Evaluation, n: i64) -> bool {
    let max = ev.int("pact.slots.max").unwrap_or(0);
    let new = c.state.pact_slots_used + n;
    if new > max {
        return false;
    }
    c.state.pact_slots_used = new.clamp(0, max);
    true
}

/// Spend a hit die of the given size. Healing is rolled at the table and
/// applied with [`heal`].
pub fn spend_hit_die(c: &mut Character, ev: &Evaluation, faces: i64, n: i64) -> bool {
    let key = format!("d{faces}");
    let max = ev.int(&format!("hit_dice.{key}.max")).unwrap_or(0);
    let used = c.state.hit_dice_used.get(&key).copied().unwrap_or(0);
    let new = used + n;
    if new > max {
        return false;
    }
    set_used(&mut c.state.hit_dice_used, &key, new.clamp(0, max));
    true
}

/// What a rest restored, for the status line.
#[derive(Debug, Default, PartialEq)]
pub struct RestReport {
    pub restored: Vec<String>,
}

pub fn short_rest(c: &mut Character, build: &Build, ev: &Evaluation) -> RestReport {
    let mut report = RestReport::default();
    for (r, _) in &build.resources {
        let used = c.state.resources_used.get(&r.key).copied().unwrap_or(0);
        if used == 0 {
            continue;
        }
        let regain = match r.recharge {
            Recharge::ShortRest => used,
            _ => ev.int(&format!("resource.{}.short_rest", r.key)).unwrap_or(0).min(used),
        };
        if regain > 0 {
            set_used(&mut c.state.resources_used, &r.key, used - regain);
            report.restored.push(format!("{} +{regain}", r.name));
        }
    }
    if c.state.pact_slots_used > 0 {
        report.restored.push("pact slots".into());
        c.state.pact_slots_used = 0;
    }
    report
}

pub fn long_rest(c: &mut Character, build: &Build, ev: &Evaluation, edition: Edition) -> RestReport {
    let mut report = RestReport::default();
    for (r, _) in &build.resources {
        if r.recharge != Recharge::Manual && c.state.resources_used.remove(&r.key).is_some() {
            report.restored.push(r.name.clone());
        }
    }
    if !c.state.slots_used.is_empty() || c.state.pact_slots_used > 0 {
        report.restored.push("spell slots".into());
    }
    c.state.slots_used.clear();
    c.state.pact_slots_used = 0;
    if c.state.hp.is_some() {
        report.restored.push("hit points".into());
    }
    c.state.hp = None;
    c.state.temp_hp = 0;
    c.state.death_successes = 0;
    c.state.death_failures = 0;
    c.state.concentration = None;
    if c.state.exhaustion > 0 {
        c.state.exhaustion -= 1;
        report.restored.push("1 exhaustion".into());
    }

    match edition {
        Edition::Modern => {
            if !c.state.hit_dice_used.is_empty() {
                report.restored.push("all hit dice".into());
            }
            c.state.hit_dice_used.clear();
        }
        Edition::Classic => {
            // Regain up to half the total hit dice (minimum 1), largest dice first.
            let total: i64 = ev.stats.keys().filter(|k| k.starts_with("hit_dice.")).filter_map(|k| ev.int(k)).sum();
            let mut budget = (total / 2).max(1);
            let mut dice: Vec<String> = c.state.hit_dice_used.keys().cloned().collect();
            dice.sort_by_key(|d| std::cmp::Reverse(d.trim_start_matches('d').parse::<i64>().unwrap_or(0)));
            let mut regained = 0;
            for d in dice {
                let used = c.state.hit_dice_used[&d];
                let back = used.min(budget);
                budget -= back;
                regained += back;
                set_used(&mut c.state.hit_dice_used, &d, used - back);
            }
            if regained > 0 {
                report.restored.push(format!("{regained} hit dice"));
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{Build, ClassState};
    use crate::engine::{Model, StatDef};
    use crate::formula::Formula;
    use crate::overlay::Resource;

    fn resource(key: &str, recharge: Recharge, regain: Option<&str>) -> (Resource, String) {
        (
            Resource {
                key: key.into(),
                name: key.into(),
                max: Formula::parse("3").unwrap(),
                recharge,
                short_rest_regain: regain.map(|r| Formula::parse(r).unwrap()),
            },
            "test".into(),
        )
    }

    fn setup() -> (Character, Build, Evaluation) {
        let mut model = Model::default();
        for (k, v) in [
            ("hp.max", "20"),
            ("resource.focus.max", "3"),
            ("resource.rage.max", "3"),
            ("resource.rage.short_rest", "1"),
            ("slots.1.max", "4"),
            ("slots.2.max", "2"),
            ("pact.slots.max", "2"),
            ("hit_dice.d10.max", "3"),
            ("hit_dice.d6.max", "2"),
        ] {
            model.stats.push(StatDef::new(k, v, "test"));
        }
        let build = Build {
            model,
            classes: Vec::<ClassState>::new(),
            features: Vec::new(),
            resources: vec![
                resource("focus", Recharge::ShortRest, None),
                resource("rage", Recharge::LongRest, Some("1")),
            ],
            toggles: Vec::new(),
            granted: Vec::new(),
            warnings: Vec::new(),
        };
        let ev = build.evaluate();
        (Character::default(), build, ev)
    }

    #[test]
    fn damage_and_healing() {
        let (mut c, _, ev) = setup();
        set_temp_hp(&mut c, 5);
        set_temp_hp(&mut c, 3);
        assert_eq!(c.state.temp_hp, 5, "temp HP doesn't stack");
        damage(&mut c, &ev, 8);
        assert_eq!((c.state.temp_hp, current_hp(&c, &ev)), (0, 17));
        damage(&mut c, &ev, 100);
        assert_eq!(current_hp(&c, &ev), 0);
        heal(&mut c, &ev, 50);
        assert_eq!(c.state.hp, None, "full HP is stored as None");
    }

    #[test]
    fn resources_and_slots() {
        let (mut c, _, ev) = setup();
        assert!(spend_resource(&mut c, &ev, "focus", 2));
        assert!(!spend_resource(&mut c, &ev, "focus", 2), "only 1 left");
        assert_eq!(resource_left(&c, &ev, "focus"), 1);
        assert!(spend_resource(&mut c, &ev, "focus", -5));
        assert!(c.state.resources_used.is_empty());

        assert!(spend_slot(&mut c, &ev, 2, 1));
        assert_eq!(c.state.slots_used, vec![0, 1]);
        assert_eq!(slots_left(&c, &ev, 2), 1);
        assert!(!spend_slot(&mut c, &ev, 3, 1), "no 3rd-level slots");
        assert!(spend_slot(&mut c, &ev, 2, -1));
        assert!(c.state.slots_used.is_empty());
    }

    #[test]
    fn short_rest_partial_regain() {
        let (mut c, build, ev) = setup();
        spend_resource(&mut c, &ev, "focus", 3);
        spend_resource(&mut c, &ev, "rage", 3);
        spend_pact_slot(&mut c, &ev, 2);
        spend_slot(&mut c, &ev, 1, 2);
        let report = short_rest(&mut c, &build, &ev);
        assert_eq!(resource_left(&c, &ev, "focus"), 3);
        assert_eq!(resource_left(&c, &ev, "rage"), 1, "2024 Rage regains one");
        assert_eq!(c.state.pact_slots_used, 0);
        assert_eq!(slots_left(&c, &ev, 1), 2, "regular slots wait for a long rest");
        assert_eq!(report.restored.len(), 3);
    }

    #[test]
    fn long_rest_by_edition() {
        let (mut c, build, ev) = setup();
        damage(&mut c, &ev, 10);
        spend_resource(&mut c, &ev, "rage", 3);
        spend_hit_die(&mut c, &ev, 10, 3);
        spend_hit_die(&mut c, &ev, 6, 2);
        c.state.exhaustion = 2;

        let mut classic = c.clone();
        long_rest(&mut classic, &build, &ev, Edition::Classic);
        // Half of 5 total = 2, largest dice first.
        assert_eq!(classic.state.hit_dice_used.get("d10"), Some(&1));
        assert_eq!(classic.state.hit_dice_used.get("d6"), Some(&2));
        assert_eq!(classic.state.hp, None);
        assert_eq!(classic.state.exhaustion, 1);

        long_rest(&mut c, &build, &ev, Edition::Modern);
        assert!(c.state.hit_dice_used.is_empty());
        assert!(c.state.resources_used.is_empty());
    }

    #[test]
    fn spent_hit_dice_are_bounded() {
        let (mut c, _, ev) = setup();
        assert!(spend_hit_die(&mut c, &ev, 6, 2));
        assert!(!spend_hit_die(&mut c, &ev, 6, 1));
        assert!(!spend_hit_die(&mut c, &ev, 12, 1), "no d12s");
    }
}
