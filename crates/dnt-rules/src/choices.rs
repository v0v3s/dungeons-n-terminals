//! Choices a class grants when a level is gained: feats (2024 Fighting
//! Style), optional features (invocations, maneuvers, metamagic), options
//! inside features (Divine Order, Hunter's Prey), Expertise and Weapon
//! Mastery. Picks are stored per level in `LevelChoices::picks`, keyed by
//! the choice's `key`.

use std::collections::BTreeSet;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::Character;
use crate::class_table::{class_columns, slug};
use crate::levelup::feat_id;
use crate::overlay::resolve_id;
use crate::standard::SKILLS;
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceKind {
    Feat,
    OptionalFeature,
    /// One of the options listed inside a feature (Divine Order, …).
    SubFeature,
    Expertise,
    WeaponMastery,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceOption {
    /// Stored value: an entity id (`feat:Defense|XPHB`) or a skill key.
    pub value: String,
    pub label: String,
    pub hint: String,
    /// e.g. an unmet prerequisite.
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// Storage key, e.g. `Fighting Style`, `Eldritch Invocations`, `Expertise`.
    pub key: String,
    pub kind: ChoiceKind,
    pub count: usize,
    pub options: Vec<ChoiceOption>,
}

/// Running total of a data progression at `level`: arrays give the total
/// per level, objects give totals at the listed levels.
fn progression_total(p: &Json, level: i64) -> i64 {
    match p {
        Json::Array(a) => a.get((level - 1).max(0) as usize).and_then(Json::as_i64).filter(|_| level >= 1).unwrap_or(0),
        Json::Object(o) => o
            .iter()
            .filter_map(|(k, v)| Some((k.parse::<i64>().ok()?, v.as_i64()?)))
            .filter(|(l, _)| *l <= level)
            .max_by_key(|(l, _)| *l)
            .map_or(0, |(_, v)| v),
        _ => 0,
    }
}

fn strs(v: Option<&Json>) -> Vec<String> {
    v.and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(str::to_string).collect()
}

/// Every value already picked anywhere on the character.
fn all_picks(c: &Character) -> BTreeSet<String> {
    let mut out: BTreeSet<String> =
        c.levels.iter().flat_map(|l| l.choices.picks.values().flatten().cloned().chain(l.choices.feat.clone())).collect();
    out.extend(c.feats.iter().cloned());
    out.extend(c.options.iter().cloned());
    out
}

/// Feats choosable for a category list, with versions (e.g. Magic
/// Initiate; Cleric) in place of their base.
pub fn feats_in(lib: &Library, categories: &[String], filter: &SourceFilter, taken: &BTreeSet<String>) -> Vec<ChoiceOption> {
    let mut out: Vec<ChoiceOption> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            e.kind == Kind::Feat
                && filter.allows(e)
                && lib.versions_of(i).is_empty()
                && e.data.get("category").and_then(Json::as_str).is_some_and(|c| categories.iter().any(|k| k == c))
        })
        .map(|i| ChoiceOption {
            value: feat_id(lib, i),
            label: lib.entity(i).name.clone(),
            hint: lib.entity(i).source.clone(),
            warning: None,
        })
        .filter(|o| !taken.contains(&o.value))
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Every spell the character has: class cantrips and spells, spellbook,
/// and spells chosen for feature grants.
fn known_spells(lib: &Library, c: &Character) -> Vec<EntityId> {
    c.spells
        .classes
        .values()
        .flat_map(|k| k.cantrips.iter().chain(&k.prepared).chain(&k.spellbook))
        .chain(c.spells.chosen.values().flatten())
        .filter_map(|id| resolve_id(lib, id))
        .collect()
}

/// Why an option's prerequisites are not met, or `None`. data lists
/// alternatives; all parts of one alternative must hold: a class level,
/// other options taken (Pact of the Blade), spells known.
fn unmet_prerequisite(
    lib: &Library,
    c: &Character,
    d: &Json,
    class_level: i64,
    filter: &SourceFilter,
    taken: &BTreeSet<String>,
) -> Option<String> {
    let alts = d.get("prerequisite")?.as_array()?;
    let known = known_spells(lib, c);
    let mut first_reason = None;
    for alt in alts {
        let mut why = Vec::new();
        let level = match alt.get("level") {
            Some(Json::Number(n)) => n.as_i64(),
            Some(Json::Object(l)) => l.get("level").and_then(Json::as_i64),
            _ => None,
        };
        if let Some(l) = level.filter(|l| *l > class_level) {
            why.push(format!("level {l}"));
        }
        for r in alt.get("optionalfeature").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str) {
            let id = format!("optionalfeature:{r}");
            let entity = resolve_id(lib, &id);
            let have = taken.iter().any(|t| resolve_id(lib, t).is_some_and(|e| Some(e) == entity) || t.eq_ignore_ascii_case(&id));
            if !have {
                why.push(entity.map(|e| lib.entity(e).name.clone()).unwrap_or_else(|| r.split('|').next().unwrap_or(r).to_string()));
            }
        }
        for sp in alt.get("spell").and_then(Json::as_array).into_iter().flatten() {
            match sp {
                Json::String(name) => {
                    let name = name.split(['#', '|']).next().unwrap_or(name);
                    if !known.iter().any(|&k| lib.entity(k).name.eq_ignore_ascii_case(name)) {
                        why.push(dnt_data::view::title_case(name));
                    }
                }
                Json::Object(o) => {
                    let Some(f) = o.get("choose").and_then(Json::as_str) else { continue };
                    let options = crate::spells::filter_spells(lib, f, filter);
                    if !known.iter().any(|k| options.contains(k)) {
                        let what = o.get("entrySummary").or_else(|| o.get("entry")).and_then(Json::as_str).map(str::to_string);
                        why.push(what.unwrap_or_else(|| crate::spells::describe_filter(f)));
                    }
                }
                _ => {}
            }
        }
        if why.is_empty() {
            return None;
        }
        first_reason.get_or_insert(format!("needs {}", why.join(", ")));
    }
    first_reason
}

fn optional_features(
    lib: &Library,
    c: &Character,
    types: &[String],
    filter: &SourceFilter,
    taken: &BTreeSet<String>,
    class_level: i64,
) -> Vec<ChoiceOption> {
    // 2014 options reprinted as 2024 feats (fighting styles) must stay
    // available to 2014 classes, so reprints are not hidden here.
    let filter = SourceFilter { hide_reprinted: false, ..filter.clone() };
    let mut out: Vec<ChoiceOption> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            e.kind == Kind::OptionalFeature && filter.allows(e) && strs(e.data.get("featureType")).iter().any(|t| types.contains(t))
        })
        .map(|i| {
            let e = lib.entity(i);
            ChoiceOption {
                value: format!("optionalfeature:{}|{}", e.name, e.source),
                label: e.name.clone(),
                hint: e.source.clone(),
                warning: unmet_prerequisite(lib, c, &e.data, class_level, &filter, taken),
            }
        })
        .filter(|o| !taken.contains(&o.value))
        .collect();
    out.sort_by(|a, b| a.warning.is_some().cmp(&b.warning.is_some()).then(a.label.cmp(&b.label)));
    out
}

/// `options` blocks inside an entity's entries: (count, refs as entity ids).
fn option_blocks(v: &Json, out: &mut Vec<(usize, Vec<String>)>) {
    match v {
        Json::Array(a) => a.iter().for_each(|x| option_blocks(x, out)),
        Json::Object(o) => {
            if o.get("type").and_then(Json::as_str) == Some("options") {
                let count = o.get("count").and_then(Json::as_u64).unwrap_or(1) as usize;
                let refs = o
                    .get("entries")
                    .and_then(Json::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| {
                        let (field, prefix) = match e.get("type").and_then(Json::as_str)? {
                            "refClassFeature" => ("classFeature", "classFeature"),
                            "refSubclassFeature" => ("subclassFeature", "subclassFeature"),
                            "refOptionalfeature" => ("optionalfeature", "optionalfeature"),
                            _ => return None,
                        };
                        Some(format!("{prefix}:{}", e.get(field)?.as_str()?))
                    })
                    .collect::<Vec<_>>();
                if !refs.is_empty() {
                    out.push((count, refs));
                }
                return;
            }
            if let Some(e) = o.get("entries") {
                option_blocks(e, out);
            }
        }
        _ => {}
    }
}

/// Features of a class (or subclass) gained exactly at `level`.
fn features_at(lib: &Library, owner: &Json, field: &str, kind: Kind, level_part: usize, level: i64) -> Vec<EntityId> {
    owner
        .get(field)
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| f.as_str().or_else(|| f.get(if kind == Kind::ClassFeature { "classFeature" } else { "subclassFeature" })?.as_str()))
        .filter(|r| r.split('|').nth(level_part).and_then(|l| l.parse::<i64>().ok()) == Some(level))
        .filter_map(|r| lib.resolve_feature_ref(kind, r))
        .collect()
}

/// The choices due when `class` reaches `level` (with `subclass`, if any).
/// `skills` are the character's skill proficiencies at that point.
pub fn level_choices(
    lib: &Library,
    c: &Character,
    class: EntityId,
    subclass: Option<EntityId>,
    level: i64,
    filter: &SourceFilter,
    skills: &[String],
) -> Vec<Choice> {
    let taken = all_picks(c);
    let mut out = Vec::new();
    let owners: Vec<&Json> = std::iter::once(&lib.entity(class).data).chain(subclass.map(|s| &lib.entity(s).data)).collect();

    // Feats by progression (Epic Boons are handled by the level-up perk).
    for owner in &owners {
        for p in owner.get("featProgression").and_then(Json::as_array).into_iter().flatten() {
            let categories: Vec<String> = strs(p.get("category")).into_iter().filter(|c| c != "EB").collect();
            if categories.is_empty() {
                continue;
            }
            let prog = p.get("progression").unwrap_or(&Json::Null);
            let new = progression_total(prog, level) - progression_total(prog, level - 1);
            if new > 0 {
                out.push(Choice {
                    key: p.get("name").and_then(Json::as_str).unwrap_or("Feat").to_string(),
                    kind: ChoiceKind::Feat,
                    count: new as usize,
                    options: feats_in(lib, &categories, filter, &taken),
                });
            }
        }
    }

    // Optional features by progression.
    let mut has_ofp = false;
    for owner in &owners {
        for p in owner.get("optionalfeatureProgression").and_then(Json::as_array).into_iter().flatten() {
            has_ofp = true;
            let prog = p.get("progression").unwrap_or(&Json::Null);
            let new = progression_total(prog, level) - progression_total(prog, level - 1);
            if new > 0 {
                out.push(Choice {
                    key: p.get("name").and_then(Json::as_str).unwrap_or("Options").to_string(),
                    kind: ChoiceKind::OptionalFeature,
                    count: new as usize,
                    options: optional_features(lib, c, &strs(p.get("featureType")), filter, &taken, level),
                });
            }
        }
    }

    // Features gained now: option blocks and Expertise.
    let mut gained = features_at(lib, owners[0], "classFeatures", Kind::ClassFeature, 3, level);
    if let Some(s) = subclass {
        gained.extend(features_at(lib, &lib.entity(s).data, "subclassFeatures", Kind::SubclassFeature, 5, level));
    }
    let mastery_column = class_columns(&lib.entity(class).data).into_iter().find(|c| c.var.ends_with(".weapon_mastery"));
    for f in gained {
        let e = lib.entity(f);
        // Classes without a mastery column (2024 Paladin, Ranger, Rogue)
        // state "two kinds of weapons" in the feature text.
        if e.name == "Weapon Mastery" && mastery_column.is_none() && e.kind == Kind::ClassFeature {
            out.push(Choice {
                key: "Weapon Mastery".into(),
                kind: ChoiceKind::WeaponMastery,
                count: 2,
                options: mastery_weapons(lib, filter, &taken),
            });
        }
        let mut blocks = Vec::new();
        option_blocks(e.data.get("entries").unwrap_or(&Json::Null), &mut blocks);
        for (count, refs) in blocks {
            // Lists of optional features are governed by a progression instead.
            if has_ofp && refs.iter().all(|r| r.starts_with("optionalfeature:")) {
                continue;
            }
            let options: Vec<ChoiceOption> = refs
                .iter()
                .filter(|r| !taken.contains(*r))
                .filter_map(|r| {
                    let id = resolve_id(lib, r)?;
                    Some(ChoiceOption {
                        value: r.clone(),
                        label: lib.entity(id).name.clone(),
                        hint: lib.entity(id).kind.label().to_string(),
                        warning: None,
                    })
                })
                .collect();
            if !options.is_empty() {
                out.push(Choice { key: e.name.clone(), kind: ChoiceKind::SubFeature, count, options });
            }
        }
        if e.name == "Expertise" {
            let expert: BTreeSet<String> = c
                .levels
                .iter()
                .flat_map(|l| l.choices.picks.get("Expertise").into_iter().flatten().cloned())
                .chain(c.proficiencies.expertise.iter().cloned())
                .collect();
            let options = skills
                .iter()
                .filter(|s| !expert.contains(*s))
                .map(|s| ChoiceOption {
                    value: s.clone(),
                    label: SKILLS.iter().find(|(k, _, _)| k == s).map_or(s.clone(), |(_, n, _)| n.to_string()),
                    hint: String::new(),
                    warning: None,
                })
                .collect();
            out.push(Choice { key: "Expertise".into(), kind: ChoiceKind::Expertise, count: 2, options });
        }
    }

    // Weapon Mastery from the class table.
    if let Some(col) = mastery_column {
        let at = |l: i64| if l < 1 { 0 } else { matches_num(&col.at(l as usize)) };
        let new = at(level) - at(level - 1);
        if new > 0 {
            out.push(Choice {
                key: "Weapon Mastery".into(),
                kind: ChoiceKind::WeaponMastery,
                count: new as usize,
                options: mastery_weapons(lib, filter, &taken),
            });
        }
    }
    out
}

fn matches_num(v: &Value) -> i64 {
    match v {
        Value::Num(n) => n.floor(),
        _ => 0,
    }
}

/// Weapons a character could switch a Weapon Mastery to (not already mastered).
pub fn mastery_swap_options(lib: &Library, c: &Character, filter: &SourceFilter) -> Vec<ChoiceOption> {
    mastery_weapons(lib, filter, &all_picks(c))
}

/// Replace a mastered weapon (after a long rest, 2024): the pick that
/// resolves to `old` becomes `new`. Returns whether one was found.
pub fn swap_mastery(lib: &Library, c: &mut Character, old: EntityId, new: &str) -> bool {
    for l in &mut c.levels {
        if let Some(list) = l.choices.picks.get_mut("Weapon Mastery") {
            if let Some(v) = list.iter_mut().find(|v| resolve_id(lib, v) == Some(old)) {
                *v = new.to_string();
                return true;
            }
        }
    }
    false
}

fn mastery_weapons(lib: &Library, filter: &SourceFilter, taken: &BTreeSet<String>) -> Vec<ChoiceOption> {
    let mut out: Vec<ChoiceOption> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            e.kind == Kind::BaseItem && filter.allows(e) && e.data.get("mastery").is_some()
        })
        .map(|i| {
            let e = lib.entity(i);
            let mastery = strs(e.data.get("mastery")).first().map(|m| m.split('|').next().unwrap_or(m).to_string()).unwrap_or_default();
            ChoiceOption { value: format!("item:{}|{}", e.name, e.source), label: e.name.clone(), hint: mastery, warning: None }
        })
        .filter(|o| !taken.contains(&o.value))
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// A choice that a level still lacks picks for.
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    /// Index into `Character::levels`.
    pub level_index: usize,
    pub class_name: String,
    pub class_level: i64,
    pub choice: Choice,
    /// How many more picks are needed.
    pub missing: usize,
}

/// Choices missing on an existing character (e.g. made before dnt knew
/// about them, or edited by hand).
pub fn pending(lib: &Library, c: &Character, filter: &SourceFilter) -> Vec<Pending> {
    let mut out = Vec::new();
    let mut counts: std::collections::BTreeMap<&str, i64> = Default::default();
    for (i, l) in c.levels.iter().enumerate() {
        let n = counts.entry(&l.class).or_default();
        *n += 1;
        let Some(class) = resolve_id(lib, &l.class) else { continue };
        let subclass = c.subclasses.get(&l.class).and_then(|s| resolve_id(lib, s));
        // The character as it was when this level was taken.
        let mut before = c.clone();
        before.levels.truncate(i);
        let skills: Vec<String> = c.proficiencies.skills.iter().map(|s| slug(s)).collect();
        for choice in level_choices(lib, &before, class, subclass, *n, filter, &skills) {
            let have = l.choices.picks.get(&choice.key).map_or(0, Vec::len);
            if have < choice.count && !choice.options.is_empty() {
                out.push(Pending {
                    level_index: i,
                    class_name: lib.entity(class).name.clone(),
                    class_level: *n,
                    missing: choice.count - have,
                    choice,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progressions() {
        let arr = json!([1, 3, 3, 3, 5]);
        assert_eq!(progression_total(&arr, 1), 1);
        assert_eq!(progression_total(&arr, 2) - progression_total(&arr, 1), 2);
        assert_eq!(progression_total(&arr, 0), 0);
        let obj = json!({"3": 3, "7": 5});
        assert_eq!(progression_total(&obj, 2), 0);
        assert_eq!(progression_total(&obj, 6), 3);
        assert_eq!(progression_total(&obj, 7) - progression_total(&obj, 6), 2);
    }

    #[test]
    fn option_blocks_are_found() {
        let entries = json!(["Pick one.", {"type": "entries", "entries": [{"type": "options", "count": 1, "entries": [
            {"type": "refClassFeature", "classFeature": "Protector|Cleric|XPHB|1|XPHB"},
            {"type": "refClassFeature", "classFeature": "Thaumaturge|Cleric|XPHB|1|XPHB"}
        ]}]}]);
        let mut blocks = Vec::new();
        option_blocks(&entries, &mut blocks);
        assert_eq!(
            blocks,
            vec![(
                1,
                vec!["classFeature:Protector|Cleric|XPHB|1|XPHB".to_string(), "classFeature:Thaumaturge|Cleric|XPHB|1|XPHB".to_string()]
            )]
        );
    }

    #[test]
    fn invocation_prerequisites() {
        use crate::character::ClassSpells;
        let mut lib = Library::new();
        lib.load_json(&serde_json::json!({
            "optionalfeature": [
                {"name": "Pact of the Blade", "source": "T", "featureType": ["EI"], "entries": ["x"]},
                {"name": "Thirsting Blade", "source": "T", "featureType": ["EI"], "entries": ["x"],
                 "prerequisite": [{"level": {"level": 5, "class": {"name": "Warlock"}}, "optionalfeature": ["pact of the blade|t"]}]},
                {"name": "Agonizing Blast", "source": "T", "featureType": ["EI"], "entries": ["x"],
                 "prerequisite": [{"level": {"level": 2, "class": {"name": "Warlock"}},
                                   "spell": [{"choose": "level=0|class=Warlock", "entrySummary": "a Warlock damage cantrip"}]}]}
            ],
            "spell": [{"name": "Eldritch Blast", "source": "T", "level": 0, "entries": ["x"], "classes": {"fromClassList": [{"name": "Warlock", "source": "T"}]}}]
        }));
        let filter = SourceFilter::default();
        let check = |c: &Character, name: &str, level: i64| {
            let e = lib.get(&dnt_data::Key::new(Kind::OptionalFeature, &[name, "T"])).unwrap();
            unmet_prerequisite(&lib, c, &lib.entity(e).data, level, &filter, &all_picks(c))
        };
        let mut c = Character::default();
        assert_eq!(check(&c, "Thirsting Blade", 3).as_deref(), Some("needs level 5, Pact of the Blade"));
        c.options.push("optionalfeature:Pact of the Blade|T".into());
        assert_eq!(check(&c, "Thirsting Blade", 5), None);
        assert_eq!(check(&c, "Agonizing Blast", 2).as_deref(), Some("needs a Warlock damage cantrip"));
        c.spells
            .classes
            .insert("class:Warlock|T".into(), ClassSpells { cantrips: vec!["spell:Eldritch Blast|T".into()], ..Default::default() });
        assert_eq!(check(&c, "Agonizing Blast", 2), None);
    }

    #[test]
    fn mastery_swaps() {
        use crate::character::Level;
        let mut lib = Library::new();
        lib.load_json(&json!({"baseitem": [
            {"name": "Longsword", "source": "T", "mastery": ["Sap|T"]},
            {"name": "Greatsword", "source": "T", "mastery": ["Graze|T"]}
        ]}));
        let mut c = Character::default();
        let mut l = Level { class: "class:Fighter|T".into(), hp: Default::default(), choices: Default::default() };
        l.choices.picks.insert("Weapon Mastery".into(), vec!["item:Longsword|T".into()]);
        c.levels.push(l);
        let options = mastery_swap_options(&lib, &c, &SourceFilter::default());
        assert_eq!(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Greatsword"], "not already mastered");
        let longsword = lib.get(&dnt_data::Key::new(Kind::BaseItem, &["Longsword", "T"])).unwrap();
        assert!(swap_mastery(&lib, &mut c, longsword, "item:Greatsword|T"));
        assert_eq!(c.levels[0].choices.picks["Weapon Mastery"], ["item:Greatsword|T"]);
        assert!(!swap_mastery(&lib, &mut c, longsword, "item:Greatsword|T"));
    }
}
