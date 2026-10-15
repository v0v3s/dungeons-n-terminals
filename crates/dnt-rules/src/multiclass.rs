//! Multiclassing rules: combined spellcaster level and slots, and
//! multiclass ability requirements.

use serde_json::Value as Json;

use crate::standard::ABILITIES;

/// How a class (or subclass) contributes to the multiclass caster level,
/// as encoded in the data's `casterProgression`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progression {
    Full,
    /// Half, rounded down (2014 Paladin/Ranger).
    Half,
    /// Half, rounded up (Artificer; 2024 Paladin/Ranger).
    HalfUp,
    /// Third, rounded down (Eldritch Knight, Arcane Trickster).
    Third,
    /// Warlock Pact Magic: separate slots, no caster level.
    Pact,
}

impl Progression {
    pub fn from_json(v: Option<&Json>) -> Option<Progression> {
        Some(match v?.as_str()? {
            "full" => Progression::Full,
            "1/2" => Progression::Half,
            "artificer" => Progression::HalfUp,
            "1/3" => Progression::Third,
            "pact" => Progression::Pact,
            _ => return None,
        })
    }

    /// Contribution to the multiclass spellcaster level.
    pub fn caster_levels(self, class_level: i64) -> i64 {
        match self {
            Progression::Full => class_level,
            Progression::Half => class_level / 2,
            Progression::HalfUp => (class_level + 1) / 2,
            Progression::Third => class_level / 3,
            Progression::Pact => 0,
        }
    }
}

/// The multiclass spellcaster table: slots of levels 1–9 per caster level.
pub const SLOTS: [[u8; 9]; 20] = [
    [2, 0, 0, 0, 0, 0, 0, 0, 0],
    [3, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, 2, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 0, 0, 0, 0, 0, 0, 0],
    [4, 3, 2, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 0, 0, 0, 0, 0, 0],
    [4, 3, 3, 1, 0, 0, 0, 0, 0],
    [4, 3, 3, 2, 0, 0, 0, 0, 0],
    [4, 3, 3, 3, 1, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 0, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 0, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 0, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 0],
    [4, 3, 3, 3, 2, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 1, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 1, 1, 1],
    [4, 3, 3, 3, 3, 2, 2, 1, 1],
];

/// Combined caster level of several (progression, class level) pairs.
pub fn caster_level(classes: &[(Progression, i64)]) -> i64 {
    classes.iter().map(|(p, l)| p.caster_levels(*l)).sum()
}

/// Slots for a combined caster level (0 → none; above 20 → as 20).
pub fn slots_for(caster_level: i64) -> [u8; 9] {
    if caster_level <= 0 {
        return [0; 9];
    }
    SLOTS[(caster_level.min(20) - 1) as usize]
}

/// One way of qualifying: every `all` minimum, plus at least one `any`
/// minimum when `any` is non-empty.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Requirement {
    pub all: Vec<(String, i64)>,
    pub any: Vec<(String, i64)>,
}

impl Requirement {
    pub fn met(&self, score: &dyn Fn(&str) -> i64) -> bool {
        self.all.iter().all(|(a, n)| score(a) >= *n) && (self.any.is_empty() || self.any.iter().any(|(a, n)| score(a) >= *n))
    }

    /// e.g. `Strength 13 and Charisma 13`, `Strength 13 or Dexterity 13`.
    pub fn describe(&self) -> String {
        let name = |a: &str| ABILITIES.iter().find(|(k, _)| *k == a).map_or(a.to_string(), |(_, n)| n.to_string());
        let fmt = |list: &[(String, i64)], sep: &str| list.iter().map(|(a, n)| format!("{} {n}", name(a))).collect::<Vec<_>>().join(sep);
        match (self.all.is_empty(), self.any.is_empty()) {
            (false, true) => fmt(&self.all, " and "),
            (true, false) => fmt(&self.any, " or "),
            (false, false) => format!("{} and ({})", fmt(&self.all, " and "), fmt(&self.any, " or ")),
            (true, true) => "no requirement".into(),
        }
    }
}

fn ability_order(a: &str) -> usize {
    ABILITIES.iter().position(|(k, _)| *k == a).unwrap_or(ABILITIES.len())
}

/// Multiclass requirements of a class: alternatives, any of which qualifies.
/// 2014 classes list `multiclassing.requirements`; 2024 classes require 13
/// in their primary ability.
pub fn requirements(class: &Json) -> Vec<Requirement> {
    let scores = |o: &serde_json::Map<String, Json>| -> Vec<(String, i64)> {
        let mut v: Vec<(String, i64)> = o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_i64()?))).collect();
        v.sort_by_key(|(a, _)| ability_order(a));
        v
    };
    if let Some(req) = class.get("multiclassing").and_then(|m| m.get("requirements")).and_then(Json::as_object) {
        let all = scores(req);
        let any = req.get("or").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_object).flat_map(scores).collect();
        return vec![Requirement { all, any }];
    }
    class
        .get("primaryAbility")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_object)
        .map(|o| Requirement {
            all: o.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| (k.clone(), 13)).collect(),
            any: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn caster_levels_round_per_edition() {
        assert_eq!(caster_level(&[(Progression::Full, 3), (Progression::Half, 5)]), 5);
        assert_eq!(caster_level(&[(Progression::Full, 3), (Progression::HalfUp, 5)]), 6);
        assert_eq!(caster_level(&[(Progression::Third, 8), (Progression::Pact, 5)]), 2);
        assert_eq!(slots_for(6), [4, 3, 3, 0, 0, 0, 0, 0, 0]);
        assert_eq!(slots_for(0), [0; 9]);
        assert_eq!(slots_for(25), SLOTS[19]);
    }

    #[test]
    fn requirements_2014() {
        let paladin = json!({"multiclassing": {"requirements": {"str": 13, "cha": 13}}});
        let fighter = json!({"multiclassing": {"requirements": {"or": [{"str": 13, "dex": 13}]}}});
        let p = &requirements(&paladin)[0];
        let f = &requirements(&fighter)[0];
        let scores = |s: &str| match s {
            "str" => 8,
            "dex" => 14,
            "cha" => 13,
            _ => 10,
        };
        assert!(!p.met(&scores));
        assert!(f.met(&scores));
        assert_eq!(f.describe(), "Strength 13 or Dexterity 13");
        assert_eq!(p.describe().matches(" and ").count(), 1);
    }

    #[test]
    fn requirements_2024_primary_ability() {
        let fighter = json!({"primaryAbility": [{"str": true}, {"dex": true}]});
        let reqs = requirements(&fighter);
        assert_eq!(reqs.len(), 2);
        assert!(reqs.iter().any(|r| r.met(&|a| if a == "dex" { 13 } else { 8 })));
        assert!(!reqs.iter().any(|r| r.met(&|_| 12)));
    }
}
