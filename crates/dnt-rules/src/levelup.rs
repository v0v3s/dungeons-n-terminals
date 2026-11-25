//! What a character can choose when gaining a level: which class (existing
//! or new, with multiclass requirements), subclass, and ASI or feat.

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::Character;
use crate::engine::Evaluation;
use crate::multiclass;
use crate::overlay::{OverlaySet, Pick};

/// The kind of choice a class grants at a level (its features' rules).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LevelPerk {
    None,
    /// Ability scores to increase (+2, or +1 and +1), or a feat instead:
    /// one of these categories (the data's), or one without a category.
    Asi {
        title: String,
        feats: Vec<String>,
    },
    /// A feat of these categories, or one without a category.
    Feat {
        title: String,
        feats: Vec<String>,
    },
}

fn feature_refs(class: &Json) -> impl Iterator<Item = (&str, bool)> {
    class.get("classFeatures").and_then(Json::as_array).into_iter().flatten().filter_map(|f| match f {
        Json::String(s) => Some((s.as_str(), false)),
        Json::Object(o) => Some((o.get("classFeature")?.as_str()?, o.get("gainSubclassFeature").and_then(Json::as_bool) == Some(true))),
        _ => None,
    })
}

fn ref_level(r: &str) -> Option<i64> {
    r.split('|').nth(3)?.parse().ok()
}

/// The class level at which the subclass is chosen.
pub fn subclass_level(class: &Json) -> Option<i64> {
    feature_refs(class).filter(|(_, sub)| *sub).filter_map(|(r, _)| ref_level(r)).min()
}

/// What choice, if any, the class grants at `level`: a feature gained then
/// whose rules ask to increase abilities or for a feat.
pub fn perk_at(lib: &Library, rules: &OverlaySet, class: EntityId, level: i64) -> LevelPerk {
    for (r, _) in feature_refs(&lib.entity(class).data) {
        if ref_level(r) != Some(level) {
            continue;
        }
        let Some(f) = lib.resolve_feature_ref(Kind::ClassFeature, r) else { continue };
        for o in rules.for_had(lib, f, Some(class)) {
            for c in &o.choices {
                let (title, feats) = (c.key.clone(), c.feats.clone());
                match c.pick {
                    Pick::AbilityIncrease => return LevelPerk::Asi { title, feats },
                    Pick::Feat => return LevelPerk::Feat { title, feats },
                    _ => {}
                }
            }
        }
    }
    LevelPerk::None
}

pub fn class_id(lib: &Library, class: EntityId) -> String {
    let e = lib.entity(class);
    format!("class:{}|{}", e.name, e.source)
}

pub fn subclass_id(lib: &Library, subclass: EntityId) -> String {
    let e = lib.entity(subclass);
    let s = |k: &str| e.data.get(k).and_then(Json::as_str).unwrap_or("");
    format!("subclass:{}|{}|{}|{}", s("shortName"), s("className"), s("classSource"), e.source)
}

pub fn feat_id(lib: &Library, feat: EntityId) -> String {
    let e = lib.entity(feat);
    format!("feat:{}|{}", e.name, e.source)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassOption {
    pub id: String,
    pub entity: EntityId,
    pub name: String,
    pub source: String,
    /// Current level in this class (0 = new class).
    pub level: i64,
    /// Unmet multiclass requirements, if any.
    pub warning: Option<String>,
}

/// Classes the character can advance: current classes first, then every
/// other class enabled by `filter`.
pub fn class_options(lib: &Library, rules: &OverlaySet, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> Vec<ClassOption> {
    let score = |ab: &str| ev.int(ab).unwrap_or(0);
    let unmet = |entity: EntityId| -> Option<String> {
        let reqs = multiclass::requirements(&lib.entity(entity).data, rules.multiclass.primary_score);
        if reqs.is_empty() || reqs.iter().any(|r| r.met(&score)) {
            None
        } else {
            Some(reqs.iter().map(|r| r.describe()).collect::<Vec<_>>().join("; or "))
        }
    };
    let current: Vec<(String, i64)> = c.class_levels().into_iter().map(|(id, n)| (id.to_string(), n)).collect();
    let multiclassing = !current.is_empty();

    let mut out = Vec::new();
    for (id, level) in &current {
        let Some(entity) = crate::overlay::resolve_id(lib, id) else { continue };
        let e = lib.entity(entity);
        out.push(ClassOption { id: id.clone(), entity, name: e.name.clone(), source: e.source.clone(), level: *level, warning: None });
    }
    // To multiclass, both the new class and every current class must qualify.
    let current_unmet: Vec<String> = out.iter().filter_map(|o| unmet(o.entity).map(|w| format!("{}: {w}", o.name))).collect();
    let mut others: Vec<ClassOption> = lib
        .entities()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == Kind::Class && filter.allows(e))
        .filter(|(i, _)| !out.iter().any(|o| o.entity == *i))
        .map(|(i, e)| {
            let mut problems: Vec<String> = Vec::new();
            if multiclassing {
                if let Some(w) = unmet(i) {
                    problems.push(format!("needs {w}"));
                }
                problems.extend(current_unmet.iter().cloned());
            }
            ClassOption {
                id: class_id(lib, i),
                entity: i,
                name: e.name.clone(),
                source: e.source.clone(),
                level: 0,
                warning: (!problems.is_empty()).then(|| problems.join("; ")),
            }
        })
        .collect();
    others.sort_by(|a, b| a.name.cmp(&b.name).then(a.source.cmp(&b.source)));
    out.extend(others);
    out
}

/// Subclasses of a class enabled by `filter`.
pub fn subclass_options(lib: &Library, class: EntityId, filter: &SourceFilter) -> Vec<EntityId> {
    lib.subclasses_of(class).into_iter().filter(|&s| filter.allows(lib.entity(s))).collect()
}

/// The feats a perk offers: of its categories or none, and not a feat
/// that is itself the ability increase (its rule's choice).
pub fn feat_options(lib: &Library, rules: &OverlaySet, perk: &LevelPerk, filter: &SourceFilter) -> Vec<EntityId> {
    let categories: &[String] = match perk {
        LevelPerk::Asi { feats, .. } | LevelPerk::Feat { feats, .. } => feats,
        LevelPerk::None => &[],
    };
    let mut feats: Vec<EntityId> = lib
        .entities()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == Kind::Feat && filter.allows(e))
        .filter(|(_, e)| {
            let category = e.data.get("category").and_then(Json::as_str);
            category.is_none_or(|c| categories.iter().any(|k| k == c))
        })
        .filter(|(i, e)| {
            let increase = rules.for_entity(&e.key).iter().any(|o| o.choices.iter().any(|c| c.pick == Pick::AbilityIncrease));
            !increase && lib.versions_of(*i).is_empty()
        })
        .map(|(i, _)| i)
        .collect();
    feats.sort_by(|&a, &b| lib.entity(a).name.cmp(&lib.entity(b).name));
    feats
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn levels_from_class_features() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "class": [{"name": "Fighter", "source": "T", "classFeatures": [
                "Second Wind|Fighter|T|1",
                {"classFeature": "Fighter Subclass|Fighter|T|3", "gainSubclassFeature": true},
                "Ability Score Improvement|Fighter|T|4",
                "Epic Boon|Fighter|T|19"
            ]}],
            "classFeature": [
                {"name": "Second Wind", "source": "T", "className": "Fighter", "classSource": "T", "level": 1, "entries": ["x"]},
                {"name": "Fighter Subclass", "source": "T", "className": "Fighter", "classSource": "T", "level": 3, "entries": ["x"]},
                {"name": "Ability Score Improvement", "source": "T", "className": "Fighter", "classSource": "T", "level": 4, "entries": ["x"]},
                {"name": "Epic Boon", "source": "T", "className": "Fighter", "classSource": "T", "level": 19, "entries": ["x"]}
            ],
            "feat": [
                {"name": "Alert", "source": "T", "category": "O", "entries": ["x"]},
                {"name": "Tough", "source": "T", "category": "G", "entries": ["x"]},
                {"name": "Boon of Speed", "source": "T", "category": "EB", "entries": ["x"]},
                {"name": "Ability Score Improvement", "source": "XPHB", "category": "G", "entries": ["x"]}
            ]
        }));
        let fighter = lib.get(&dnt_data::Key::new(Kind::Class, &["Fighter", "T"])).unwrap();
        assert_eq!(subclass_level(&lib.entity(fighter).data), Some(3));
        // The built-in rules: every class's Ability Score Improvement and Epic Boon.
        let rules = crate::builtin::overlays().0;
        let asi = perk_at(&lib, &rules, fighter, 4);
        assert_eq!(asi, LevelPerk::Asi { title: "Ability Score Improvement".into(), feats: vec!["G".into()] });
        assert_eq!(perk_at(&lib, &rules, fighter, 5), LevelPerk::None);
        let boon = perk_at(&lib, &rules, fighter, 19);
        assert!(matches!(&boon, LevelPerk::Feat { title, .. } if title == "Epic Boon"));
        assert_eq!(perk_at(&lib, &OverlaySet::default(), fighter, 4), LevelPerk::None, "no rules, no perk");
        let names = |perk: &LevelPerk| -> Vec<String> {
            feat_options(&lib, &rules, perk, &SourceFilter::default()).iter().map(|&f| lib.entity(f).name.clone()).collect()
        };
        // General feats, not the ability increase itself.
        assert_eq!(names(&asi), ["Tough"]);
        assert_eq!(names(&boon), ["Boon of Speed", "Tough"]);
    }
}
