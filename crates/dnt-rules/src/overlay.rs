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
//! A rule can change attacks: for the weapons a filter picks (their
//! properties: `weapon.melee`, `weapon.simple`, `weapon.light`, …), the
//! ability they use, a damage die used when better, each damage die
//! counting at least `dice_min` or rerolled at `reroll` or less, and a
//! note. With a `kind`, those weapons are a kind the player marks weapons
//! in or out of (the filter says which are, until marked):
//!
//! ```toml
//! [[feature]]
//! id = "classFeature:Martial Arts|Monk|XPHB|1"
//!
//! [[feature.attack]]
//! kind = "monk weapon"
//! weapons = "weapon.melee and (weapon.simple or (weapon.martial and weapon.light))"
//! ability = "max(mod.str, mod.dex)"
//! die = "class.monk.martial_arts"
//! ```
//!
//! and which weapons allow the off-hand attack (besides Light ones in
//! both hands), with what is needed when they do not:
//!
//! ```toml
//! [[feature.offhand]]
//! main = "main.light"
//! off = "weapon.melee and not weapon.two_handed"
//! note = "the main weapon must be Light, this one a melee weapon without Two-Handed"
//! ```
//!
//! A rule can ask for a choice when its feature is gained: abilities to
//! increase (or a feat instead), a feat, skills to gain Expertise in, or
//! weapons to master. `count` is how many the feature gives in all at a
//! class level (a formula: the class table's columns are at that level):
//!
//! ```toml
//! [[feature]]
//! id = "classFeature:Weapon Mastery|Fighter|XPHB|1"
//!
//! [[feature.choice]]
//! pick = "weapon_mastery"
//! count = "class.fighter.weapon_mastery"
//! ```
//!
//! A rule can change rolls: for the rolls a filter picks (`roll.attack`,
//! `roll.check`, `roll.save`, `roll.initiative`, and the ability rolled
//! with: `roll.str` … `roll.cha`), advantage, disadvantage, or a fail
//! without rolling (with a note saying why). Rules for a condition apply
//! while the character has it; a `summary` says what it does in a line:
//!
//! ```toml
//! [[feature]]
//! id = "condition:Restrained|XPHB"
//! summary = "Speed 0; Disadvantage on attacks and Dex saves"
//!
//! [[feature.effect]]
//! target = "speed"
//! min = "0"
//!
//! [[feature.roll]]
//! rolls = "roll.attack or (roll.save and roll.dex)"
//! mode = "disadvantage"
//! ```
//!
//! A condition with `levels` (Exhaustion) is had at a level, what each
//! level does in a line; its formulas have the level as `exhaustion`:
//!
//! ```toml
//! [[feature]]
//! id = "condition:Exhaustion|XPHB"
//! levels = ["−2 to d20 tests, −5 ft. speed", "…", "death"]
//!
//! [[feature.effect]]
//! target = "d20.bonus"   # every d20 test: saves, checks, attacks
//! add = "-2 * exhaustion"
//! ```
//!
//! A file's `[multiclass]` table has the multiclass spellcaster's slots
//! (levels 1–9, per combined caster level) and the score a class's
//! primary ability needs when its data lists no requirements:
//!
//! ```toml
//! [multiclass]
//! slots = [[2], [3], [4, 2]]   # …to level 20
//! primary_score = 13
//! ```
//!
//! A rule can give the character an action of its own (an attack, a save
//! with its DC, damage or healing dice, temporary hit points, paid with a
//! resource), defenses (damage resisted, immune or vulnerable, conditions
//! immune) while `when` holds, and proficiencies (skills, saves, tools):
//!
//! ```toml
//! [[feature.action]]
//! name = "Frost Breath"
//! save = "con"
//! dc = "8 + mod.con + prof"
//! damage = "dice(2, 6)"
//! damage_type = "cold"
//! resource = "breath"
//!
//! [[feature.defense]]
//! resist = ["cold"]
//! when = "toggle('frost_form')"
//!
//! [[feature.proficiency]]
//! skills = ["survival"]
//! tools = ["Brewer's Supplies"]
//! ```
//!
//! A choice can also pick one of a rule's own options (`pick = "option"`,
//! `options = [...]`), which formulas know as `choice.<key>`
//! (`when = "choice.runeforging == 'Blood'"`), and each option picked as
//! `choice.<key>.<option>` (true). An action with a `when` is there while
//! it holds. `all_options = true` says the options the feature lists in
//! its text are all gained, not one chosen. Senses (`sense.darkvision`,
//! `sense.blindsight`, …), other speeds (`speed.fly`, …) and the
//! creature type (`creature_type`) are stats effects can change.
//!
//! An id with a `*` part names every entity that has anything there:
//! `classFeature:Ability Score Improvement|*|*|*` is every class's.
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
use crate::standard::{ABILITIES, SKILLS};
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
    /// How it changes attacks.
    pub attacks: Vec<AttackRule>,
    /// Weapons it lets attack off hand.
    pub offhand: Vec<OffhandRule>,
    /// Choices it asks for when gained.
    pub choices: Vec<ChoiceRule>,
    /// What it does, in a line (a condition's).
    pub summary: Option<String>,
    /// A condition had at a level: what each level does, in a line.
    pub levels: Vec<String>,
    /// How it changes rolls.
    pub rolls: Vec<RollRule>,
    /// The options its text lists are all gained, not one chosen.
    pub all_options: bool,
    /// Actions of its own.
    pub actions: Vec<ActionRule>,
    pub defenses: Vec<DefenseRule>,
    pub proficiencies: Vec<ProficiencyRule>,
}

/// An action a feature gives: an attack roll or a save against its DC,
/// with damage or healing dice, or temporary hit points.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionRule {
    pub name: String,
    /// The attack bonus, for an attack roll.
    pub attack: Option<Formula>,
    /// The ability of the target's save, and its DC.
    pub save: Option<String>,
    pub dc: Option<Formula>,
    /// Dice (`dice(2, 6) + mod.con`).
    pub damage: Option<Formula>,
    pub damage_type: Option<String>,
    pub heal: Option<Formula>,
    /// Temporary hit points the character gains.
    pub temp_hp: Option<Formula>,
    /// What it costs: uses of a resource (its key).
    pub resource: Option<String>,
    pub cost: i64,
    pub note: Option<String>,
    /// There while this holds.
    pub when: Option<Formula>,
}

/// Damage and conditions a feature defends against, while `when` holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DefenseRule {
    /// Damage types (`all` for every one).
    pub resist: Vec<String>,
    pub immune: Vec<String>,
    pub vulnerable: Vec<String>,
    /// Conditions it is immune to.
    pub conditions: Vec<String>,
    pub when: Option<Formula>,
}

/// Proficiencies a feature gives.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProficiencyRule {
    /// Skill keys (`sleight_of_hand`).
    pub skills: Vec<String>,
    /// Ability keys of saves.
    pub saves: Vec<String>,
    pub tools: Vec<String>,
}

/// What a roll rule does to the rolls it picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollMode {
    Advantage,
    Disadvantage,
    /// The roll fails without rolling.
    Fail,
}

/// How a rule changes rolls: those its filter picks.
#[derive(Debug, Clone, PartialEq)]
pub struct RollRule {
    /// The rule's name.
    pub origin: String,
    pub rolls: Formula,
    pub mode: RollMode,
    /// Why, shown with the roll (a fail's).
    pub note: Option<String>,
}

/// The multiclassing tables of a `[multiclass]` table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Multiclass {
    /// Spell slots of levels 1–9 per combined caster level (from 1).
    pub slots: Vec<Vec<i64>>,
    /// The score a class's primary ability needs, when its data lists no
    /// requirements.
    pub primary_score: Option<i64>,
}

impl Multiclass {
    /// Slots for a combined caster level (none at 0; the last row above).
    pub fn slots_for(&self, caster_level: i64) -> [i64; 9] {
        let mut out = [0; 9];
        if caster_level > 0 && !self.slots.is_empty() {
            if let Some(row) = self.slots.get((caster_level as usize).min(self.slots.len()) - 1) {
                for (o, n) in out.iter_mut().zip(row) {
                    *o = *n;
                }
            }
        }
        out
    }
}

/// What a choice picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Ability scores to increase (2 points), or a feat instead.
    AbilityIncrease,
    /// A feat.
    Feat,
    /// Proficient skills to gain Expertise in.
    Expertise,
    /// Weapons to master.
    WeaponMastery,
    /// One of the rule's own options.
    Option,
}

/// A choice a feature asks for when it is gained.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceRule {
    /// What it is called, and what the character file keeps it under.
    pub key: String,
    pub pick: Pick,
    /// How many the feature gives in all at a class level (Expertise,
    /// Weapon Mastery, an option).
    pub count: Option<Formula>,
    /// The feat categories it offers (the data's: `G`, `EB`).
    pub feats: Vec<String>,
    /// What an `option` choice offers.
    pub options: Vec<String>,
}

/// How a rule changes the attacks with some weapons.
#[derive(Debug, Clone, PartialEq)]
pub struct AttackRule {
    /// The rule's name.
    pub origin: String,
    /// Which weapons (all without one).
    pub weapons: Option<Formula>,
    /// What those weapons are called, when the player marks weapons in or
    /// out (`monk weapon`).
    pub kind: Option<String>,
    /// The ability modifier for attack and damage rolls.
    pub ability: Option<Formula>,
    /// A damage die used instead of the weapon's when it is better.
    pub die: Option<Formula>,
    /// Each damage die counts at least this.
    pub dice_min: Option<i64>,
    /// Damage dice of this or less are rolled again, once.
    pub reroll: Option<i64>,
    /// Shown with the attack.
    pub note: Option<String>,
}

/// Weapons a rule lets attack off hand: the main hand's (`main.*`) and the
/// off hand's (`weapon.*`).
#[derive(Debug, Clone, PartialEq)]
pub struct OffhandRule {
    pub main: Option<Formula>,
    pub off: Option<Formula>,
    /// What it needs, shown when the weapons do not have it.
    pub note: Option<String>,
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
    multiclass: Option<RawMulticlass>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMulticlass {
    #[serde(default)]
    slots: Vec<Vec<i64>>,
    primary_score: Option<i64>,
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
    #[serde(default)]
    attack: Vec<RawAttack>,
    #[serde(default)]
    offhand: Vec<RawOffhand>,
    #[serde(default)]
    choice: Vec<RawChoice>,
    summary: Option<String>,
    #[serde(default)]
    levels: Vec<String>,
    #[serde(default)]
    roll: Vec<RawRoll>,
    #[serde(default)]
    all_options: bool,
    #[serde(default)]
    action: Vec<RawAction>,
    #[serde(default)]
    defense: Vec<RawDefense>,
    #[serde(default)]
    proficiency: Vec<RawProficiency>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAction {
    name: String,
    attack: Option<String>,
    save: Option<String>,
    dc: Option<String>,
    damage: Option<String>,
    damage_type: Option<String>,
    heal: Option<String>,
    temp_hp: Option<String>,
    resource: Option<String>,
    cost: Option<i64>,
    note: Option<String>,
    when: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDefense {
    #[serde(default)]
    resist: Vec<String>,
    #[serde(default)]
    immune: Vec<String>,
    #[serde(default)]
    vulnerable: Vec<String>,
    #[serde(default)]
    conditions: Vec<String>,
    when: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProficiency {
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    saves: Vec<String>,
    #[serde(default)]
    tools: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoll {
    rolls: String,
    mode: String,
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChoice {
    key: Option<String>,
    pick: String,
    count: Option<String>,
    #[serde(default)]
    feats: Vec<String>,
    #[serde(default)]
    options: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttack {
    weapons: Option<String>,
    kind: Option<String>,
    ability: Option<String>,
    die: Option<String>,
    dice_min: Option<i64>,
    reroll: Option<i64>,
    note: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOffhand {
    main: Option<String>,
    off: Option<String>,
    note: Option<String>,
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
    pub multiclass: Option<Multiclass>,
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

            let mut attacks = Vec::new();
            for (ai, a) in f.attack.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".attack[{ai}]{field}"));
                let mut ok = true;
                let mut opt = |src: &Option<String>, field: &str, diags: &mut Vec<Diagnostic>| match src {
                    Some(src) => {
                        let f = formula(src, loc(&format!(".{field}")), diags);
                        ok &= f.is_some();
                        f
                    }
                    None => None,
                };
                let (weapons, ability, die) =
                    (opt(&a.weapons, "weapons", &mut diags), opt(&a.ability, "ability", &mut diags), opt(&a.die, "die", &mut diags));
                if a.kind.is_some() && a.weapons.is_none() {
                    diags.push(Diagnostic { location: loc(".kind"), message: "a kind of weapon needs `weapons`: which are".into() });
                    ok = false;
                }
                if a.dice_min.is_some_and(|n| n < 2) || a.reroll.is_some_and(|n| n < 1) {
                    diags.push(Diagnostic { location: loc(""), message: "`dice_min` is 2 or more, `reroll` 1 or more".into() });
                    ok = false;
                }
                let changes =
                    [a.kind.is_some(), ability.is_some(), die.is_some(), a.dice_min.is_some(), a.reroll.is_some(), a.note.is_some()];
                if ok && !changes.contains(&true) {
                    diags.push(Diagnostic {
                        location: loc(""),
                        message: "an attack rule changes something: `kind`, `ability`, `die`, `dice_min`, `reroll` or `note`".into(),
                    });
                    ok = false;
                }
                if ok {
                    attacks.push(AttackRule {
                        origin: name.clone(),
                        weapons,
                        kind: a.kind,
                        ability,
                        die,
                        dice_min: a.dice_min,
                        reroll: a.reroll,
                        note: a.note,
                    });
                }
            }

            let mut offhand = Vec::new();
            for (oi, o) in f.offhand.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".offhand[{oi}].{field}"));
                let main = o.main.as_deref().map(|src| formula(src, loc("main"), &mut diags));
                let off = o.off.as_deref().map(|src| formula(src, loc("off"), &mut diags));
                if matches!(main, Some(None)) || matches!(off, Some(None)) {
                    continue;
                }
                offhand.push(OffhandRule { main: main.flatten(), off: off.flatten(), note: o.note });
            }

            let mut choices = Vec::new();
            for (ci, c) in f.choice.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".choice[{ci}]{field}"));
                let pick = match c.pick.as_str() {
                    "ability_increase" => Pick::AbilityIncrease,
                    "feat" => Pick::Feat,
                    "expertise" => Pick::Expertise,
                    "weapon_mastery" => Pick::WeaponMastery,
                    "option" => Pick::Option,
                    other => {
                        diags.push(Diagnostic {
                            location: loc(".pick"),
                            message: format!("`{other}`: expected `ability_increase`, `feat`, `expertise`, `weapon_mastery` or `option`"),
                        });
                        continue;
                    }
                };
                if (pick == Pick::Option) != !c.options.is_empty() {
                    let message = match pick {
                        Pick::Option => "an `option` choice lists its `options`".to_string(),
                        _ => format!("`{}` has no `options`", c.pick),
                    };
                    diags.push(Diagnostic { location: loc(".options"), message });
                    continue;
                }
                let counted = matches!(pick, Pick::Expertise | Pick::WeaponMastery);
                let count = match (&c.count, counted) {
                    (None, false) if pick == Pick::Option => Some(Formula::constant(Value::int(1))),
                    (Some(src), false) if pick == Pick::Option => match formula(src, loc(".count"), &mut diags) {
                        Some(f) => Some(f),
                        None => continue,
                    },
                    (Some(src), true) => match formula(src, loc(".count"), &mut diags) {
                        Some(f) => Some(f),
                        None => continue,
                    },
                    (None, true) => {
                        diags.push(Diagnostic { location: loc(""), message: format!("`{}` needs a `count`", c.pick) });
                        continue;
                    }
                    (Some(_), false) => {
                        diags.push(Diagnostic { location: loc(".count"), message: format!("`{}` has no `count`", c.pick) });
                        continue;
                    }
                    (None, false) => None,
                };
                if !c.feats.is_empty() && counted {
                    diags.push(Diagnostic { location: loc(".feats"), message: format!("`{}` offers no feats", c.pick) });
                    continue;
                }
                choices.push(ChoiceRule { key: c.key.unwrap_or_else(|| name.clone()), pick, count, feats: c.feats, options: c.options });
            }

            let mut rolls = Vec::new();
            for (ri, r) in f.roll.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".roll[{ri}]{field}"));
                let mode = match r.mode.as_str() {
                    "advantage" => RollMode::Advantage,
                    "disadvantage" => RollMode::Disadvantage,
                    "fail" => RollMode::Fail,
                    other => {
                        diags.push(Diagnostic {
                            location: loc(".mode"),
                            message: format!("`{other}`: expected `advantage`, `disadvantage` or `fail`"),
                        });
                        continue;
                    }
                };
                if let Some(filter) = formula(&r.rolls, loc(".rolls"), &mut diags) {
                    rolls.push(RollRule { origin: name.clone(), rolls: filter, mode, note: r.note });
                }
            }

            let mut actions = Vec::new();
            for (ai, a) in f.action.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".action[{ai}]{field}"));
                let mut ok = true;
                let mut opt = |src: &Option<String>, field: &str, diags: &mut Vec<Diagnostic>| match src {
                    Some(src) => {
                        let f = formula(src, loc(&format!(".{field}")), diags);
                        ok &= f.is_some();
                        f
                    }
                    None => None,
                };
                let attack = opt(&a.attack, "attack", &mut diags);
                let dc = opt(&a.dc, "dc", &mut diags);
                let damage = opt(&a.damage, "damage", &mut diags);
                let heal = opt(&a.heal, "heal", &mut diags);
                let temp_hp = opt(&a.temp_hp, "temp_hp", &mut diags);
                let when = opt(&a.when, "when", &mut diags);
                if a.save.is_some() != a.dc.is_some() {
                    diags.push(Diagnostic { location: loc(""), message: "a save has a `dc`, and a `dc` is a save's".into() });
                    ok = false;
                }
                if a.save.as_deref().is_some_and(|ab| !ABILITIES.iter().any(|(k, _)| *k == ab)) {
                    diags.push(Diagnostic { location: loc(".save"), message: "`save` is an ability: `str` … `cha`".into() });
                    ok = false;
                }
                if a.cost.is_some() && a.resource.is_none() {
                    diags.push(Diagnostic { location: loc(".cost"), message: "`cost` is what it takes of a `resource`".into() });
                    ok = false;
                }
                if ok && [&attack, &dc, &damage, &heal, &temp_hp].iter().all(|f| f.is_none()) {
                    diags.push(Diagnostic {
                        location: loc(""),
                        message: "an action does something: `attack`, `save`, `damage`, `heal` or `temp_hp`".into(),
                    });
                    ok = false;
                }
                if ok {
                    actions.push(ActionRule {
                        name: a.name,
                        attack,
                        save: a.save,
                        dc,
                        damage,
                        damage_type: a.damage_type,
                        heal,
                        temp_hp,
                        resource: a.resource,
                        cost: a.cost.unwrap_or(1).max(1),
                        note: a.note,
                        when,
                    });
                }
            }

            let mut defenses = Vec::new();
            for (di, d) in f.defense.into_iter().enumerate() {
                let when = match &d.when {
                    Some(w) => match formula(w, at(&format!(".defense[{di}].when")), &mut diags) {
                        Some(f) => Some(f),
                        None => continue,
                    },
                    None => None,
                };
                let lower = |v: Vec<String>| v.into_iter().map(|s| s.to_lowercase()).collect::<Vec<_>>();
                defenses.push(DefenseRule {
                    resist: lower(d.resist),
                    immune: lower(d.immune),
                    vulnerable: lower(d.vulnerable),
                    conditions: lower(d.conditions),
                    when,
                });
            }

            let mut proficiencies = Vec::new();
            for (pi, p) in f.proficiency.into_iter().enumerate() {
                let loc = |field: &str| at(&format!(".proficiency[{pi}].{field}"));
                if let Some(bad) = p.skills.iter().find(|k| !SKILLS.iter().any(|(s, _, _)| s == k)) {
                    diags.push(Diagnostic { location: loc("skills"), message: format!("`{bad}` is not a skill key (`sleight_of_hand`)") });
                    continue;
                }
                if let Some(bad) = p.saves.iter().find(|k| !ABILITIES.iter().any(|(a, _)| a == k)) {
                    diags.push(Diagnostic { location: loc("saves"), message: format!("`{bad}` is not an ability: `str` … `cha`") });
                    continue;
                }
                proficiencies.push(ProficiencyRule { skills: p.skills, saves: p.saves, tools: p.tools });
            }

            if !f.levels.is_empty() && !keys.iter().any(|k| k.kind == Kind::Condition) {
                diags.push(Diagnostic { location: at(".levels"), message: "only a condition has `levels`".into() });
                continue;
            }

            features.push(FeatureOverlay {
                id: f.id,
                keys,
                name,
                class,
                resources,
                toggles,
                effects,
                spells,
                attacks,
                offhand,
                choices,
                summary: f.summary,
                levels: f.levels,
                rolls,
                all_options: f.all_options,
                actions,
                defenses,
                proficiencies,
            });
        }
        let multiclass = match raw.multiclass {
            Some(m) if m.slots.iter().any(|row| row.len() > 9 || row.iter().any(|n| *n < 0)) => {
                diags.push(Diagnostic {
                    location: "multiclass.slots".into(),
                    message: "a row has up to 9 numbers of slots (levels 1–9), none below 0".into(),
                });
                None
            }
            Some(m) => Some(Multiclass { slots: m.slots, primary_score: m.primary_score }),
            None => None,
        };
        Ok((Overlay { features, multiclass }, diags))
    }

    /// Feature ids that do not exist in the library (likely typos, or a
    /// homebrew source that is not loaded).
    pub fn unresolved<'a>(&'a self, lib: &Library) -> Vec<&'a str> {
        let names = |k: &Key| match is_pattern(k) {
            true => lib.entities().iter().any(|e| pattern_matches(k, &e.key)),
            false => lib.get(k).is_some(),
        };
        self.features.iter().filter(|f| !f.keys.iter().any(names)).map(|f| f.id.as_str()).collect()
    }
}

/// All loaded overlays, indexed by the entity they automate.
#[derive(Debug, Default, Clone)]
pub struct OverlaySet {
    by_key: HashMap<Key, Vec<FeatureOverlay>>,
    /// Rules whose id has a `*` part (every class's Ability Score
    /// Improvement): checked against each entity.
    patterns: Vec<FeatureOverlay>,
    /// The last loaded file's `[multiclass]`.
    pub multiclass: Multiclass,
}

/// Whether a key has a `*` part.
pub fn is_pattern(k: &Key) -> bool {
    k.path.split('|').any(|p| p == "*")
}

/// Whether a key (or a pattern) is one a pattern names: each part the
/// same, or `*` in the pattern.
pub fn pattern_matches(pattern: &Key, key: &Key) -> bool {
    let (p, k): (Vec<&str>, Vec<&str>) = (pattern.path.split('|').collect(), key.path.split('|').collect());
    pattern.kind == key.kind && p.len() == k.len() && p.iter().zip(&k).all(|(p, k)| *p == "*" || p == k)
}

impl OverlaySet {
    pub fn add(&mut self, overlay: Overlay) {
        if let Some(m) = overlay.multiclass {
            self.multiclass = m;
        }
        for f in overlay.features {
            if f.keys.iter().any(is_pattern) {
                self.patterns.push(f);
                continue;
            }
            for k in &f.keys {
                self.by_key.entry(k.clone()).or_default().push(f.clone());
            }
        }
    }

    /// Add an overlay whose features replace any already loaded for the
    /// same entities and class (a user's version of a built-in overlay).
    pub fn replace(&mut self, overlay: Overlay) {
        for f in &overlay.features {
            self.patterns.retain(|o| !(o.keys == f.keys && same_class(&o.class, &f.class)));
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
        self.patterns.retain(|o| o.class.is_some() || !o.keys.contains(key));
    }

    /// Every overlay for an entity, whichever class each is for: its own,
    /// then those whose id has a `*` part that names it.
    pub fn for_entity(&self, key: &Key) -> Vec<&FeatureOverlay> {
        let own = self.by_key.get(key).into_iter().flatten();
        own.chain(self.patterns.iter().filter(|o| o.keys.iter().any(|p| pattern_matches(p, key)))).collect()
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
            .into_iter()
            .filter(|o| match &o.class {
                None => own_class,
                Some(c) => through.is_some_and(|t| c.eq_ignore_ascii_case(t)),
            })
            .collect()
    }

    /// The rules for a feature as a character has it: through `class`
    /// (the class entity it came with), for a class's or subclass's feature.
    pub fn for_had(&self, lib: &Library, feature: dnt_data::EntityId, class: Option<dnt_data::EntityId>) -> Vec<&FeatureOverlay> {
        let e = lib.entity(feature);
        let field = |k: &str| e.data.get(k).and_then(serde_json::Value::as_str).unwrap_or("");
        let own = matches!(e.kind, Kind::ClassFeature | Kind::SubclassFeature)
            .then(|| format!("{}|{}", field("className"), field("classSource")));
        let through = class.map(|c| format!("{}|{}", lib.entity(c).name, lib.entity(c).source));
        self.for_feature(&e.key, own.as_deref(), through.as_deref())
    }

    pub fn len(&self) -> usize {
        self.by_key.values().flatten().chain(&self.patterns).map(FeatureOverlay::ident).collect::<std::collections::HashSet<_>>().len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.is_empty() && self.patterns.is_empty()
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
        let left: Vec<String> = set.for_entity(&key).into_iter().map(FeatureOverlay::ident).collect();
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
    fn changes_attacks_and_names_mistakes() {
        let text = r#"
[[feature]]
id = "feat:Iron Fists|T"
[[feature.attack]]
kind = "fist weapon"
weapons = "weapon.melee and weapon.simple"
die = "1d6"
[[feature.attack]]
weapons = "weapon.heavy"
[[feature.attack]]
kind = "fist weapon"
[[feature.attack]]
weapons = "weapon.melee and"
note = "x"
[[feature.attack]]
dice_min = 1
[[feature.offhand]]
off = "weapon.melee"
note = "any melee weapon"
[[feature.offhand]]
main = "main."
"#;
        let (overlay, diags) = Overlay::parse(text).unwrap();
        let f = &overlay.features[0];
        assert_eq!(f.attacks.len(), 1);
        assert_eq!((f.attacks[0].kind.as_deref(), f.attacks[0].origin.as_str()), (Some("fist weapon"), "Iron Fists"));
        assert_eq!(f.offhand.len(), 1);
        let locations: Vec<&str> = diags.iter().map(|d| d.location.as_str()).collect();
        assert_eq!(
            locations,
            [
                "feature[0].attack[1]",
                "feature[0].attack[2].kind",
                "feature[0].attack[3].weapons",
                "feature[0].attack[4]",
                "feature[0].offhand[1].main"
            ]
        );
        assert!(diags[0].message.contains("changes something"), "{}", diags[0].message);
    }

    #[test]
    fn asks_for_choices_and_names_mistakes() {
        let text = r#"
[[feature]]
id = "classFeature:Ability Score Improvement|*|*|*"
[[feature.choice]]
pick = "ability_increase"
feats = ["G"]
[[feature.choice]]
key = "Sharp Eye"
pick = "expertise"
count = "1"
[[feature.choice]]
pick = "expertise"
[[feature.choice]]
pick = "feat"
count = "1"
[[feature.choice]]
pick = "weapon_mastery"
count = "2"
feats = ["G"]
[[feature.choice]]
pick = "skills"
"#;
        let (overlay, diags) = Overlay::parse(text).unwrap();
        let c = &overlay.features[0].choices;
        assert_eq!(c.len(), 2);
        assert_eq!(
            (c[0].key.as_str(), c[0].pick, c[0].feats.as_slice()),
            ("Ability Score Improvement", Pick::AbilityIncrease, &["G".to_string()][..])
        );
        assert_eq!((c[1].key.as_str(), c[1].pick), ("Sharp Eye", Pick::Expertise));
        let locations: Vec<&str> = diags.iter().map(|d| d.location.as_str()).collect();
        assert_eq!(
            locations,
            ["feature[0].choice[2]", "feature[0].choice[3].count", "feature[0].choice[4].feats", "feature[0].choice[5].pick"]
        );
        // A `*` part names every class's feature of that name.
        let mut set = OverlaySet::default();
        set.add(overlay);
        let fighter = Key::new(Kind::ClassFeature, &["Ability Score Improvement", "Fighter", "T", "4", "T"]);
        let other = Key::new(Kind::ClassFeature, &["Extra Attack", "Fighter", "T", "5", "T"]);
        assert_eq!((set.for_entity(&fighter).len(), set.for_entity(&other).len()), (1, 0));
    }

    #[test]
    fn toml_errors_fail_the_file() {
        assert!(Overlay::parse("[[feature]]\nid = ").is_err());
        assert!(Overlay::parse("[[feature]]\nid = \"feat:Alert\"\nbogus = 1").is_err());
    }
}
