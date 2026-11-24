//! Automation overlays: TOML files that attach resources, toggles and
//! effects to library entities without touching the source data.
//!
//! ```toml
//! [[feature]]
//! id = "classFeature:Blood Maledict|Blood Hunter|BH|2"
//!
//! [[feature.resource]]
//! key = "blood_maledict"
//! name = "Blood Maledict"
//! max = "scale(level('Blood Hunter'), {2: 1, 6: 2, 13: 3, 17: 4})"
//! recharge = "short_rest"
//!
//! [[feature.toggle]]
//! key = "crimson_rite"
//! name = "Crimson Rite"
//!
//! [[feature.effect]]
//! target = "attack.damage"   # every weapon attack; see attacks.rs for the groups
//! add = "class.blood_hunter.hemocraft_die"
//! when = "toggle('crimson_rite')"
//! ```
//!
//! `id` is `<type>:<reference>` using the data format's pipe syntax, e.g.
//! `feat:Alert|XPHB`, `item:Shield|XPHB`, `classFeature:Rage|Barbarian|XPHB|1`.
//!
//! A rule can grant spells: a spell, or a choice of spells (the data
//! format's filter), prepared, known, or cast free (`uses` times per long
//! rest, at will without `uses`, or paid with a resource):
//!
//! ```toml
//! [[feature]]
//! id = "classFeature:Mystic Arcanum|Warlock|XPHB|11"
//!
//! [[feature.spell]]
//! choose = "level=6|class=Warlock"
//! grant = "free"
//! uses = 1
//! ```
//!
//! A class's features are its own: rules for one apply when it comes with
//! the class it names (the Barbarian's Rage, for a Barbarian). Another
//! class that has it too (a book's class made from the Barbarian) has its
//! own rules for it, which name that class:
//!
//! ```toml
//! [[feature]]
//! id = "classFeature:Rage|Barbarian|XPHB|1"
//! class = "Shaman|WoW"
//! ```

use std::collections::HashMap;

use dnt_data::{Key, Kind, Library};
use serde::{Deserialize, Serialize};

use crate::engine::{Effect, Model, Op, StatDef};
use crate::formula::Formula;
use crate::spells::{Grant, Granted};
use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recharge {
    ShortRest,
    #[default]
    LongRest,
    Dawn,
    Manual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resource {
    pub key: String,
    pub name: String,
    pub max: Formula,
    pub recharge: Recharge,
    /// Uses regained on a short rest when `recharge` is longer, e.g. `1`
    /// for 2024 Rage or Channel Divinity.
    pub short_rest_regain: Option<Formula>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toggle {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeatureOverlay {
    /// The `id` as written in the file.
    pub id: String,
    /// Candidate keys: an `item:` id may be a magic item or a base item.
    pub keys: Vec<Key>,
    /// Display name used as the origin of effects; defaults to the entity name.
    pub name: String,
    /// For a class feature: the class (`Shaman|WoW`) these rules are for,
    /// when it is another than the one the feature names.
    pub class: Option<String>,
    pub resources: Vec<Resource>,
    pub toggles: Vec<Toggle>,
    pub effects: Vec<Effect>,
    /// Spells it grants.
    pub spells: Vec<Granted>,
}

/// A problem found while loading an overlay. Loading continues past these.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// e.g. `feature[2].effect[0].add`
    pub location: String,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

// Raw TOML shapes.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    feature: Vec<RawFeature>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFeature {
    id: String,
    name: Option<String>,
    class: Option<String>,
    #[serde(default)]
    resource: Vec<RawResource>,
    #[serde(default)]
    toggle: Vec<RawToggle>,
    #[serde(default)]
    effect: Vec<RawEffect>,
    #[serde(default)]
    spell: Vec<RawSpell>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpell {
    spell: Option<String>,
    choose: Option<String>,
    count: Option<usize>,
    grant: String,
    uses: Option<i64>,
    resource: Option<String>,
    cost: Option<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawResource {
    key: String,
    name: String,
    max: String,
    recharge: Recharge,
    short_rest_regain: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawToggle {
    key: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEffect {
    target: String,
    add: Option<String>,
    set: Option<String>,
    max: Option<String>,
    min: Option<String>,
    mul: Option<String>,
    when: Option<String>,
    origin: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Overlay {
    pub features: Vec<FeatureOverlay>,
}

fn valid_key(k: &str) -> bool {
    let mut parts = k.split('.');
    parts.all(|p| {
        let mut c = p.chars();
        c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_') && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    })
}

/// A `[[feature.spell]]`: what it grants, and how.
fn spell_grant(g: RawSpell) -> Result<Granted, String> {
    let free_only = || {
        if g.uses.is_some() || g.resource.is_some() || g.cost.is_some() {
            return Err(format!("`uses`, `resource` and `cost` are for `grant = \"free\"`, not `{}`", g.grant));
        }
        Ok(())
    };
    let grant = match g.grant.as_str() {
        "prepared" => free_only().map(|_| Grant::Prepared)?,
        "known" => free_only().map(|_| Grant::Known)?,
        "free" => match (g.uses, g.resource, g.cost) {
            (Some(_), Some(_), _) => return Err("a free cast has `uses` or a `resource`, not both".into()),
            (_, None, Some(_)) => return Err("`cost` is what a cast takes of a `resource`".into()),
            (Some(n), None, None) if n < 1 => return Err("`uses` is a number of casts, 1 or more".into()),
            (uses, None, None) => Grant::Innate(uses),
            (None, Some(name), cost) => Grant::Resource { name, cost: cost.unwrap_or(1).max(1) },
        },
        other => return Err(format!("`grant = \"{other}\"`: expected `prepared`, `known` or `free`")),
    };
    match (g.spell, g.choose) {
        (Some(id), None) => {
            let keys = parse_id(&id)?;
            if !keys.iter().any(|k| k.kind == Kind::Spell) {
                return Err(format!("`{id}` is not a spell: `spell:<name>|<source>`"));
            }
            Ok(Granted::Spell { id: id.to_lowercase(), grant })
        }
        (None, Some(filter)) => Ok(Granted::Choice { filter, count: g.count.unwrap_or(1).max(1), grant }),
        _ => Err("a spell grant has one of `spell` (a spell) or `choose` (a filter, e.g. `level=0|class=Wizard`)".into()),
    }
}

/// Parse an entity id such as `feat:Alert|XPHB` into candidate library keys.
/// The prefix is a data type or tag name; tag names that cover several
/// kinds (`item`, `race`) yield one candidate per kind.
pub fn parse_id(id: &str) -> Result<Vec<Key>, String> {
    let (prefix, reference) = id.split_once(':').ok_or_else(|| "expected `<type>:<reference>`, e.g. `feat:Alert|XPHB`".to_string())?;
    let kinds: Vec<Kind> = match Kind::for_tag(prefix) {
        [] => Kind::from_json_key(prefix).into_iter().collect(),
        many => many.to_vec(),
    };
    if kinds.is_empty() {
        return Err(format!("unknown type `{prefix}`"));
    }
    let parts: Vec<&str> = reference.split('|').collect();
    let keys: Vec<Key> = kinds.iter().filter_map(|&k| Key::from_ref(k, prefix, &parts).map(|(key, _)| key)).collect();
    if keys.is_empty() {
        return Err(format!("incomplete reference for {prefix}"));
    }
    Ok(keys)
}

/// Resolve an entity id to the library entity it names, if any.
pub fn resolve_id(lib: &Library, id: &str) -> Option<dnt_data::EntityId> {
    parse_id(id).ok()?.iter().find_map(|k| lib.get(k))
}

impl Overlay {
    /// Parse overlay TOML. Structural (TOML) errors fail the whole file;
    /// invalid formulas, ids or keys are reported and the item skipped.
    pub fn parse(text: &str) -> Result<(Overlay, Vec<Diagnostic>), String> {
        let raw: RawFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut diags = Vec::new();
        let mut features = Vec::new();
        for (fi, f) in raw.feature.into_iter().enumerate() {
            let at = |s: &str| format!("feature[{fi}]{s}");
            let keys = match parse_id(&f.id) {
                Ok(k) => k,
                Err(message) => {
                    diags.push(Diagnostic { location: at(".id"), message });
                    continue;
                }
            };
            let class = match f.class {
                Some(c) if c.split('|').count() != 2 || c.split('|').any(|p| p.trim().is_empty()) => {
                    diags.push(Diagnostic {
                        location: at(".class"),
                        message: format!("`{c}` is not a class: `<name>|<source>`, e.g. `Shaman|WoW`"),
                    });
                    continue;
                }
                Some(_) if !keys.iter().any(|k| matches!(k.kind, Kind::ClassFeature | Kind::SubclassFeature)) => {
                    diags.push(Diagnostic { location: at(".class"), message: "only a class's features have a `class`".into() });
                    continue;
                }
                c => c,
            };
            let name =
                f.name.clone().unwrap_or_else(|| f.id.split_once(':').map_or("", |(_, r)| r).split('|').next().unwrap_or("").to_string());
            let formula = |src: &str, loc: String, diags: &mut Vec<Diagnostic>| match Formula::parse(src) {
                Ok(f) => Some(f),
                Err(e) => {
                    diags.push(Diagnostic { location: loc, message: e.display(src) });
                    None
                }
            };

            let mut resources = Vec::new();
            for (ri, r) in f.resource.into_iter().enumerate() {
                if !valid_key(&r.key) {
                    diags.push(Diagnostic {
                        location: at(&format!(".resource[{ri}].key")),
                        message: format!("`{}` is not a valid key (letters, digits, `_`)", r.key),
                    });
                    continue;
                }
                let max = formula(&r.max, at(&format!(".resource[{ri}].max")), &mut diags);
                let regain = match &r.short_rest_regain {
                    Some(src) => match formula(src, at(&format!(".resource[{ri}].short_rest_regain")), &mut diags) {
                        Some(f) => Some(f),
                        None => continue,
                    },
                    None => None,
                };
                if let Some(max) = max {
                    resources.push(Resource { key: r.key, name: r.name, max, recharge: r.recharge, short_rest_regain: regain });
                }
            }

            let mut toggles = Vec::new();
            for (ti, t) in f.toggle.into_iter().enumerate() {
                if valid_key(&t.key) {
                    toggles.push(Toggle { key: t.key, name: t.name });
                } else {
                    diags.push(Diagnostic {
                        location: at(&format!(".toggle[{ti}].key")),
                        message: format!("`{}` is not a valid key", t.key),
                    });
                }
            }

            let mut effects = Vec::new();
            for (ei, e) in f.effect.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".effect[{ei}]{field}"));
                if !valid_key(&e.target) {
                    diags.push(Diagnostic { location: loc(".target"), message: format!("`{}` is not a valid stat key", e.target) });
                    continue;
                }
                let ops: Vec<(Op, &str, &String)> = [
                    (Op::Add, "add", &e.add),
                    (Op::Set, "set", &e.set),
                    (Op::Max, "max", &e.max),
                    (Op::Min, "min", &e.min),
                    (Op::Mul, "mul", &e.mul),
                ]
                .into_iter()
                .filter_map(|(op, n, v)| v.as_ref().map(|v| (op, n, v)))
                .collect();
                let [(op, field, src)] = ops.as_slice() else {
                    diags.push(Diagnostic {
                        location: loc(""),
                        message: "an effect needs exactly one of `add`, `set`, `max`, `min`, `mul`".into(),
                    });
                    continue;
                };
                let value = formula(src, loc(&format!(".{field}")), &mut diags);
                let when = match &e.when {
                    Some(w) => match formula(w, loc(".when"), &mut diags) {
                        Some(f) => Some(f),
                        None => continue,
                    },
                    None => None,
                };
                let Some(value) = value else { continue };
                effects.push(Effect { target: e.target, op: *op, value, when, origin: e.origin.unwrap_or_else(|| name.clone()) });
            }

            let mut spells = Vec::new();
            for (si, g) in f.spell.into_iter().enumerate() {
                match spell_grant(g) {
                    Ok(g) => spells.push(g),
                    Err(message) => diags.push(Diagnostic { location: at(&format!(".spell[{si}]")), message }),
                }
            }

            features.push(FeatureOverlay { id: f.id, keys, name, class, resources, toggles, effects, spells });
        }
        Ok((Overlay { features }, diags))
    }

    /// Feature ids that do not exist in the library (likely typos, or a
    /// homebrew source that is not loaded).
    pub fn unresolved<'a>(&'a self, lib: &Library) -> Vec<&'a str> {
        self.features.iter().filter(|f| !f.keys.iter().any(|k| lib.get(k).is_some())).map(|f| f.id.as_str()).collect()
    }
}

/// All loaded overlays, indexed by the entity they automate.
#[derive(Debug, Default, Clone)]
pub struct OverlaySet {
    by_key: HashMap<Key, Vec<FeatureOverlay>>,
}

impl OverlaySet {
    pub fn add(&mut self, overlay: Overlay) {
        for f in overlay.features {
            for k in &f.keys {
                self.by_key.entry(k.clone()).or_default().push(f.clone());
            }
        }
    }

    /// Add an overlay whose features replace any already loaded for the
    /// same entities and class (a user's version of a built-in overlay).
    pub fn replace(&mut self, overlay: Overlay) {
        for f in &overlay.features {
            for k in &f.keys {
                if let Some(list) = self.by_key.get_mut(k) {
                    list.retain(|o| !same_class(&o.class, &f.class));
                }
            }
        }
        self.add(overlay);
    }

    /// Remove the overlays for an entity (not those for another class).
    pub fn remove(&mut self, key: &Key) {
        if let Some(list) = self.by_key.get_mut(key) {
            list.retain(|o| o.class.is_some());
        }
    }

    /// Every overlay for an entity, whichever class each is for.
    pub fn for_entity(&self, key: &Key) -> &[FeatureOverlay] {
        self.by_key.get(key).map(Vec::as_slice).unwrap_or_default()
    }

    /// The overlays for a feature as a character has it: a class's
    /// feature (`class`: the feature's own class, `through`: the class it
    /// came with) has the rules for the class it came with: those naming
    /// it, or its own class's when it is that one. Other features have
    /// their rules.
    pub fn for_feature(&self, key: &Key, own: Option<&str>, through: Option<&str>) -> Vec<&FeatureOverlay> {
        let own_class = match (own, through) {
            (Some(own), Some(through)) => own.eq_ignore_ascii_case(through),
            _ => true,
        };
        self.for_entity(key)
            .iter()
            .filter(|o| match &o.class {
                None => own_class,
                Some(c) => through.is_some_and(|t| c.eq_ignore_ascii_case(t)),
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.by_key.values().flatten().map(FeatureOverlay::ident).collect::<std::collections::HashSet<_>>().len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty()
    }
}

/// A file's `[[feature]]` blocks, each with its sub-tables: the text
/// before the first one and the comments that head the next one are left
/// out.
pub fn blocks(text: &str) -> Vec<String> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    for line in text.lines() {
        if line.trim() == "[[feature]]" {
            out.push(Vec::new());
        }
        if let Some(b) = out.last_mut() {
            b.push(line);
        }
    }
    out.into_iter()
        .map(|b| {
            let end = b.iter().rposition(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).map_or(0, |i| i + 1);
            b[..end].join("\n") + "\n"
        })
        .collect()
}

/// A commented starting point for a feature's overlay.
pub fn template(id: &str, name: &str) -> String {
    format!(
        r#"# Rules for {name}. Formulas can use: level, prof, str … cha, mod.str …,
# class.<class>.level, class table columns (class.rogue.sneak_attack),
# level('Fighter'), scale(level('Fighter'), {{3: 4, 7: 5}}), if(cond, a, b),
# max(a, b), dice like 1d8, toggle('<key>').

[[feature]]
id = "{id}"

# A use-limited ability:
# [[feature.resource]]
# key = "my_feature"
# name = "{name}"
# max = "prof"
# recharge = "long_rest"        # or "short_rest"
# short_rest_regain = "1"       # optional

# An on/off state for effects to depend on:
# [[feature.toggle]]
# key = "my_state"
# name = "{name} active"

# A change to a number. Targets: ac, speed, hp.max, initiative, save.dex,
# skill.stealth, attack.bonus, attack.damage, attack.melee.damage,
# attack.ranged.bonus, attack.heavy.damage, attack.unarmed.damage,
# rider.<key> (extra damage on a hit), resource.<key>.max …
# [[feature.effect]]
# target = "ac"
# add = "1"                     # or set / max / min / mul
# when = "toggle('my_state')"   # optional
# origin = "{name}: when it applies"   # optional
"#
    )
}

fn same_class(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// A rule's identity: its feature's id, and the class it is for when it
/// names one (`classFeature:Rage|Barbarian|XPHB|1@Shaman|WoW`).
pub fn ident(id: &str, class: Option<&str>) -> String {
    match class {
        Some(c) => format!("{id}@{c}"),
        None => id.to_string(),
    }
}

impl FeatureOverlay {
    pub fn ident(&self) -> String {
        ident(&self.id, self.class.as_deref())
    }

    /// Add this feature's automation to a character model. Resources become
    /// `resource.<key>.max` stats; toggles default to off.
    pub fn apply(&self, model: &mut Model) {
        for r in &self.resources {
            model.stats.push(StatDef { key: format!("resource.{}.max", r.key), base: r.max.clone(), origin: r.name.clone() });
            if let Some(regain) = &r.short_rest_regain {
                model.stats.push(StatDef {
                    key: format!("resource.{}.short_rest", r.key),
                    base: regain.clone(),
                    origin: format!("{} (short rest)", r.name),
                });
            }
        }
        for t in &self.toggles {
            model.inputs.entry(format!("toggle.{}", t.key)).or_insert(Value::Bool(false));
        }
        model.effects.extend(self.effects.iter().cloned());
    }
}

#[cfg(test)]
mod class_tests {
    use super::*;

    const SHAMAN_RAGE: &str = "[[feature]]\nid = \"classFeature:Rage|Barbarian|T|1\"\nclass = \"Shaman|B\"\n";

    #[test]
    fn a_rule_for_another_class_replaces_only_that_class_s() {
        let mut set = OverlaySet::default();
        set.add(Overlay::parse("[[feature]]\nid = \"classFeature:Rage|Barbarian|T|1\"\n").unwrap().0);
        set.add(Overlay::parse(SHAMAN_RAGE).unwrap().0);
        let key = parse_id("classFeature:Rage|Barbarian|T|1").unwrap().remove(0);
        assert_eq!(set.len(), 2);
        set.replace(Overlay::parse(SHAMAN_RAGE).unwrap().0);
        assert_eq!(set.for_entity(&key).len(), 2);
        set.remove(&key);
        let left: Vec<String> = set.for_entity(&key).iter().map(FeatureOverlay::ident).collect();
        assert_eq!(left, ["classFeature:Rage|Barbarian|T|1@Shaman|B"]);
    }

    #[test]
    fn a_class_is_a_name_and_a_source_and_only_for_class_features() {
        let (o, d) = Overlay::parse("[[feature]]\nid = \"classFeature:Rage|Barbarian|T|1\"\nclass = \"Shaman\"\n").unwrap();
        assert!(o.features.is_empty() && d[0].location == "feature[0].class", "{d:?}");
        let (o, d) = Overlay::parse("[[feature]]\nid = \"feat:Alert|T\"\nclass = \"Shaman|B\"\n").unwrap();
        assert!(o.features.is_empty() && d.len() == 1);
    }
}

#[cfg(test)]
mod blocks_tests {
    #[test]
    fn a_file_splits_into_its_features() {
        let text = "# Rules\n\n[[feature]]\nid = \"feat:A|T\"\n[[feature.effect]]\ntarget = \"ac\"\nadd = \"1\"\n\n# ---- B ----\n\n[[feature]]\nid = \"feat:B|T\"\n";
        assert_eq!(
            super::blocks(text),
            ["[[feature]]\nid = \"feat:A|T\"\n[[feature.effect]]\ntarget = \"ac\"\nadd = \"1\"\n", "[[feature]]\nid = \"feat:B|T\"\n"]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOOD_HUNTER: &str = r#"
[[feature]]
id = "classFeature:Blood Maledict|Blood Hunter|BH|2"

[[feature.resource]]
key = "blood_maledict"
name = "Blood Maledict"
max = "scale(level('Blood Hunter'), {2: 1, 6: 2, 13: 3, 17: 4})"
recharge = "short_rest"

[[feature]]
id = "classFeature:Crimson Rite|Blood Hunter|BH|1"

[[feature.toggle]]
key = "crimson_rite"
name = "Crimson Rite"

[[feature.effect]]
target = "attack.weapon.damage_bonus"
add = "class.blood_hunter.hemocraft_die"
when = "toggle('crimson_rite')"

[[feature]]
id = "optionalfeature:Defense|XPHB"
name = "Defense (Fighting Style)"

[[feature.effect]]
target = "ac"
add = "1"
when = "not armor.none"
"#;

    #[test]
    fn parses_and_applies() {
        let (overlay, diags) = Overlay::parse(BLOOD_HUNTER).unwrap();
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(overlay.features.len(), 3);
        assert_eq!(overlay.features[0].keys[0].path, "blood maledict|blood hunter|bh|2|bh");
        assert_eq!(overlay.features[0].name, "Blood Maledict");
        assert_eq!(overlay.features[2].effects[0].origin, "Defense (Fighting Style)");

        let mut model = Model::default();
        model.class_levels.insert("blood hunter".into(), 6);
        model.inputs.insert("class.blood_hunter.hemocraft_die".into(), crate::value::Dice::new(1, 6).into());
        for f in &overlay.features[..2] {
            f.apply(&mut model);
        }
        let ev = model.evaluate();
        assert_eq!(ev.int("resource.blood_maledict.max"), Some(2));
        // Toggle defaults to off, so the rite adds nothing yet.
        assert_eq!(ev.int("attack.weapon.damage_bonus"), Some(0));

        model.inputs.insert("toggle.crimson_rite".into(), Value::Bool(true));
        assert_eq!(model.evaluate().get("attack.weapon.damage_bonus").unwrap().to_string(), "1d6");
    }

    #[test]
    fn reports_bad_items_and_keeps_good_ones() {
        let text = r#"
[[feature]]
id = "feat:Alert|XPHB"
[[feature.effect]]
target = "initiative"
add = "prof +"
[[feature.effect]]
target = "initiative"
add = "prof"
set = "1"
[[feature.effect]]
target = "initiative"
add = "prof"

[[feature]]
id = "nonsense:Thing"
"#;
        let (overlay, diags) = Overlay::parse(text).unwrap();
        assert_eq!(overlay.features.len(), 1);
        assert_eq!(overlay.features[0].effects.len(), 1);
        let locations: Vec<&str> = diags.iter().map(|d| d.location.as_str()).collect();
        assert_eq!(locations, vec!["feature[0].effect[0].add", "feature[0].effect[1]", "feature[1].id"]);
        assert!(diags[0].message.contains("unexpected end of formula"));
    }

    #[test]
    fn grants_spells_and_names_mistakes() {
        let text = r#"
[[feature]]
id = "feat:Hedge Magic|T"
[[feature.spell]]
spell = "spell:Light|T"
grant = "known"
[[feature.spell]]
choose = "level=1|class=Wizard"
count = 2
grant = "free"
uses = 1
[[feature.spell]]
spell = "spell:Jump|T"
grant = "free"
resource = "Binding"
[[feature.spell]]
spell = "spell:Light|T"
choose = "level=0"
grant = "known"
[[feature.spell]]
spell = "feat:Light|T"
grant = "known"
[[feature.spell]]
spell = "spell:Light|T"
grant = "prepared"
uses = 1
[[feature.spell]]
spell = "spell:Light|T"
grant = "free"
uses = 1
resource = "Binding"
[[feature.spell]]
spell = "spell:Light|T"
grant = "learned"
"#;
        let (overlay, diags) = Overlay::parse(text).unwrap();
        assert_eq!(
            overlay.features[0].spells,
            vec![
                Granted::Spell { id: "spell:light|t".into(), grant: Grant::Known },
                Granted::Choice { filter: "level=1|class=Wizard".into(), count: 2, grant: Grant::Innate(Some(1)) },
                Granted::Spell { id: "spell:jump|t".into(), grant: Grant::Resource { name: "Binding".into(), cost: 1 } },
            ]
        );
        let locations: Vec<&str> = diags.iter().map(|d| d.location.as_str()).collect();
        assert_eq!(
            locations,
            ["feature[0].spell[3]", "feature[0].spell[4]", "feature[0].spell[5]", "feature[0].spell[6]", "feature[0].spell[7]"]
        );
        assert!(diags[1].message.contains("not a spell"), "{}", diags[1].message);
        assert!(diags[4].message.contains("`prepared`, `known` or `free`"));
    }

    #[test]
    fn toml_errors_fail_the_file() {
        assert!(Overlay::parse("[[feature]]\nid = ").is_err());
        assert!(Overlay::parse("[[feature]]\nid = \"feat:Alert\"\nbogus = 1").is_err());
    }
}
