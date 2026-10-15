//! Weapon attacks: weapon proficiency from classes, species, backgrounds
//! and feats, and an attack bonus and damage stat per carried weapon.
//!
//! Each attack `<slug>` gets stats the breakdown can explain:
//! `attack.<slug>.mod` (the ability used), `attack.<slug>.bonus` and
//! `attack.<slug>.damage` (and `.versatile` for a two-handed grip).
//! Overlays affect groups of attacks by targeting these keys, which are
//! copied onto every attack they apply to:
//!
//! | target                      | applies to                                  |
//! |-----------------------------|---------------------------------------------|
//! | `attack.bonus` / `.damage`  | every weapon attack and the unarmed strike  |
//! | `attack.melee.bonus/damage` | melee weapons and the unarmed strike        |
//! | `attack.ranged.bonus/damage`| ranged weapons (Archery)                    |
//! | `attack.one_handed.damage`  | melee weapons held in one hand (Dueling)    |
//! | `attack.strength.damage`    | attacks using Strength (Rage)               |

use dnt_data::view::item_property;
use dnt_data::{EntityId, Library};
use serde_json::Value as Json;

use crate::character::{Character, Hand};
use crate::engine::{Effect, Model, Op, StatDef};
use crate::formula::Formula;
use crate::value::Value;

/// Which weapons a character is proficient with.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeaponProficiency {
    pub simple: bool,
    pub martial: bool,
    /// Martial weapons with any of these properties (property codes, e.g.
    /// `F` finesse, `L` light), for Monks and Rogues.
    pub martial_with: Vec<String>,
    /// Specific weapons, lowercase base names (`longsword`).
    pub weapons: Vec<String>,
    /// Where each part comes from, for display: `Fighter: simple, martial`.
    pub origins: Vec<String>,
}

impl WeaponProficiency {
    /// Add one data weapon proficiency entry: `"simple"`, `"martial"`,
    /// `"{@item longsword|phb}"`, a sentence with a property filter, or an
    /// object like `{"martial": true, "longsword|phb": true}`.
    fn add(&mut self, entry: &Json) -> Vec<String> {
        let mut what = Vec::new();
        match entry {
            Json::String(s) => {
                let lower = s.to_lowercase();
                if lower == "simple" {
                    self.simple = true;
                    what.push("simple".into());
                } else if lower == "martial" {
                    self.martial = true;
                    what.push("martial".into());
                } else if let Some(props) = lower.split("property=").nth(1) {
                    // "Martial weapons that have the {@filter Finesse or Light|items|…|property=finesse;light} property"
                    let props = props.split(['}', '|']).next().unwrap_or("");
                    for p in props.split(';') {
                        if let Some(code) = property_code(p.trim()) {
                            self.martial_with.push(code.to_string());
                        }
                    }
                    what.push(format!("martial ({})", props.replace(';', " or ")));
                } else if let Some(rest) = lower.strip_prefix("{@item ") {
                    let name = rest.split(['|', '}']).next().unwrap_or("").trim().to_string();
                    what.push(name.clone());
                    self.weapons.push(name);
                } else {
                    what.push(dnt_data::tags::strip(s));
                }
            }
            Json::Object(o) => {
                for (k, v) in o {
                    if v.as_bool() != Some(true) {
                        continue;
                    }
                    match k.as_str() {
                        "simple" => self.simple = true,
                        "martial" => self.martial = true,
                        // Firearms, improvised weapons and choices are not tracked.
                        "firearms" | "improvised" | "choose" => continue,
                        name => self.weapons.push(name.split('|').next().unwrap_or(name).to_lowercase()),
                    }
                    what.push(k.split('|').next().unwrap_or(k).to_string());
                }
            }
            _ => {}
        }
        what
    }

    fn add_all(&mut self, origin: &str, list: Option<&Json>) {
        let what: Vec<String> = list.and_then(Json::as_array).into_iter().flatten().flat_map(|e| self.add(e)).collect();
        if !what.is_empty() {
            self.origins.push(format!("{origin}: {}", what.join(", ")));
        }
    }

    /// Whether this covers a weapon (its data and lowercase base name).
    pub fn covers(&self, weapon: &Weapon) -> bool {
        match weapon.category.as_deref() {
            Some("simple") if self.simple => return true,
            Some("martial") if self.martial => return true,
            Some("martial") if weapon.properties.iter().any(|p| self.martial_with.contains(p)) => return true,
            _ => {}
        }
        self.weapons.contains(&weapon.base_name)
    }
}

fn property_code(name: &str) -> Option<&'static str> {
    Some(match name {
        "finesse" => "F",
        "light" => "L",
        "heavy" => "H",
        "reach" => "R",
        "thrown" => "T",
        "two-handed" => "2H",
        "versatile" => "V",
        "ammunition" => "A",
        "loading" => "LD",
        _ => return None,
    })
}

/// A field of an item, looking through `inherits` (magic variants).
fn field<'a>(d: &'a Json, key: &str) -> Option<&'a Json> {
    d.get(key).or_else(|| d.get("inherits").and_then(|i| i.get(key)))
}

fn str_field<'a>(d: &'a Json, key: &str) -> Option<&'a str> {
    field(d, key).and_then(Json::as_str)
}

/// `"+1"` → 1.
fn bonus(d: &Json, key: &str) -> i64 {
    str_field(d, key).and_then(|b| b.trim().trim_start_matches('+').parse().ok()).unwrap_or(0)
}

/// The weapon facts attacks need, read from item data.
#[derive(Debug, Clone, PartialEq)]
pub struct Weapon {
    pub melee: bool,
    pub category: Option<String>,
    /// Property codes (`F`, `L`, `2H`, …).
    pub properties: Vec<String>,
    pub dice: Option<String>,
    pub versatile: Option<String>,
    pub damage_type: Option<String>,
    /// `"80/320"` for ranged and thrown weapons.
    pub range: Option<String>,
    /// Lowercase name of the base weapon (`longsword` for a Flame Tongue Longsword).
    pub base_name: String,
    pub mastery: Option<String>,
    /// Magic bonuses to attack and damage rolls.
    pub attack_bonus: i64,
    pub damage_bonus: i64,
}

impl Weapon {
    /// Read a weapon from item data, or `None` if the item is not a weapon.
    pub fn from_item(name: &str, d: &Json) -> Option<Weapon> {
        let code = str_field(d, "type").map(|t| t.split('|').next().unwrap_or(t));
        let is_weapon = field(d, "weapon").and_then(Json::as_bool) == Some(true)
            || field(d, "weaponCategory").is_some()
            || matches!(code, Some("M" | "R"));
        if !is_weapon || code == Some("A") {
            return None;
        }
        let properties: Vec<String> = field(d, "property")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str().or_else(|| p.get("uid").and_then(Json::as_str)))
            .map(|p| p.split('|').next().unwrap_or(p).to_string())
            .collect();
        let base_name = str_field(d, "baseItem").map(|b| b.split('|').next().unwrap_or(b).to_string()).unwrap_or_else(|| name.to_string());
        Some(Weapon {
            melee: code != Some("R"),
            category: str_field(d, "weaponCategory").map(str::to_lowercase),
            dice: str_field(d, "dmg1").map(str::to_string),
            versatile: str_field(d, "dmg2").map(str::to_string),
            damage_type: str_field(d, "dmgType").map(|t| dnt_data::view::damage_type(t).to_string()),
            range: str_field(d, "range").map(str::to_string),
            base_name: base_name.to_lowercase(),
            mastery: field(d, "mastery")
                .and_then(Json::as_array)
                .and_then(|m| m.first())
                .and_then(|m| m.as_str().or_else(|| m.get("uid").and_then(Json::as_str)))
                .map(|m| m.split('|').next().unwrap_or(m).to_string()),
            attack_bonus: bonus(d, "bonusWeapon") + bonus(d, "bonusWeaponAttack"),
            damage_bonus: bonus(d, "bonusWeapon") + bonus(d, "bonusWeaponDamage"),
            properties,
        })
    }

    pub fn has(&self, code: &str) -> bool {
        self.properties.iter().any(|p| p == code)
    }
}

/// The Monk's Martial Arts: which weapons are monk weapons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MartialArts {
    /// 2024 rules (simple melee and Light martial melee weapons) rather
    /// than 2014 (shortswords and simple melee weapons that are neither
    /// two-handed nor heavy).
    pub rules_2024: bool,
}

impl MartialArts {
    /// Whether the rules make this weapon a monk weapon.
    pub fn preset(&self, w: &Weapon) -> bool {
        if !w.melee {
            return false;
        }
        let simple = w.category.as_deref() == Some("simple");
        if self.rules_2024 {
            simple || (w.category.as_deref() == Some("martial") && w.has("L"))
        } else {
            w.base_name == "shortsword" || (simple && !w.has("2H") && !w.has("H"))
        }
    }
}

/// The Dual Wielder feat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DualWielder {
    /// 2024: the main weapon must be Light, the off-hand one any melee
    /// weapon without Two-Handed. 2014: any one-handed melee weapons.
    pub rules_2024: bool,
}

/// One attack on the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Attack {
    /// Stat key part: `attack.<slug>.bonus`.
    pub slug: String,
    pub name: String,
    pub item: Option<EntityId>,
    /// Held in hand (weapons not in hand can still be drawn).
    pub hand: Option<Hand>,
    pub melee: bool,
    pub proficient: bool,
    /// `5 ft.`, `10 ft.`, `80/320 ft.`, `5 ft. or 20/60 ft.`
    pub range: String,
    pub damage_type: String,
    pub has_versatile: bool,
    /// Property names (`Finesse`, `Light`).
    pub properties: Vec<String>,
    /// The weapon's mastery property, if the character has mastered it.
    pub mastery: Option<String>,
    /// Used as a monk weapon (Martial Arts).
    pub monk_weapon: bool,
    /// Has an off-hand attack (`attack.<slug>.offhand` damage).
    pub offhand: bool,
    /// Why something is off (no damage dice, needs attunement, …).
    pub notes: Vec<String>,
}

/// Collect weapon proficiencies: the first class's starting proficiencies,
/// later classes' multiclass gains, then species, backgrounds and feats
/// (`sources`), then the character's own list.
pub fn weapon_proficiency(lib: &Library, character: &Character, classes: &[EntityId], sources: &[EntityId]) -> WeaponProficiency {
    let mut p = WeaponProficiency::default();
    for (i, &c) in classes.iter().enumerate() {
        let e = lib.entity(c);
        let list = if i == 0 {
            e.data.get("startingProficiencies").and_then(|s| s.get("weapons"))
        } else {
            e.data.get("multiclassing").and_then(|m| m.get("proficienciesGained")).and_then(|g| g.get("weapons"))
        };
        p.add_all(&e.name, list);
    }
    for &s in sources {
        let e = lib.entity(s);
        p.add_all(&e.name, e.data.get("weaponProficiencies"));
    }
    // The character's own list: `simple`, `martial` or weapon names.
    let own: Vec<Json> = character
        .proficiencies
        .weapons
        .iter()
        .map(|w| match w.to_lowercase().as_str() {
            "simple" | "martial" => Json::String(w.to_lowercase()),
            name => serde_json::json!({ name: true }),
        })
        .collect();
    p.add_all("Character", Some(&Json::Array(own)));
    p
}

fn slug(name: &str) -> String {
    let mut s: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    s.trim_matches('_').to_string()
}

fn effect(target: &str, value: &str, origin: &str) -> Effect {
    Effect {
        target: target.to_string(),
        op: Op::Add,
        value: Formula::parse(value).unwrap_or_else(|_| Formula::constant(Value::int(0))),
        when: None,
        origin: origin.to_string(),
    }
}

/// Add the attacks' stats to the model. Call after overlays are applied:
/// effects on the group targets are copied onto each attack.
pub fn add_attacks(
    model: &mut Model,
    lib: &Library,
    character: &Character,
    prof: &WeaponProficiency,
    mastered: &[EntityId],
    martial_arts: Option<MartialArts>,
    dual_wielder: Option<DualWielder>,
) -> Vec<Attack> {
    let mut attacks = Vec::new();
    let mut seen = Vec::new();
    // Two-Weapon Fighting sets this to add the ability modifier off hand.
    model.stats.push(StatDef::new("attack.offhand.full_mod", "0", "off-hand attacks add no ability modifier"));
    let weapon_of = |line: &crate::character::InventoryItem| {
        let id = line.item.as_deref().and_then(|i| crate::overlay::resolve_id(lib, i))?;
        let e = lib.entity(id);
        Some((id, Weapon::from_item(&e.name.to_lowercase(), &e.data)?))
    };
    // Is a weapon (not a shield) held in the off hand?
    let off_weapon = character.inventory.iter().any(|l| l.hand.is_some_and(Hand::uses_off) && weapon_of(l).is_some());
    let main_weapon = character.inventory.iter().find(|l| l.hand.is_some_and(Hand::uses_main)).and_then(&weapon_of);
    let main_light = main_weapon.as_ref().is_some_and(|(_, w)| w.has("L"));
    // Separate melee weapons in each hand (for the 2014 Dual Wielder's AC).
    let off_melee = character.inventory.iter().any(|l| l.hand.is_some_and(Hand::uses_off) && weapon_of(l).is_some_and(|(_, w)| w.melee));
    let two_melee = main_weapon.as_ref().is_some_and(|(_, w)| w.melee) && off_melee;
    model.inputs.insert("hands.two_melee".into(), Value::Bool(two_melee));
    // What is in hand first (main, then off), then the rest of what is carried.
    let mut lines: Vec<_> = character.inventory.iter().collect();
    lines.sort_by_key(|l| match l.hand {
        Some(Hand::Main | Hand::Both | Hand::Pair) => 0,
        Some(Hand::Off) => 1,
        None => 2,
    });
    for line in lines {
        let Some((id, w)) = weapon_of(line) else { continue };
        let e = lib.entity(id);
        let name = line.name.clone().unwrap_or_else(|| e.name.clone());
        if seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        let mut s = slug(&name);
        while attacks.iter().any(|a: &Attack| a.slug == s) {
            s.push('_');
        }
        let attuned_ok = !crate::equipment::requires_attunement(&e.data) || line.attuned;
        let mastery = w.mastery.clone().filter(|_| {
            mastered.iter().any(|&m| lib.entity(m).name.eq_ignore_ascii_case(&w.base_name) || m == id)
        });
        let monk = martial_arts.is_some_and(|m| line.monk_weapon.unwrap_or_else(|| m.preset(&w)));
        // Dueling: a one-handed melee weapon and no other weapon in hand.
        let dueling = line.hand == Some(Hand::Main) && !off_weapon;
        let held = Held { hand: line.hand, dueling };
        let mut a = weapon_attack(model, &s, &name, Some(id), held, &w, prof, attuned_ok, mastery, monk);
        if martial_arts.is_none() && line.monk_weapon == Some(true) {
            a.notes.push("marked as a monk weapon, but no Martial Arts".into());
        }
        // The off-hand extra attack: Light weapons, or Dual Wielder.
        if a.offhand {
            let one_handed_melee = w.melee && !w.has("2H");
            let (ok, why) = match dual_wielder {
                None => (w.has("L") && main_light, "the off-hand attack needs Light weapons in both hands (or Dual Wielder)"),
                Some(d) if d.rules_2024 => (
                    one_handed_melee && main_light,
                    "Dual Wielder: the main weapon must be Light, this one a melee weapon without Two-Handed",
                ),
                Some(_) => (
                    one_handed_melee && main_weapon.as_ref().is_some_and(|(_, m)| m.melee && !m.has("2H")),
                    "Dual Wielder: both weapons must be one-handed melee weapons",
                ),
            };
            if !ok {
                a.notes.push(why.into());
            }
        }
        attacks.push(a);
    }
    attacks.push(unarmed(model));
    attacks
}

/// How a weapon is held.
#[derive(Clone, Copy)]
struct Held {
    hand: Option<Hand>,
    /// One-handed with no other weapon in hand.
    dueling: bool,
}

#[allow(clippy::too_many_arguments)]
fn weapon_attack(
    model: &mut Model,
    s: &str,
    name: &str,
    item: Option<EntityId>,
    held: Held,
    w: &Weapon,
    prof: &WeaponProficiency,
    attuned_ok: bool,
    mastery: Option<String>,
    monk: bool,
) -> Attack {
    let key = |part: &str| format!("attack.{s}.{part}");
    let finesse = w.has("F");
    // Finesse and monk weapons use the better of Strength and Dexterity.
    let flexible = finesse || monk;
    let (ability, label) = match (monk, finesse, w.melee) {
        (true, _, _) => ("max(mod.str, mod.dex)", "Strength or Dexterity (Martial Arts)"),
        (_, true, _) => ("max(mod.str, mod.dex)", "Strength or Dexterity (finesse)"),
        (_, false, true) => ("mod.str", "Strength"),
        (_, false, false) => ("mod.dex", "Dexterity"),
    };
    model.stats.push(StatDef::new(&key("mod"), ability, label));
    let proficient = prof.covers(w);
    let mut notes = Vec::new();

    // Attack bonus: ability + proficiency + magic + features.
    model.stats.push(StatDef::new(&key("bonus"), &key("mod"), label));
    if proficient {
        let what = w.category.as_deref().map(|c| format!("proficiency ({c} weapons)")).unwrap_or_else(|| "proficiency".into());
        model.effects.push(effect(&key("bonus"), "prof", &what));
    } else {
        notes.push("not proficient".into());
    }
    let magic = (w.attack_bonus, w.damage_bonus);
    if !attuned_ok && (magic.0 != 0 || magic.1 != 0) {
        notes.push("magic bonus needs attunement".into());
    }
    if attuned_ok && magic.0 != 0 {
        model.effects.push(effect(&key("bonus"), &magic.0.to_string(), name));
    }

    // Damage: dice + ability + magic + features.
    let dice = w.dice.clone().unwrap_or_else(|| {
        notes.push("no damage dice in the item data".into());
        "0".into()
    });
    let mut groups = vec!["attack"];
    groups.push(if w.melee { "attack.melee" } else { "attack.ranged" });
    let mut damage_groups = groups.clone();
    if w.melee && !w.has("2H") && held.dueling {
        damage_groups.push("attack.one_handed");
    }
    for (stat, dice, one_handed) in [(key("damage"), Some(dice), true), (key("versatile"), w.versatile.clone(), false)] {
        let Some(dice) = dice else { continue };
        let origin = format!("{name} ({})", if one_handed { "damage dice" } else { "two-handed" });
        let (base, origin) = damage_dice(model, &dice, monk, origin);
        model.stats.push(StatDef { key: stat.clone(), base, origin });
        model.effects.push(effect(&stat, &key("mod"), label));
        if attuned_ok && magic.1 != 0 {
            model.effects.push(effect(&stat, &magic.1.to_string(), name));
        }
        let groups: Vec<&str> = damage_groups.iter().copied().filter(|g| one_handed || *g != "attack.one_handed").collect();
        copy_group_effects(model, &groups, "damage", &stat);
        // Strength-based attacks (Rage): melee weapons, and finesse ones
        // when Strength is the better ability.
        match (w.melee, flexible) {
            (true, false) => copy_group_effects(model, &["attack.strength"], "damage", &stat),
            (_, true) => copy_group_effects_when(model, &["attack.strength"], "damage", &stat, Some("mod.str >= mod.dex")),
            _ => {}
        }
    }
    copy_group_effects(model, &groups, "bonus", &key("bonus"));

    // Off hand (the Light property's extra attack): no ability modifier
    // to damage unless it is negative, or with Two-Weapon Fighting.
    let offhand = matches!(held.hand, Some(Hand::Off | Hand::Pair));
    if offhand {
        let stat = key("offhand");
        let (base, origin) = damage_dice(model, w.dice.as_deref().unwrap_or("0"), monk, format!("{name} (off hand)"));
        model.stats.push(StatDef { key: stat.clone(), base, origin });
        let m = key("mod");
        model.effects.push(effect(
            &stat,
            &format!("if(attack.offhand.full_mod > 0, {m}, min({m}, 0))"),
            "ability (only a penalty off hand, unless Two-Weapon Fighting)",
        ));
        if attuned_ok && magic.1 != 0 {
            model.effects.push(effect(&stat, &magic.1.to_string(), name));
        }
        copy_group_effects(model, &groups, "damage", &stat);
        if w.melee {
            copy_group_effects_when(model, &["attack.strength"], "damage", &stat, flexible.then_some("mod.str >= mod.dex"));
        }
    }

    let reach = if w.has("R") { "10 ft." } else { "5 ft." };
    let range = match (w.melee, &w.range) {
        (true, Some(r)) if w.has("T") => format!("{reach} or {r} ft."),
        (true, _) => reach.to_string(),
        (false, Some(r)) => format!("{r} ft."),
        (false, None) => String::new(),
    };
    Attack {
        slug: s.to_string(),
        name: name.to_string(),
        item,
        hand: held.hand,
        melee: w.melee,
        proficient,
        range,
        damage_type: w.damage_type.clone().unwrap_or_default(),
        has_versatile: w.versatile.is_some(),
        monk_weapon: monk,
        offhand,
        properties: w.properties.iter().filter(|p| !matches!(p.as_str(), "V")).map(|p| item_property(p).to_string()).collect(),
        mastery,
        notes,
    }
}

/// A weapon's damage dice, or for a monk weapon the Martial Arts die
/// when it is better.
fn damage_dice(model: &Model, dice: &str, monk: bool, origin: String) -> (Formula, String) {
    let own = Formula::parse(dice).unwrap_or_else(|_| Formula::constant(Value::int(0)));
    if !monk {
        return (own, origin);
    }
    let average = |v: Option<Value>| match v {
        Some(Value::Dice(d)) => d.average(),
        Some(Value::Num(n)) => n.floor(),
        _ => 0,
    };
    let die = model.inputs.get("class.monk.martial_arts").cloned();
    if die.is_some() && average(die) > average(own.eval(&model.inputs).ok()) {
        return (Formula::parse("class.monk.martial_arts").unwrap(), "Martial Arts die".into());
    }
    (own, origin)
}

/// The unarmed strike everyone has: Strength + proficiency, 1 + Strength
/// bludgeoning (Martial Arts and similar overlays change `attack.unarmed.*`).
fn unarmed(model: &mut Model) -> Attack {
    model.stats.push(StatDef::new("attack.unarmed.mod", "mod.str", "Strength"));
    model.stats.push(StatDef::new("attack.unarmed.bonus", "attack.unarmed.mod", "Strength (or Martial Arts)"));
    model.effects.push(effect("attack.unarmed.bonus", "prof", "proficiency (unarmed strike)"));
    model.stats.push(StatDef::new("attack.unarmed.damage", "1 + attack.unarmed.mod", "1 + Strength"));
    copy_group_effects(model, &["attack", "attack.melee"], "bonus", "attack.unarmed.bonus");
    copy_group_effects(model, &["attack", "attack.melee", "attack.strength"], "damage", "attack.unarmed.damage");
    Attack {
        slug: "unarmed".into(),
        name: "Unarmed Strike".into(),
        item: None,
        hand: None,
        melee: true,
        proficient: true,
        range: "5 ft.".into(),
        damage_type: "bludgeoning".into(),
        has_versatile: false,
        properties: Vec::new(),
        mastery: None,
        monk_weapon: false,
        offhand: false,
        notes: Vec::new(),
    }
}

/// Copy effects aimed at `<group>.<what>` onto `target`.
fn copy_group_effects(model: &mut Model, groups: &[&str], what: &str, target: &str) {
    copy_group_effects_when(model, groups, what, target, None);
}

/// Like `copy_group_effects`, with an extra condition on each copy.
fn copy_group_effects_when(model: &mut Model, groups: &[&str], what: &str, target: &str, extra: Option<&str>) {
    let copies: Vec<Effect> = model
        .effects
        .iter()
        .filter(|e| groups.iter().any(|g| e.target == format!("{g}.{what}")))
        .map(|e| {
            let when = match (extra, &e.when) {
                (None, w) => w.clone(),
                (Some(x), None) => Formula::parse(x).ok(),
                (Some(x), Some(w)) => Formula::parse(&format!("({}) and ({x})", w.source)).ok(),
            };
            Effect {
                target: target.to_string(),
                when,
                ..e.clone()
            }
        })
        .collect();
    model.effects.extend(copies);
}

#[cfg(test)]
mod tests {
    use crate::builder::build;
    use crate::character::{Character, InventoryItem, Level};
    use crate::overlay::{Overlay, OverlaySet};
    use dnt_data::Library;
    use serde_json::json;

    fn library() -> Library {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "class": [
                {"name": "Fighter", "source": "T", "hd": {"faces": 10}, "proficiency": ["str", "con"],
                 "startingProficiencies": {"weapons": ["simple", "martial"]},
                 "multiclassing": {"proficienciesGained": {"weapons": ["martial"]}}, "classFeatures": []},
                {"name": "Rogue", "source": "T", "hd": {"faces": 8}, "proficiency": ["dex", "int"],
                 "startingProficiencies": {"weapons": ["simple",
                    "Martial weapons that have the {@filter Finesse or Light|items|type=martial weapon|property=finesse;light} property"]},
                 "classFeatures": []},
                {"name": "Wizard", "source": "T", "hd": {"faces": 6}, "proficiency": ["int", "wis"],
                 "startingProficiencies": {"weapons": ["simple"]}, "classFeatures": []}
            ],
            "baseitem": [
                {"name": "Longsword", "source": "T", "type": "M|T", "rarity": "none", "weaponCategory": "martial", "weapon": true,
                 "property": ["V|T"], "mastery": ["Sap|T"], "dmg1": "1d8", "dmgType": "S", "dmg2": "1d10"},
                {"name": "Rapier", "source": "T", "type": "M|T", "rarity": "none", "weaponCategory": "martial", "weapon": true,
                 "property": ["F|T"], "dmg1": "1d8", "dmgType": "P"},
                {"name": "Greataxe", "source": "T", "type": "M|T", "rarity": "none", "weaponCategory": "martial", "weapon": true,
                 "property": ["H|T", "2H|T"], "dmg1": "1d12", "dmgType": "S"},
                {"name": "Longbow", "source": "T", "type": "R|T", "rarity": "none", "weaponCategory": "martial", "weapon": true,
                 "property": ["A|T", "H|T", "2H|T"], "range": "150/600", "dmg1": "1d8", "dmgType": "P"},
                {"name": "Dagger", "source": "T", "type": "M|T", "rarity": "none", "weaponCategory": "simple", "weapon": true,
                 "property": ["F|T", "L|T", "T|T"], "range": "20/60", "dmg1": "1d4", "dmgType": "P"}
            ],
            "item": [
                {"name": "Longsword +1", "source": "T", "baseItem": "longsword|T", "type": "M|T", "rarity": "uncommon",
                 "weaponCategory": "martial", "property": ["V|T"], "dmg1": "1d8", "dmgType": "S", "dmg2": "1d10", "bonusWeapon": "+1"}
            ],
            "feat": [{"name": "Archery", "source": "T", "entries": ["x"]}, {"name": "Dueling", "source": "T", "entries": ["x"]},
                     {"name": "Dual Wielder", "source": "XPHB", "entries": ["x"]}, {"name": "Dual Wielder", "source": "T", "entries": ["x"]}]
        }));
        lib
    }

    fn character(classes: &[&str], items: &[&str]) -> Character {
        let mut c = Character { name: "T".into(), ..Character::default() };
        c.abilities.str = 16;
        c.abilities.dex = 14;
        c.levels = classes.iter().map(|k| Level { class: format!("class:{k}|T"), hp: Default::default(), choices: Default::default() }).collect();
        c.inventory = items
            .iter()
            .map(|i| InventoryItem { item: Some(format!("item:{i}|T")), equipped: true, ..Default::default() })
            .collect();
        c
    }

    fn overlays() -> OverlaySet {
        let (o, d) = Overlay::parse(
            r#"
[[feature]]
id = "feat:Archery|T"
[[feature.effect]]
target = "attack.ranged.bonus"
add = "2"
[[feature]]
id = "feat:Dueling|T"
[[feature.effect]]
target = "attack.one_handed.damage"
add = "2"
"#,
        )
        .unwrap();
        assert!(d.is_empty(), "{d:?}");
        let mut set = OverlaySet::default();
        set.add(o);
        set
    }

    #[test]
    fn bonus_and_damage() {
        let lib = library();
        let c = character(&["Fighter"], &["Longsword", "Rapier", "Longbow", "Dagger", "Longsword +1"]);
        let b = build(&c, &lib, &OverlaySet::default());
        let ev = b.evaluate();
        let names: Vec<&str> = b.attacks.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Longsword", "Rapier", "Longbow", "Dagger", "Longsword +1", "Unarmed Strike"]);
        // Str +3, proficiency +2.
        assert_eq!(ev.int("attack.longsword.bonus"), Some(5));
        assert_eq!(ev.get("attack.longsword.damage").unwrap().to_string(), "1d8 + 3");
        assert_eq!(ev.get("attack.longsword.versatile").unwrap().to_string(), "1d10 + 3");
        // Finesse picks the better of Str and Dex; ranged uses Dex.
        assert_eq!(ev.int("attack.rapier.bonus"), Some(5));
        assert_eq!(ev.int("attack.longbow.bonus"), Some(4));
        assert_eq!(ev.get("attack.longbow.damage").unwrap().to_string(), "1d8 + 2");
        assert_eq!(b.attacks[2].range, "150/600 ft.");
        assert_eq!(b.attacks[3].range, "5 ft. or 20/60 ft.");
        // +1 weapon.
        assert_eq!(ev.int("attack.longsword_1.bonus"), Some(6));
        assert_eq!(ev.get("attack.longsword_1.damage").unwrap().to_string(), "1d8 + 4");
        // Unarmed: Str + prof, 1 + Str.
        assert_eq!(ev.int("attack.unarmed.bonus"), Some(5));
        assert_eq!(ev.get("attack.unarmed.damage").unwrap().to_string(), "4");
    }

    #[test]
    fn proficiency_by_class() {
        let lib = library();
        let wizard = build(&character(&["Wizard"], &["Longsword", "Dagger"]), &lib, &OverlaySet::default());
        assert!(!wizard.attacks[0].proficient && wizard.attacks[1].proficient);
        assert_eq!(wizard.evaluate().int("attack.longsword.bonus"), Some(3), "no proficiency bonus");
        // Rogues: martial weapons with Finesse or Light only.
        let rogue = build(&character(&["Rogue"], &["Rapier", "Longsword"]), &lib, &OverlaySet::default());
        assert!(rogue.attacks[0].proficient && !rogue.attacks[1].proficient);
        // A Fighter multiclass grants martial weapons; a Wizard multiclass nothing new.
        let multi = build(&character(&["Wizard", "Fighter"], &["Longsword"]), &lib, &OverlaySet::default());
        assert!(multi.attacks[0].proficient, "{:?}", multi.weapon_proficiency);
        // A listed weapon.
        let mut c = character(&["Wizard"], &["Longsword"]);
        c.proficiencies.weapons.push("longsword".into());
        assert!(build(&c, &lib, &OverlaySet::default()).attacks[0].proficient);
    }

    #[test]
    fn fighting_styles_and_rage() {
        let lib = library();
        let mut c = character(&["Fighter"], &["Longsword", "Longbow", "Greataxe", "Rapier"]);
        c.feats = vec!["feat:Archery|T".into(), "feat:Dueling|T".into()];
        c.inventory[0].hand = Some(crate::character::Hand::Main);
        let ev = build(&c, &lib, &overlays()).evaluate();
        assert_eq!(ev.int("attack.longbow.bonus"), Some(6), "Archery");
        assert_eq!(ev.int("attack.longsword.bonus"), Some(5), "Archery is ranged only");
        assert_eq!(ev.get("attack.longsword.damage").unwrap().to_string(), "1d8 + 5", "Dueling");
        assert_eq!(ev.get("attack.longsword.versatile").unwrap().to_string(), "1d10 + 3", "not two-handed");
        assert_eq!(ev.get("attack.greataxe.damage").unwrap().to_string(), "1d12 + 3");

    }

    #[test]
    fn dual_wielding() {
        use crate::character::Hand;
        let lib = library();
        let held = |feat: Option<&str>, main: &str, off: &str| {
            let mut c = character(&["Fighter"], &[main, off]);
            c.inventory[0].hand = Some(Hand::Main);
            c.inventory[1].hand = Some(Hand::Off);
            c.feats = feat.into_iter().map(str::to_string).collect();
            let b = build(&c, &lib, &OverlaySet::default());
            b.attacks.iter().find(|a| a.offhand).unwrap().notes.clone()
        };
        assert!(!held(None, "Dagger", "Rapier").is_empty(), "Rapier is not Light");
        assert!(held(Some("feat:Dual Wielder|XPHB"), "Dagger", "Rapier").is_empty(), "2024: Light main, any one-handed off");
        assert!(!held(Some("feat:Dual Wielder|XPHB"), "Rapier", "Dagger").is_empty(), "2024: the main weapon must be Light");
        assert!(held(Some("feat:Dual Wielder|T"), "Rapier", "Longsword").is_empty(), "2014: any one-handed melee");
        assert!(!held(Some("feat:Dual Wielder|T"), "Rapier", "Greataxe").is_empty());
    }

    #[test]
    fn wielding() {
        use crate::character::Hand;
        use crate::equipment::wield;
        let lib = library();
        let mut c = character(&["Fighter"], &["Longsword", "Dagger", "Greataxe"]);
        for l in &mut c.inventory {
            l.equipped = false;
        }
        c.inventory[1].quantity = 2;
        wield(&lib, &mut c, 0, Hand::Main);
        wield(&lib, &mut c, 1, Hand::Off);
        assert_eq!((c.inventory[0].hand, c.inventory[1].hand), (Some(Hand::Main), Some(Hand::Off)));
        // The second dagger into the main hand makes a pair and puts the longsword away.
        let msg = wield(&lib, &mut c, 1, Hand::Main);
        assert_eq!((c.inventory[0].hand, c.inventory[1].hand), (None, Some(Hand::Pair)), "{msg}");
        // A two-handed weapon takes both hands.
        wield(&lib, &mut c, 2, Hand::Main);
        assert_eq!((c.inventory[1].hand, c.inventory[2].hand), (None, Some(Hand::Both)));
        // Again: put away.
        wield(&lib, &mut c, 2, Hand::Main);
        assert_eq!(c.inventory[2].hand, None);
    }

    #[test]
    fn strength_group_effects() {
        use crate::engine::{Effect, Model, Op};
        use crate::formula::Formula;
        let lib = library();
        let c = character(&["Fighter"], &["Greataxe", "Longbow", "Rapier"]);
        let mut model = Model { stats: crate::standard::stats(), inputs: crate::standard::default_inputs(), ..Model::default() };
        for (ab, v) in [("str", 16), ("dex", 14)] {
            model.inputs.insert(format!("base.{ab}"), crate::value::Value::int(v));
        }
        model.inputs.insert("level".into(), crate::value::Value::int(1));
        model.effects.push(Effect {
            target: "attack.strength.damage".into(),
            op: Op::Add,
            value: Formula::parse("2").unwrap(),
            when: None,
            origin: "Rage".into(),
        });
        let prof = super::weapon_proficiency(&lib, &c, &[], &[]);
        super::add_attacks(&mut model, &lib, &c, &prof, &[], None, None);
        let ev = model.evaluate();
        assert_eq!(ev.get("attack.greataxe.damage").unwrap().to_string(), "1d12 + 5", "Strength melee");
        assert_eq!(ev.get("attack.longbow.damage").unwrap().to_string(), "1d8 + 2", "not ranged");
        assert_eq!(ev.get("attack.rapier.damage").unwrap().to_string(), "1d8 + 5", "finesse using Strength");
        assert_eq!(ev.get("attack.unarmed.damage").unwrap().to_string(), "6");
    }

    fn monk_library() -> Library {
        let mut lib = library();
        lib.load_json(&json!({
            "class": [{"name": "Monk", "source": "XPHB", "hd": {"faces": 8}, "proficiency": ["str", "dex"],
                "startingProficiencies": {"weapons": ["simple",
                    "Martial weapons that have the {@filter Light|items|type=martial weapon|property=light} property"]},
                "classTableGroups": [{"colLabels": ["Martial Arts"], "rows": [[{"type": "dice", "toRoll": [{"number": 1, "faces": 6}]}]]}],
                "classFeatures": ["Martial Arts|Monk|XPHB|1"]}],
            "classFeature": [{"name": "Martial Arts", "source": "XPHB", "className": "Monk", "classSource": "XPHB", "level": 1, "entries": ["x"]}],
            "baseitem": [
                {"name": "Shortsword", "source": "T", "type": "M|T", "weaponCategory": "martial", "weapon": true,
                 "property": ["F|T", "L|T"], "dmg1": "1d6", "dmgType": "P"}
            ],
            "item": [
                {"name": "Nunchaku", "source": "HB", "type": "M|XPHB", "weaponCategory": "simple", "weapon": true, "dmg1": "1d4", "dmgType": "B"}
            ]
        }));
        lib
    }

    fn monk(items: &[&str]) -> Character {
        let mut c = Character { name: "M".into(), ..Character::default() };
        c.abilities.str = 10;
        c.abilities.dex = 16;
        c.levels = vec![Level { class: "class:Monk|XPHB".into(), hp: Default::default(), choices: Default::default() }];
        c.inventory = items
            .iter()
            .map(|i| InventoryItem { item: Some(format!("item:{i}")), equipped: true, ..Default::default() })
            .collect();
        c
    }

    #[test]
    fn monk_weapons() {
        let lib = monk_library();
        let c = monk(&["Nunchaku|HB", "Shortsword|T", "Longsword|T"]);
        let b = build(&c, &lib, &OverlaySet::default());
        assert_eq!(b.martial_arts, Some(super::MartialArts { rules_2024: true }));
        let ev = b.evaluate();
        let by = |n: &str| b.attacks.iter().find(|a| a.name == n).unwrap();
        // A homebrew simple melee weapon is a monk weapon: Dex, and the
        // Martial Arts die (1d6) beats its own 1d4.
        assert!(by("Nunchaku").monk_weapon);
        assert_eq!(ev.int("attack.nunchaku.bonus"), Some(3 + 2));
        assert_eq!(ev.get("attack.nunchaku.damage").unwrap().to_string(), "1d6 + 3");
        // Light martial weapons are monk weapons in 2024.
        assert!(by("Shortsword").monk_weapon && by("Shortsword").proficient);
        // A Longsword is not (and not proficient).
        assert!(!by("Longsword").monk_weapon);
        assert_eq!(ev.get("attack.longsword.damage").unwrap().to_string(), "1d8");

        // Marking overrides the rules either way.
        let mut c = monk(&["Nunchaku|HB", "Longsword|T"]);
        c.inventory[0].monk_weapon = Some(false);
        c.inventory[1].monk_weapon = Some(true);
        let b = build(&c, &lib, &OverlaySet::default());
        let ev = b.evaluate();
        assert_eq!(ev.get("attack.nunchaku.damage").unwrap().to_string(), "1d4", "Strength +0, own die");
        assert_eq!(ev.get("attack.longsword.damage").unwrap().to_string(), "1d8 + 3", "Dex, its own bigger die");

        // Without Martial Arts nothing is a monk weapon.
        let fighter = build(&character(&["Fighter"], &["Dagger"]), &library(), &OverlaySet::default());
        assert!(!fighter.attacks[0].monk_weapon && fighter.martial_arts.is_none());
    }
}