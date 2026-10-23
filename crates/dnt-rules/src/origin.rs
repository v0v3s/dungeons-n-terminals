//! What a species, background or class grants at creation: skill
//! proficiencies, ability score increases and feats, parsed from the
//! data shapes (`skillProficiencies`, `ability`, `feats`).

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::class_table::slug;
use crate::standard::SKILLS;

/// Skills granted: some fixed, plus `count` chosen from `from` (empty
/// `from` means any skill).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillGrant {
    pub fixed: Vec<String>,
    pub from: Vec<String>,
    pub count: usize,
}

impl SkillGrant {
    /// The skills this grant lets you pick from.
    pub fn options(&self) -> Vec<String> {
        if self.from.is_empty() {
            SKILLS.iter().map(|(k, _, _)| k.to_string()).collect()
        } else {
            self.from.clone()
        }
    }
}

/// Parse `skillProficiencies` / `startingProficiencies.skills`. Only the
/// first alternative is used (alternatives are rare and homebrew-only).
pub fn skill_grant(v: Option<&Json>) -> SkillGrant {
    let Some(first) = v.and_then(Json::as_array).and_then(|a| a.first()).and_then(Json::as_object) else {
        return SkillGrant::default();
    };
    let known = |s: &str| {
        let k = slug(s);
        SKILLS.iter().any(|(sk, _, _)| *sk == k).then_some(k)
    };
    let mut g = SkillGrant::default();
    for (k, v) in first {
        match (k.as_str(), v) {
            ("choose", Json::Object(c)) => {
                g.from = c.get("from").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).filter_map(known).collect();
                g.count = c.get("count").and_then(Json::as_u64).unwrap_or(1) as usize;
            }
            ("any", n) => g.count = n.as_u64().unwrap_or(1) as usize,
            (skill, Json::Bool(true)) => g.fixed.extend(known(skill)),
            _ => {}
        }
    }
    g
}

/// One part of an ability score grant.
#[derive(Debug, Clone, PartialEq)]
pub enum AbilityGrant {
    Fixed(BTreeMap<String, i64>),
    /// Increase `count` different abilities from `from` by `amount` each.
    Choose {
        from: Vec<String>,
        count: usize,
        amount: i64,
    },
    /// Distribute `weights` (e.g. +2 and +1) among different abilities of `from`.
    Weighted {
        from: Vec<String>,
        weights: Vec<i64>,
    },
}

/// Parse an `ability` array: each element is an alternative, made of fixed
/// increases and/or choices.
pub fn ability_grants(v: Option<&Json>) -> Vec<Vec<AbilityGrant>> {
    let strings = |v: Option<&Json>| -> Vec<String> {
        v.and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_string).collect()
    };
    v.and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_object)
        .map(|alt| {
            let mut parts = Vec::new();
            let fixed: BTreeMap<String, i64> = alt.iter().filter_map(|(k, v)| Some((k.clone(), v.as_i64()?))).collect();
            if !fixed.is_empty() {
                parts.push(AbilityGrant::Fixed(fixed));
            }
            if let Some(c) = alt.get("choose") {
                if let Some(w) = c.get("weighted") {
                    parts.push(AbilityGrant::Weighted {
                        from: strings(w.get("from")),
                        weights: w.get("weights").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_i64).collect(),
                    });
                } else {
                    parts.push(AbilityGrant::Choose {
                        from: strings(c.get("from")),
                        count: c.get("count").and_then(Json::as_u64).unwrap_or(1) as usize,
                        amount: c.get("amount").and_then(Json::as_i64).unwrap_or(1),
                    });
                }
            }
            parts
        })
        .filter(|p: &Vec<AbilityGrant>| !p.is_empty())
        .collect()
}

/// A species' ability score increases: its `ability` list, or, for a
/// species written for "customizing your origin" (`lineage`, as in
/// Monsters of the Multiverse), +2 and +1, or +1 to three, to any.
pub fn species_ability_grants(data: &Json) -> Vec<Vec<AbilityGrant>> {
    let listed = ability_grants(data.get("ability"));
    let lineage = data.get("lineage").is_some_and(|l| l.as_bool().unwrap_or(true) && !l.is_null());
    if !listed.is_empty() || !lineage {
        return listed;
    }
    let all: Vec<String> = ["str", "dex", "con", "int", "wis", "cha"].iter().map(|a| a.to_string()).collect();
    vec![
        vec![AbilityGrant::Weighted { from: all.clone(), weights: vec![2, 1] }],
        vec![AbilityGrant::Weighted { from: all, weights: vec![1, 1, 1] }],
    ]
}

/// Feats granted: fixed ones (as entity ids) and/or a choice from categories.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FeatGrant {
    pub fixed: Vec<String>,
    /// Feat categories to choose from (e.g. `["O"]` for Origin) and how many.
    pub choose: Option<(Vec<String>, usize)>,
}

pub fn feat_grant(v: Option<&Json>) -> FeatGrant {
    let mut g = FeatGrant::default();
    for alt in v.and_then(Json::as_array).into_iter().flatten().take(1).filter_map(Json::as_object) {
        for (k, v) in alt {
            if k == "anyFromCategory" {
                let cats =
                    v.get("category").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_string).collect();
                g.choose = Some((cats, v.get("count").and_then(Json::as_u64).unwrap_or(1) as usize));
            } else if v.as_bool() == Some(true) && !k.starts_with("any") {
                // `magic initiate; cleric|xphb` names a version of the feat
                // (expanded from `_versions` by the library).
                let (name, source) = k.split_once('|').unwrap_or((k, "phb"));
                g.fixed.push(format!("feat:{}|{source}", name.trim()));
            }
        }
    }
    g
}

#[cfg(test)]
mod tests {
    #[test]
    fn lineage_species_choose_their_increases() {
        let kenku = serde_json::json!({"name": "Kenku", "source": "MPMM", "lineage": "VRGR"});
        let alts = species_ability_grants(&kenku);
        assert_eq!(alts.len(), 2, "+2/+1, or +1/+1/+1");
        let elf = serde_json::json!({"name": "Elf", "ability": [{"dex": 2}]});
        assert_eq!(species_ability_grants(&elf).len(), 1);
        let xphb = serde_json::json!({"name": "Elf", "source": "XPHB"});
        assert!(species_ability_grants(&xphb).is_empty());
        assert!(species_ability_grants(&serde_json::json!({"lineage": null})).is_empty());
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn skills() {
        let bg = skill_grant(Some(&json!([{"insight": true, "religion": true}])));
        assert_eq!(bg.fixed, vec!["insight", "religion"]);
        assert_eq!(bg.count, 0);
        let rogue = skill_grant(Some(&json!([{"choose": {"from": ["stealth", "sleight of hand"], "count": 2}}])));
        assert_eq!(rogue.from, vec!["stealth", "sleight_of_hand"]);
        assert_eq!(rogue.count, 2);
        let bard = skill_grant(Some(&json!([{"any": 3}])));
        assert_eq!((bard.count, bard.options().len()), (3, 18));
        let elf = skill_grant(Some(&json!([{"choose": {"from": ["insight", "perception", "survival"]}}])));
        assert_eq!(elf.count, 1);
    }

    #[test]
    fn abilities() {
        let dragonborn = ability_grants(Some(&json!([{"str": 2, "cha": 1}])));
        assert_eq!(dragonborn, vec![vec![AbilityGrant::Fixed([("cha".to_string(), 1), ("str".to_string(), 2)].into())]]);
        let half_elf = ability_grants(Some(&json!([{"cha": 2, "choose": {"from": ["str", "dex"], "count": 2}}])));
        assert_eq!(half_elf[0].len(), 2);
        let acolyte = ability_grants(Some(&json!([
            {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [2, 1]}}},
            {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [1, 1, 1]}}}
        ])));
        assert_eq!(acolyte.len(), 2, "two alternatives");
        assert!(matches!(&acolyte[1][0], AbilityGrant::Weighted { weights, .. } if weights == &vec![1, 1, 1]));
        assert!(ability_grants(None).is_empty());
    }

    #[test]
    fn feats() {
        let acolyte = feat_grant(Some(&json!([{"magic initiate; cleric|xphb": true}])));
        assert_eq!(acolyte.fixed, vec!["feat:magic initiate; cleric|xphb"]);
        let human = feat_grant(Some(&json!([{"anyFromCategory": {"category": ["O"], "count": 1}}])));
        assert_eq!(human.choose, Some((vec!["O".to_string()], 1)));
    }
}
