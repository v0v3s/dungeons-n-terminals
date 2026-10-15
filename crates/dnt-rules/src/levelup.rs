//! What a character can choose when gaining a level: which class (existing
//! or new, with multiclass requirements), subclass, and ASI or feat.

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::Character;
use crate::engine::Evaluation;
use crate::multiclass;

/// The kind of choice a class grants at a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelPerk {
    None,
    /// Ability Score Improvement: +2 / +1+1, or a feat.
    Asi,
    /// 2024 Epic Boon (level 19): a feat, usually from the Epic Boon category.
    EpicBoon,
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

/// What choice, if any, the class grants at `level`.
pub fn perk_at(class: &Json, level: i64) -> LevelPerk {
    for (r, _) in feature_refs(class) {
        if ref_level(r) != Some(level) {
            continue;
        }
        match r.split('|').next() {
            Some("Ability Score Improvement") => return LevelPerk::Asi,
            Some("Epic Boon") => return LevelPerk::EpicBoon,
            _ => {}
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
pub fn class_options(lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> Vec<ClassOption> {
    let score = |ab: &str| ev.int(ab).unwrap_or(0);
    let unmet = |entity: EntityId| -> Option<String> {
        let reqs = multiclass::requirements(&lib.entity(entity).data);
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

/// Feats for an ASI or Epic Boon choice. 2024 feats are limited to the
/// fitting category (General, or Epic Boon); 2014 feats have no category.
pub fn feat_options(lib: &Library, perk: LevelPerk, filter: &SourceFilter) -> Vec<EntityId> {
    let mut feats: Vec<EntityId> = lib
        .entities()
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == Kind::Feat && filter.allows(e))
        .filter(|(_, e)| {
            let category = e.data.get("category").and_then(Json::as_str);
            match (perk, category) {
                (_, None) => true,
                (LevelPerk::EpicBoon, Some(c)) => c == "EB" || c == "G",
                (_, Some(c)) => c == "G",
            }
        })
        .filter(|(i, e)| e.name != "Ability Score Improvement" && lib.versions_of(*i).is_empty())
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
        let fighter = json!({"classFeatures": [
            "Second Wind|Fighter|XPHB|1",
            {"classFeature": "Fighter Subclass|Fighter|XPHB|3", "gainSubclassFeature": true},
            "Ability Score Improvement|Fighter|XPHB|4",
            {"classFeature": "Subclass Feature|Fighter|XPHB|7", "gainSubclassFeature": true},
            "Epic Boon|Fighter|XPHB|19"
        ]});
        assert_eq!(subclass_level(&fighter), Some(3));
        assert_eq!(perk_at(&fighter, 4), LevelPerk::Asi);
        assert_eq!(perk_at(&fighter, 5), LevelPerk::None);
        assert_eq!(perk_at(&fighter, 19), LevelPerk::EpicBoon);
    }
}
