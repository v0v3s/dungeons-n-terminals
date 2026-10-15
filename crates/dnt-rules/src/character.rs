//! The character file: the player's choices and current state, never derived
//! values (those come from [`crate::builder`]).
//!
//! Entity references use overlay-style ids (`class:Wizard|XPHB`,
//! `subclass:Evoker|Wizard|XPHB|XPHB`, `feat:Alert|XPHB`).
//!
//! ```toml
//! name = "Elara"
//! species = "race:Elf|XPHB"
//! background = "background:Sage|XPHB"
//! feats = ["feat:Alert|XPHB"]
//!
//! [abilities]
//! str = 8
//! dex = 14
//! con = 13
//! int = 15
//! wis = 12
//! cha = 10
//!
//! [[ability_bonus]]
//! origin = "Background (Sage)"
//! int = 2
//! con = 1
//!
//! [proficiencies]
//! skills = ["arcana", "history"]
//!
//! [[levels]]                       # one per character level, in order taken
//! class = "class:Wizard|XPHB"
//! [[levels]]
//! class = "class:Fighter|XPHB"
//! hp = 7                           # rolled; default "average"
//!
//! [subclasses]
//! "class:Wizard|XPHB" = "subclass:Evoker|Wizard|XPHB|XPHB"
//!
//! [state]
//! hp = 12
//! resources_used = { arcane_recovery = 1 }
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Character {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub species: Option<String>,
    /// 2014-style subrace, e.g. `race:Dwarf (Hill)|PHB`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subspecies: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feats: Vec<String>,
    /// Chosen optional features: fighting styles, invocations, maneuvers, …
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default)]
    pub abilities: Abilities,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ability_bonus: Vec<AbilityBonus>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub proficiencies: Proficiencies,
    /// One entry per character level, in the order taken. The first entry's
    /// class is the starting class.
    #[serde(default)]
    pub levels: Vec<Level>,
    /// Chosen subclass per class id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub subclasses: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub equipped: Equipped,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inventory: Vec<InventoryItem>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub currency: Currency,
    #[serde(default, skip_serializing_if = "is_default")]
    pub spells: Spells,
    #[serde(default, skip_serializing_if = "is_default")]
    pub state: State,
    /// Manual overrides: stat key → formula (usually just a number).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Abilities {
    pub str: i64,
    pub dex: i64,
    pub con: i64,
    pub int: i64,
    pub wis: i64,
    pub cha: i64,
}

impl Default for Abilities {
    fn default() -> Self {
        Abilities {
            str: 10,
            dex: 10,
            con: 10,
            int: 10,
            wis: 10,
            cha: 10,
        }
    }
}

impl Abilities {
    pub fn get(&self, ability: &str) -> i64 {
        match ability {
            "str" => self.str,
            "dex" => self.dex,
            "con" => self.con,
            "int" => self.int,
            "wis" => self.wis,
            "cha" => self.cha,
            _ => 0,
        }
    }
}

/// Ability score increases with their origin (species, background, ASI, feat).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AbilityBonus {
    pub origin: String,
    #[serde(flatten)]
    pub scores: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Proficiencies {
    /// Skill keys as in `skill.<key>`, e.g. `sleight_of_hand`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expertise: Vec<String>,
    /// Skills with half proficiency (e.g. Jack of All Trades applies to all
    /// others; listing them here is for homebrew/partial cases).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub half: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// Extra weapon proficiencies: `simple`, `martial` or a weapon name.
    /// Classes, species, backgrounds and feats add theirs automatically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weapons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Level {
    pub class: String,
    #[serde(default, skip_serializing_if = "HitPoints::is_average")]
    pub hp: HitPoints,
    /// Choices made when taking this level (ASI or feat).
    #[serde(default, skip_serializing_if = "LevelChoices::is_empty")]
    pub choices: LevelChoices,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LevelChoices {
    /// Ability score increases, e.g. `{ str = 2 }` or `{ dex = 1, con = 1 }`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub ability: BTreeMap<String, i64>,
    /// A feat taken instead of (or as part of) an ASI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feat: Option<String>,
    /// Class feature choices, keyed by choice: `"Fighting Style" =
    /// ["feat:Defense|XPHB"]`, `Expertise = ["stealth", "perception"]`,
    /// `"Weapon Mastery" = ["item:Longsword|XPHB"]`, …
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub picks: BTreeMap<String, Vec<String>>,
}

impl LevelChoices {
    pub fn is_empty(&self) -> bool {
        self.ability.is_empty() && self.feat.is_none() && self.picks.is_empty()
    }
}

/// Hit points gained at a level. The very first level is always the maximum.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum HitPoints {
    Rolled(i64),
    Method(HpMethod),
    #[default]
    #[serde(skip)]
    Average,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HpMethod {
    Average,
    Max,
}

impl HitPoints {
    fn is_average(&self) -> bool {
        matches!(self, HitPoints::Average | HitPoints::Method(HpMethod::Average))
    }

    /// HP for one level with a hit die of `faces`.
    pub fn amount(&self, faces: i64) -> i64 {
        match self {
            HitPoints::Rolled(n) => *n,
            HitPoints::Method(HpMethod::Max) => faces,
            _ => faces / 2 + 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Equipped {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shield: bool,
}

/// One inventory line: a library item (`item`) or a custom one (`name`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InventoryItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    /// Display name; required for custom items, overrides the library name otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub quantity: i64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub equipped: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub attuned: bool,
    /// Weight per item in pounds, for custom items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    /// Counts as a monk weapon (`true`) or not (`false`) regardless of the
    /// Martial Arts rules; unset follows the rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monk_weapon: Option<bool>,
    /// Where a worn item is worn (with `equipped`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<Slot>,
    /// A weapon or shield in hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<Hand>,
    /// Names a container so other lines can be `inside` it (set when the
    /// first thing goes in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The `id` of the container this line is in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inside: Option<String>,
    /// Make any item a container (`true`) or stop one being one (`false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<bool>,
    /// What a container holds, in pounds (0: no limit), instead of the item's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<f64>,
    /// The contents weigh nothing (a Bag of Holding), instead of the item's setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weightless: Option<bool>,
}

/// Where a worn item goes. Each slot takes one item, except `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Slot {
    Head,
    Eyes,
    Neck,
    Shoulders,
    Body,
    Arms,
    Hands,
    Waist,
    Feet,
    Ring1,
    Ring2,
    Other,
}

impl Slot {
    pub const ALL: [Slot; 12] = [
        Slot::Head,
        Slot::Eyes,
        Slot::Neck,
        Slot::Shoulders,
        Slot::Body,
        Slot::Arms,
        Slot::Hands,
        Slot::Waist,
        Slot::Feet,
        Slot::Ring1,
        Slot::Ring2,
        Slot::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Eyes => "Eyes",
            Slot::Neck => "Neck",
            Slot::Shoulders => "Shoulders",
            Slot::Body => "Body",
            Slot::Arms => "Arms",
            Slot::Hands => "Hands",
            Slot::Waist => "Waist",
            Slot::Feet => "Feet",
            Slot::Ring1 => "Ring",
            Slot::Ring2 => "Ring",
            Slot::Other => "Other",
        }
    }

    /// What usually goes there, for the slot picker.
    pub fn examples(self) -> &'static str {
        match self {
            Slot::Head => "helms, hats, circlets",
            Slot::Eyes => "goggles, lenses",
            Slot::Neck => "amulets, necklaces, periapts",
            Slot::Shoulders => "cloaks, capes, mantles",
            Slot::Body => "armor, robes",
            Slot::Arms => "bracers",
            Slot::Hands => "gloves, gauntlets",
            Slot::Waist => "belts, girdles",
            Slot::Feet => "boots, slippers",
            Slot::Ring1 | Slot::Ring2 => "rings",
            Slot::Other => "anything else (any number)",
        }
    }
}

/// How a weapon or shield is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hand {
    Main,
    Off,
    /// One weapon in both hands (two-handed, or versatile used two-handed).
    Both,
    /// Two of the same weapon, one in each hand (two daggers).
    Pair,
}

impl Hand {
    pub fn uses_main(self) -> bool {
        matches!(self, Hand::Main | Hand::Both | Hand::Pair)
    }

    pub fn uses_off(self) -> bool {
        matches!(self, Hand::Off | Hand::Both | Hand::Pair)
    }

    pub fn label(self) -> &'static str {
        match self {
            Hand::Main => "main hand",
            Hand::Off => "off hand",
            Hand::Both => "both hands",
            Hand::Pair => "one in each hand",
        }
    }
}

fn one() -> i64 {
    1
}

fn is_one(n: &i64) -> bool {
    *n == 1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Currency {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cp: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub sp: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ep: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gp: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pp: i64,
}

impl Currency {
    /// Total value in copper pieces.
    pub fn total_cp(&self) -> i64 {
        self.cp + self.sp * 10 + self.ep * 50 + self.gp * 100 + self.pp * 1000
    }

    /// Add an amount in copper as the largest coins (gold, silver, copper).
    pub fn add_cp(&mut self, cp: i64) {
        self.gp += cp / 100;
        self.sp += (cp % 100) / 10;
        self.cp += cp % 10;
    }

    /// Apply a change like `+15gp`, `-3 sp`, `20gp` (sets gold) or `+250`
    /// (gold by default). Returns false if the text cannot be parsed.
    pub fn apply(&mut self, text: &str) -> bool {
        let t: String = text.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        let (sign, rest) = match t.chars().next() {
            Some('+') => (Some(1), &t[1..]),
            Some('-') => (Some(-1), &t[1..]),
            _ => (None, t.as_str()),
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let Ok(n) = digits.parse::<i64>() else { return false };
        let coin = match &rest[digits.len()..] {
            "" | "gp" | "g" => &mut self.gp,
            "sp" | "s" => &mut self.sp,
            "cp" | "c" => &mut self.cp,
            "ep" | "e" => &mut self.ep,
            "pp" | "p" => &mut self.pp,
            _ => return false,
        };
        match sign {
            Some(s) => *coin = (*coin + s * n).max(0),
            None => *coin = n,
        }
        true
    }
}

/// Chosen spells. Unlike level choices these change often (daily
/// preparation), so they live outside the level history.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Spells {
    /// Per class id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub classes: BTreeMap<String, ClassSpells>,
    /// Spells picked for feature grants ("choose 2 Cleric cantrips"),
    /// keyed by grant (see `Build::granted`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub chosen: BTreeMap<String, Vec<String>>,
    /// Spells shown first on the Spells tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub favorites: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ClassSpells {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cantrips: Vec<String>,
    /// Prepared (or known) leveled spells.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prepared: Vec<String>,
    /// A Wizard's spellbook.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spellbook: Vec<String>,
}

/// Mutable play state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    /// Current hit points; `None` means full.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hp: Option<i64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub temp_hp: i64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resources_used: BTreeMap<String, i64>,
    /// Expended spell slots per level, index 0 = 1st level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots_used: Vec<i64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pact_slots_used: i64,
    /// Expended hit dice per die, e.g. `d8 = 1`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hit_dice_used: BTreeMap<String, i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub toggles: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exhaustion: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub death_successes: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub death_failures: i64,
    /// Spell being concentrated on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concentration: Option<String>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

impl Character {
    pub fn total_level(&self) -> usize {
        self.levels.len()
    }

    /// Classes in the order first taken, with their levels.
    pub fn class_levels(&self) -> Vec<(&str, i64)> {
        let mut out: Vec<(&str, i64)> = Vec::new();
        for l in &self.levels {
            match out.iter_mut().find(|(c, _)| *c == l.class) {
                Some((_, n)) => *n += 1,
                None => out.push((&l.class, 1)),
            }
        }
        out
    }

    pub fn starting_class(&self) -> Option<&str> {
        self.levels.first().map(|l| l.class.as_str())
    }

    /// Level of one class.
    pub fn class_level(&self, class: &str) -> i64 {
        self.levels.iter().filter(|l| l.class == class).count() as i64
    }

    /// Remove the most recent level, and the class's subclass if no levels
    /// of that class remain. Returns the removed level.
    pub fn remove_last_level(&mut self) -> Option<Level> {
        let last = self.levels.pop()?;
        if self.class_level(&last.class) == 0 {
            self.subclasses.remove(&last.class);
        }
        Some(last)
    }

    pub fn from_toml(text: &str) -> Result<Character, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("character serialises")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELARA: &str = r#"
name = "Elara"
species = "race:Elf|XPHB"
feats = ["feat:Alert|XPHB"]

[abilities]
str = 8
dex = 14
con = 13
int = 15
wis = 12
cha = 10

[[ability_bonus]]
origin = "Background (Sage)"
int = 2
con = 1

[proficiencies]
skills = ["arcana", "history"]

[[levels]]
class = "class:Wizard|XPHB"
[[levels]]
class = "class:Wizard|XPHB"
hp = "max"
[[levels]]
class = "class:Fighter|XPHB"
hp = 7
[[levels]]
class = "class:Rogue|XPHB"

[subclasses]
"class:Wizard|XPHB" = "subclass:Evoker|Wizard|XPHB|XPHB"

[state]
hp = 12
resources_used = { arcane_recovery = 1 }
"#;

    #[test]
    fn parses_multiclass_character() {
        let c = Character::from_toml(ELARA).unwrap();
        assert_eq!(c.total_level(), 4);
        assert_eq!(c.class_levels(), vec![("class:Wizard|XPHB", 2), ("class:Fighter|XPHB", 1), ("class:Rogue|XPHB", 1)]);
        assert_eq!(c.starting_class(), Some("class:Wizard|XPHB"));
        assert_eq!(c.levels[1].hp.amount(6), 6);
        assert_eq!(c.levels[2].hp.amount(10), 7);
        assert_eq!(c.levels[3].hp.amount(8), 5);
        assert_eq!(c.ability_bonus[0].scores["int"], 2);
        assert_eq!(c.state.resources_used["arcane_recovery"], 1);
    }

    #[test]
    fn currency() {
        let mut c = Currency::default();
        c.add_cp(1550);
        assert_eq!((c.gp, c.sp, c.cp), (15, 5, 0));
        assert!(c.apply("+10gp"));
        assert!(c.apply("-3 sp"));
        assert!(c.apply("7cp"));
        assert_eq!((c.gp, c.sp, c.cp), (25, 2, 7));
        assert!(c.apply("+5"));
        assert_eq!(c.gp, 30);
        assert!(!c.apply("lots"));
        assert_eq!(c.total_cp(), 3027);
    }

    #[test]
    fn roundtrips() {
        let c = Character::from_toml(ELARA).unwrap();
        let again = Character::from_toml(&c.to_toml()).unwrap();
        assert_eq!(c, again);
        // Defaults are not written out.
        assert!(!c.to_toml().contains("death_failures"));
    }
}
