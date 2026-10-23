//! Friendly editing of common entity fields: each format shows a field as
//! short text ("1 bonus action", "Concentration, up to 1 minute", "50 gp")
//! or a list of choices, and parses the text back into data-format JSON.

use serde_json::{json, Map, Value};

use crate::kind::Kind;
use crate::library::Library;
use crate::view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    SpellLevel,
    School,
    Time,
    Range,
    Components,
    Duration,
    Classes,
    Ritual,
    Rarity,
    ItemType,
    Attunement,
    Coins,
    DamageType,
    WeaponCategory,
    FeatCategory,
    /// Damage dice: `1d8`, `2d6`, `1d4+1`.
    Dice,
    /// A magic bonus: `+1`.
    Bonus,
    /// Weapon properties: several of a list, and custom ones.
    Properties,
    /// Ability score increases (`ability`): fixed, chosen or weighted.
    AbilityIncrease,
    /// Proficiencies of a kind: names, `any N`, `choose N: a / b`.
    Proficiency(Proficiency),
    /// Feats granted, by name.
    Feats,
    /// A species' sizes.
    Size,
    /// Which rules an entry is written for (`edition`).
    Edition,
}

/// What a proficiency field grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proficiency {
    Skill,
    Tool,
    Language,
    Weapon,
    Armor,
}

/// The friendly format of a field, if it has one.
pub fn format_for(kind: Kind, field: &str) -> Option<Format> {
    use Format::*;
    Some(match (kind, field) {
        (Kind::Spell, "level") => SpellLevel,
        (Kind::Spell, "school") => School,
        (Kind::Spell, "time") => Time,
        (Kind::Spell, "range") => Range,
        (Kind::Spell, "components") => Components,
        (Kind::Spell, "duration") => Duration,
        (Kind::Spell, "classes") => Classes,
        (Kind::Spell, "meta") => Ritual,
        (Kind::Item | Kind::BaseItem | Kind::MagicVariant, f) => match f {
            "rarity" => Rarity,
            "type" => ItemType,
            "reqAttune" => Attunement,
            "value" => Coins,
            "dmgType" => DamageType,
            "weaponCategory" => WeaponCategory,
            "dmg1" | "dmg2" => Dice,
            "bonusWeapon" | "bonusAc" | "bonusSpellAttack" | "bonusSavingThrow" => Bonus,
            "property" => Properties,
            _ => return None,
        },
        (Kind::Feat, "category") => FeatCategory,
        (Kind::Background | Kind::Race | Kind::Subrace | Kind::Feat, "ability") => AbilityIncrease,
        (Kind::Background | Kind::Race | Kind::Subrace, "feats") => Feats,
        (Kind::Race | Kind::Subrace, "size") => Size,
        (_, "edition") => Edition,
        (_, "skillProficiencies") => Proficiency(self::Proficiency::Skill),
        (_, "toolProficiencies") => Proficiency(self::Proficiency::Tool),
        (_, "languageProficiencies") => Proficiency(self::Proficiency::Language),
        (_, "weaponProficiencies") => Proficiency(self::Proficiency::Weapon),
        (_, "armorProficiencies") => Proficiency(self::Proficiency::Armor),
        _ => return None,
    })
}

/// A short hint shown while typing a value.
pub fn hint(format: Format) -> &'static str {
    match format {
        Format::Time => "e.g. 1 action · 1 bonus action · 1 reaction, which you take when … · 10 minutes",
        Format::Range => "e.g. 60 feet · 1 mile · Self · Touch · Sight · Self (15-foot cone)",
        Format::Components => "e.g. V, S, M (a pinch of salt)",
        Format::Duration => "e.g. Instantaneous · 1 hour · Concentration, up to 1 minute · Until dispelled",
        Format::Classes => "e.g. Wizard, Sorcerer",
        Format::Attunement => "yes · no · or who: by a wizard",
        Format::Coins => "e.g. 50 gp · 5 sp · 10 cp",
        Format::Dice => "dice: 1d8 · 2d6 · 1d4+1",
        Format::Bonus => "e.g. +1 · +2 · +3",
        Format::Properties => "tick them; a custom one is added by its name",
        Format::AbilityIncrease => "e.g. Strength +2, Constitution +1 · +2/+1 or +1/+1/+1 among Str, Dex, Con · +1 to 2 of Str/Dex/Con",
        Format::Proficiency(_) => "tick them; or add: any 2 · choose 2: Arcana / History / Nature",
        Format::Feats => "feats by name, e.g. Alert, Magic Initiate: Cleric",
        Format::Size => "tick one (two: the player chooses)",
        _ => "",
    }
}

const SCHOOLS: &[&str] = &["A", "C", "D", "E", "V", "I", "N", "T"];
const RARITIES: &[&str] = &["none", "common", "uncommon", "rare", "very rare", "legendary", "artifact", "varies", "unknown"];
const ITEM_TYPES: &[&str] = &[
    "G|XPHB", "M|XPHB", "R|XPHB", "A|XPHB", "LA|XPHB", "MA|XPHB", "HA|XPHB", "S|XPHB", "SCF|XPHB", "P|XPHB", "SC|XPHB", "RD|XDMG",
    "RG|XDMG", "WD|XDMG", "INS|XPHB", "AT|XPHB", "T|XPHB", "GS|XPHB", "FD|XPHB", "TG|XDMG", "$G|XDMG",
];
const DAMAGE_TYPES: &[&str] = &["B", "P", "S", "A", "C", "F", "O", "L", "N", "I", "Y", "R", "T"];
const FEAT_CATEGORIES: &[&str] = &["O", "G", "FS", "EB"];
const PROPERTIES: &[&str] =
    &["A|XPHB", "F|XPHB", "H|XPHB", "L|XPHB", "LD|XPHB", "R|XPHB", "T|XPHB", "2H|XPHB", "V|XPHB", "RLD|XDMG", "BF|XDMG", "S"];

const SIZES: &[(&str, &str)] = &[("T", "Tiny"), ("S", "Small"), ("M", "Medium"), ("L", "Large"), ("H", "Huge"), ("G", "Gargantuan")];
const ABILITIES: &[(&str, &str)] =
    &[("str", "Strength"), ("dex", "Dexterity"), ("con", "Constitution"), ("int", "Intelligence"), ("wis", "Wisdom"), ("cha", "Charisma")];

/// The names a proficiency field offers to tick (as the data writes them).
fn proficiency_names(kind: Proficiency) -> &'static [&'static str] {
    match kind {
        Proficiency::Skill => &[
            "acrobatics",
            "animal handling",
            "arcana",
            "athletics",
            "deception",
            "history",
            "insight",
            "intimidation",
            "investigation",
            "medicine",
            "nature",
            "perception",
            "performance",
            "persuasion",
            "religion",
            "sleight of hand",
            "stealth",
            "survival",
        ],
        Proficiency::Tool => &[
            "thieves' tools",
            "herbalism kit",
            "disguise kit",
            "forgery kit",
            "poisoner's kit",
            "navigator's tools",
            "calligrapher's supplies",
            "cartographer's tools",
            "smith's tools",
            "tinker's tools",
            "vehicles (land)",
            "vehicles (water)",
        ],
        Proficiency::Language => &[
            "common",
            "dwarvish",
            "elvish",
            "giant",
            "gnomish",
            "goblin",
            "halfling",
            "orc",
            "draconic",
            "infernal",
            "celestial",
            "abyssal",
            "sylvan",
            "primordial",
            "undercommon",
            "deep speech",
            "thieves' cant",
            "druidic",
        ],
        Proficiency::Weapon => &["simple", "martial"],
        Proficiency::Armor => &["light", "medium", "heavy", "shield"],
    }
}

/// What a proficiency key's "any" means, for people.
const ANY: &[(&str, &str)] = &[
    ("any", "any"),
    ("anyStandard", "any standard"),
    ("anyExotic", "any exotic"),
    ("anyGamingSet", "any gaming set"),
    ("anyArtisansTool", "any artisan's tools"),
    ("anyMusicalInstrument", "any musical instrument"),
];

fn title(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The name of a `name|source` key (feats, items): `Magic Initiate: Cleric`.
fn feat_label(key: &str) -> String {
    title(&key.split('|').next().unwrap_or(key).replace("; ", ": "))
}

/// A feat key for a typed name: `magic initiate; cleric|xphb`.
fn feat_key(name: &str, lib: &Library) -> Result<String, String> {
    let wanted = name.replace(": ", "; ").to_lowercase();
    let base = wanted.split(';').next().unwrap_or(&wanted).trim().to_string();
    // The feat itself, or the feat of a version; prefer the 2024 one.
    let found = lib
        .entities()
        .iter()
        .filter(|e| e.kind == Kind::Feat && (e.name.to_lowercase() == wanted || e.name.to_lowercase() == base))
        .max_by_key(|e| (e.name.to_lowercase() == wanted, e.source == "XPHB"))
        .ok_or_else(|| format!("no feat named {name:?}"))?;
    Ok(format!("{wanted}|{}", found.source.to_lowercase()))
}

fn ability_of(s: &str) -> Option<&'static str> {
    let s = s.trim().to_lowercase();
    ABILITIES.iter().find(|(k, n)| s == *k || s == n.to_lowercase()).map(|(k, _)| *k)
}

fn ability_text(v: &Value) -> String {
    let abbr = |a: &Value| a.as_str().map_or("?".to_string(), title);
    let list =
        |from: Option<&Value>| from.and_then(Value::as_array).map_or("any".into(), |f| f.iter().map(abbr).collect::<Vec<_>>().join("/"));
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|alt| {
            let mut parts = Vec::new();
            let fixed: Vec<String> =
                alt.iter().filter_map(|(k, n)| Some(format!("{} {:+}", ABILITIES.iter().find(|(a, _)| a == k)?.1, n.as_i64()?))).collect();
            if !fixed.is_empty() {
                parts.push(fixed.join(", "));
            }
            if let Some(c) = alt.get("choose") {
                if let Some(w) = c.get("weighted") {
                    let weights: Vec<String> = w
                        .get("weights")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_i64)
                        .map(|n| format!("{n:+}"))
                        .collect();
                    parts.push(format!("{} among {}", weights.join("/"), list(w.get("from"))));
                } else {
                    let count = c.get("count").and_then(Value::as_u64).unwrap_or(1);
                    let amount = c.get("amount").and_then(Value::as_i64).unwrap_or(1);
                    parts.push(format!("{amount:+} to {count} of {}", list(c.get("from"))));
                }
            }
            parts.join("; ")
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn parse_ability(t: &str) -> Result<Vec<Value>, String> {
    let all: Vec<Value> = ABILITIES.iter().map(|(k, _)| json!(k)).collect();
    let abilities = |list: &str| -> Result<Vec<Value>, String> {
        if list.trim().eq_ignore_ascii_case("any") {
            return Ok(all.clone());
        }
        list.split(['/', ','])
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(|a| ability_of(a).map(|k| json!(k)).ok_or_else(|| format!("{a:?} is not an ability")))
            .collect()
    };
    let mut out = Vec::new();
    // "+2/+1 or +1/+1/+1 among …": an alternative without its list takes the next one's.
    let alts: Vec<&str> = t.split(" or ").map(str::trim).filter(|a| !a.is_empty()).collect();
    let shared = alts.iter().rev().find_map(|a| a.split_once(" among ").map(|(_, l)| l.to_string()));
    for alt in alts {
        let mut o = Map::new();
        for part in alt.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let lower = part.to_lowercase();
            if lower.starts_with('+') && (lower.contains(" among ") || (!lower.contains(" to ") && lower.contains('/'))) {
                let (w, list) = lower
                    .split_once(" among ")
                    .map_or((lower.as_str(), shared.clone().unwrap_or("any".into())), |(w, l)| (w, l.to_string()));
                let weights: Vec<i64> = w
                    .split('/')
                    .map(|n| n.trim().trim_start_matches('+').parse::<i64>())
                    .collect::<Result<_, _>>()
                    .map_err(|_| format!("{part:?}: write +2/+1"))?;
                o.insert("choose".into(), json!({"weighted": {"from": abilities(&list)?, "weights": weights}}));
            } else if let Some((amount, rest)) = lower.split_once(" to ") {
                let amount: i64 =
                    amount.trim().trim_start_matches('+').parse().map_err(|_| format!("{part:?}: write +1 to 2 of Str/Dex"))?;
                let (count, list) = rest.split_once(" of ").ok_or_else(|| format!("{part:?}: write +1 to 2 of Str/Dex"))?;
                let count: u64 = count.trim().parse().map_err(|_| format!("{part:?}: how many?"))?;
                let mut c = json!({"from": abilities(list)?, "count": count});
                if amount != 1 {
                    c["amount"] = json!(amount);
                }
                o.insert("choose".into(), c);
            } else {
                for f in part.split(',').map(str::trim).filter(|f| !f.is_empty()) {
                    let (name, n) = f.rsplit_once(' ').ok_or_else(|| format!("{f:?}: write Strength +2"))?;
                    let k = ability_of(name).ok_or_else(|| format!("{name:?} is not an ability"))?;
                    let n: i64 = n.trim_start_matches('+').parse().map_err(|_| format!("{f:?}: write Strength +2"))?;
                    o.insert(k.into(), json!(n));
                }
            }
        }
        if !o.is_empty() {
            out.push(Value::Object(o));
        }
    }
    Ok(out)
}

fn proficiency_text(v: &Value) -> String {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|alt| {
            alt.iter()
                .map(|(k, n)| match (k.as_str(), n) {
                    ("choose", c) => {
                        let from: Vec<String> =
                            c.get("from").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(title).collect();
                        format!("choose {}: {}", c.get("count").and_then(Value::as_u64).unwrap_or(1), from.join(" / "))
                    }
                    (k, n) if ANY.iter().any(|(a, _)| *a == k) => {
                        format!("{} {}", ANY.iter().find(|(a, _)| *a == k).unwrap().1, n.as_u64().unwrap_or(1))
                    }
                    (k, _) => title(k.split('|').next().unwrap_or(k)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn parse_proficiency(t: &str, format: Format) -> Result<Vec<Value>, String> {
    let known = choices(format).unwrap_or_default();
    let mut out = Vec::new();
    for alt in t.split(" or ").map(str::trim).filter(|a| !a.is_empty()) {
        let mut o = Map::new();
        for item in alt.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let lower = item.to_lowercase();
            if let Some(rest) = lower.strip_prefix("choose ") {
                let (count, list) = rest.split_once(':').ok_or_else(|| format!("{item:?}: write choose 2: Arcana / History"))?;
                let count: u64 = count.trim().parse().map_err(|_| format!("{item:?}: how many?"))?;
                let from: Vec<Value> = list.split('/').map(str::trim).filter(|x| !x.is_empty()).map(|x| json!(x)).collect();
                o.insert("choose".into(), json!({"from": from, "count": count}));
            } else if let Some((key, _)) = ANY.iter().rev().find(|(_, label)| lower.starts_with(label)) {
                let n: u64 = lower[ANY.iter().find(|(k, _)| k == key).unwrap().1.len()..].trim().parse().unwrap_or(1);
                o.insert(key.to_string(), json!(n));
            } else {
                let key = known
                    .iter()
                    .find(|(label, _)| label.eq_ignore_ascii_case(item))
                    .and_then(|(_, v)| v.as_ref()?.as_str().map(str::to_string));
                o.insert(key.unwrap_or(lower), json!(true));
            }
        }
        if !o.is_empty() {
            out.push(Value::Object(o));
        }
    }
    Ok(out)
}

/// A property's name: a known code, else as written.
fn property_label(code: &str) -> String {
    match view::item_property(code.split('|').next().unwrap_or(code)) {
        "Other" => code.split('|').next().unwrap_or(code).to_string(),
        name => name.to_string(),
    }
}

/// The choices of a choice-like format: label and value (`None` removes
/// the field).
pub fn choices(format: Format) -> Option<Vec<(String, Option<Value>)>> {
    Some(match format {
        Format::SpellLevel => {
            (0..=9u64).map(|l| (if l == 0 { "Cantrip".to_string() } else { format!("Level {l}") }, Some(json!(l)))).collect()
        }
        Format::School => SCHOOLS.iter().map(|s| (view::spell_school(s).to_string(), Some(json!(s)))).collect(),
        Format::Ritual => vec![("Not a ritual".into(), None), ("Ritual".into(), Some(json!({"ritual": true})))],
        Format::Rarity => {
            RARITIES.iter().map(|r| (if *r == "none" { "none (mundane)".to_string() } else { r.to_string() }, Some(json!(r)))).collect()
        }
        Format::ItemType => std::iter::once(("— none (wondrous item or other)".to_string(), None))
            .chain(ITEM_TYPES.iter().map(|t| (view::item_type(t.split('|').next().unwrap()).to_string(), Some(json!(t)))))
            .collect(),
        Format::DamageType => DAMAGE_TYPES.iter().map(|d| (view::damage_type(d).to_string(), Some(json!(d)))).collect(),
        Format::WeaponCategory => ["simple", "martial"].iter().map(|c| (c.to_string(), Some(json!(c)))).collect(),
        Format::FeatCategory => std::iter::once(("— none".to_string(), None))
            .chain(FEAT_CATEGORIES.iter().map(|c| (view::feat_category(c).to_string(), Some(json!(c)))))
            .collect(),
        Format::Properties => PROPERTIES.iter().map(|p| (property_label(p), Some(json!(p)))).collect(),
        Format::Proficiency(kind) => proficiency_names(kind).iter().map(|n| (title(n), Some(json!(n)))).collect(),
        Format::Size => SIZES.iter().map(|(code, name)| (name.to_string(), Some(json!(code)))).collect(),
        Format::Edition => {
            vec![("2024 rules".into(), Some(json!("one"))), ("2014 rules".into(), Some(json!("classic"))), ("— not said".into(), None)]
        }
        _ => return None,
    })
}

/// Show a value in the format.
pub fn display(format: Format, value: Option<&Value>) -> String {
    let Some(v) = value else {
        return match format {
            Format::Ritual => "no".into(),
            Format::Attunement => "no".into(),
            _ => "—".into(),
        };
    };
    let wrap = |field: &str| json!({ field: v });
    match format {
        Format::SpellLevel => match v.as_u64() {
            Some(0) => "Cantrip".into(),
            Some(l) => format!("Level {l}"),
            None => v.to_string(),
        },
        Format::School => v.as_str().map(view::spell_school).unwrap_or("?").to_string(),
        Format::Time => view::spell_time(&wrap("time")),
        Format::Range => view::spell_range(&wrap("range")),
        Format::Components => components_text(v),
        Format::Duration => view::spell_duration(&wrap("duration")),
        Format::Classes => class_names(v).join(", "),
        Format::Ritual => if v.get("ritual").and_then(Value::as_bool) == Some(true) { "yes" } else { "no" }.into(),
        Format::Rarity | Format::WeaponCategory => v.as_str().unwrap_or("").to_string(),
        Format::ItemType => v.as_str().map(|t| view::item_type(t.split('|').next().unwrap_or(t))).unwrap_or("").to_string(),
        Format::Attunement => match v {
            Value::Bool(true) => "yes".into(),
            Value::String(s) => s.clone(),
            _ => "no".into(),
        },
        Format::Coins => v.as_u64().map(view::format_coins).unwrap_or_else(|| v.to_string()),
        Format::DamageType => v.as_str().map(view::damage_type).unwrap_or("").to_string(),
        Format::FeatCategory => v.as_str().map(view::feat_category).unwrap_or("").to_string(),
        Format::Dice | Format::Bonus => v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
        Format::Edition => match v.as_str() {
            Some("one") => "2024 rules".into(),
            Some("classic") => "2014 rules".into(),
            _ => v.to_string(),
        },
        Format::AbilityIncrease => ability_text(v),
        Format::Proficiency(_) => proficiency_text(v),
        Format::Feats => v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
            .flat_map(|o| o.keys().map(|k| feat_label(k)))
            .collect::<Vec<_>>()
            .join(", "),
        Format::Size => v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|c| SIZES.iter().find(|(code, _)| *code == c).map_or(c.to_string(), |(_, n)| n.to_string()))
            .collect::<Vec<_>>()
            .join(", "),
        Format::Properties => v
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| property_label(p.as_str().or_else(|| p.get("uid").and_then(Value::as_str)).unwrap_or("?")))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

/// Whether several choices can be picked (and custom ones added).
pub fn multiple(format: Format) -> bool {
    matches!(format, Format::Properties | Format::Proficiency(_) | Format::Size)
}

fn components_text(v: &Value) -> String {
    let mut parts = Vec::new();
    for (key, label) in [("v", "V"), ("s", "S"), ("r", "R")] {
        if v.get(key).and_then(Value::as_bool) == Some(true) {
            parts.push(label.to_string());
        }
    }
    match v.get("m") {
        Some(Value::String(m)) => parts.push(format!("M ({m})")),
        Some(Value::Object(m)) => parts.push(format!("M ({})", m.get("text").and_then(Value::as_str).unwrap_or(""))),
        Some(Value::Bool(true)) => parts.push("M".into()),
        _ => {}
    }
    parts.join(", ")
}

fn class_names(v: &Value) -> Vec<String> {
    v.get("fromClassList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.get("name").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// Parse text typed for a field. `previous` is the current value, used to
/// keep details the text does not show (a material component's cost).
pub fn parse(format: Format, text: &str, previous: Option<&Value>, lib: &Library) -> Result<Option<Value>, String> {
    let t = text.trim();
    let lower = t.to_lowercase();
    match format {
        Format::Time => parse_time(t).map(Some),
        Format::Range => parse_range(&lower).map(Some),
        Format::Components => parse_components(t, previous).map(Some),
        Format::Duration => parse_duration(&lower).map(Some),
        Format::Classes => parse_classes(t, lib).map(Some),
        Format::Coins => parse_coins(&lower).map(|cp| Some(json!(cp))),
        Format::Dice => {
            let ok = |d: &str| {
                let (dice, plus) = d.split_once('+').unwrap_or((d, "0"));
                let (n, faces) = dice.split_once('d').unwrap_or(("", ""));
                (n.is_empty() || n.parse::<u32>().is_ok())
                    && faces.parse::<u32>().is_ok_and(|f| f > 0)
                    && plus.trim().parse::<u32>().is_ok()
            };
            let d = lower.replace(' ', "");
            if d.is_empty() {
                Ok(None)
            } else if ok(&d) {
                Ok(Some(json!(d)))
            } else {
                Err(format!("{t:?} is not dice: write 1d8, 2d6 or 1d4+1"))
            }
        }
        Format::Bonus => {
            let n = lower.trim_start_matches('+').trim();
            match n.parse::<i64>() {
                _ if n.is_empty() => Ok(None),
                Ok(0) => Ok(None),
                Ok(b) => Ok(Some(json!(format!("{b:+}")))),
                Err(_) => Err(format!("{t:?} is not a bonus: write +1, +2 or +3")),
            }
        }
        Format::AbilityIncrease => parse_ability(t).map(|v| (!v.is_empty()).then_some(Value::Array(v))),
        Format::Proficiency(_) => parse_proficiency(t, format).map(|v| (!v.is_empty()).then_some(Value::Array(v))),
        Format::Feats => {
            let mut grant = Map::new();
            for name in t.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                grant.insert(feat_key(name, lib)?, json!(true));
            }
            Ok((!grant.is_empty()).then(|| json!([grant])))
        }
        Format::Size => {
            let codes: Vec<Value> = t
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| {
                    SIZES
                        .iter()
                        .find(|(code, name)| name.eq_ignore_ascii_case(p) || code.eq_ignore_ascii_case(p))
                        .map(|(code, _)| json!(code))
                        .ok_or_else(|| format!("{p:?} is not a size"))
                })
                .collect::<Result<_, _>>()?;
            Ok((!codes.is_empty()).then_some(Value::Array(codes)))
        }
        Format::Properties => {
            let known = choices(format).unwrap_or_default();
            let list: Vec<Value> = t
                .split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| {
                    known.iter().find(|(label, _)| label.eq_ignore_ascii_case(p)).and_then(|(_, v)| v.clone()).unwrap_or_else(|| json!(p))
                })
                .collect();
            Ok((!list.is_empty()).then_some(Value::Array(list)))
        }
        Format::Attunement => Ok(match lower.as_str() {
            "" | "no" | "n" | "false" => None,
            "yes" | "y" | "true" => Some(json!(true)),
            _ => Some(json!(t)),
        }),
        _ => match choices(format) {
            Some(options) => options
                .into_iter()
                .find(|(label, _)| label.eq_ignore_ascii_case(t))
                .map(|(_, v)| v)
                .ok_or_else(|| format!("choose one of the options for {t:?}")),
            None => Ok(Some(json!(t))),
        },
    }
}

fn number_and_rest(s: &str) -> (u64, &str) {
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    let n = s[..digits].parse().unwrap_or(1);
    (n, s[digits..].trim())
}

fn singular(unit: &str) -> &str {
    unit.strip_suffix('s').unwrap_or(unit)
}

fn parse_time(t: &str) -> Result<Value, String> {
    let mut out = Vec::new();
    for part in t.split(" or ") {
        let (main, condition) = match part.split_once(',') {
            Some((m, c)) => (m.trim(), Some(c.trim())),
            None => (part.trim(), None),
        };
        let (n, unit) = number_and_rest(main);
        let unit = match singular(&unit.to_lowercase()) {
            "action" => "action",
            "bonus action" | "bonus" => "bonus",
            "reaction" => "reaction",
            "minute" | "min" => "minute",
            "hour" | "hr" => "hour",
            "round" => "round",
            other => return Err(format!("unknown casting time unit {other:?}")),
        };
        let mut o = Map::new();
        o.insert("number".into(), json!(n));
        o.insert("unit".into(), json!(unit));
        if let Some(c) = condition.filter(|c| !c.is_empty()) {
            o.insert("condition".into(), json!(c));
        }
        out.push(Value::Object(o));
    }
    Ok(Value::Array(out))
}

fn parse_range(t: &str) -> Result<Value, String> {
    let simple = |kind: &str| json!({"type": "point", "distance": {"type": kind}});
    Ok(match t {
        "self" => simple("self"),
        "touch" => simple("touch"),
        "sight" => simple("sight"),
        "unlimited" => simple("unlimited"),
        "special" => json!({"type": "special"}),
        _ => {
            if let Some(shape) = t.strip_prefix("self (").and_then(|r| r.strip_suffix(')')) {
                // "15-foot cone", "30-foot-radius sphere", "10-foot emanation"
                let (n, rest) = number_and_rest(shape);
                let rest = rest.trim_start_matches('-').trim_start_matches("foot").trim_start_matches("feet");
                let rest = rest.trim_start_matches('-').trim_start_matches("radius").trim();
                let shape = match rest {
                    "cone" | "cube" | "line" | "sphere" | "hemisphere" | "radius" | "emanation" | "cylinder" => rest,
                    "" => "radius",
                    other => return Err(format!("unknown area {other:?}")),
                };
                return Ok(json!({"type": shape, "distance": {"type": "feet", "amount": n}}));
            }
            let (n, unit) = number_and_rest(t);
            let unit = match singular(unit) {
                "foot" | "feet" | "feet." | "ft" | "ft." => "feet",
                "mile" => "miles",
                other => return Err(format!("unknown range {other:?}")),
            };
            json!({"type": "point", "distance": {"type": unit, "amount": n}})
        }
    })
}

fn parse_components(t: &str, previous: Option<&Value>) -> Result<Value, String> {
    let mut o = Map::new();
    let (head, material) = match t.split_once('(') {
        Some((h, m)) => (h, Some(m.trim_end().trim_end_matches(')').trim())),
        None => (t, None),
    };
    for part in head.split(',').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()) {
        match part.as_str() {
            "v" | "s" | "r" => {
                o.insert(part, json!(true));
            }
            "m" => {
                let prev = previous.and_then(|p| p.get("m"));
                let m = match (material, prev) {
                    // Same text: keep the cost and consumed flags.
                    (Some(text), Some(Value::Object(p))) if p.get("text").and_then(Value::as_str) == Some(text) => Value::Object(p.clone()),
                    (Some(text), _) => json!(text),
                    (None, _) => json!(true),
                };
                o.insert("m".into(), m);
            }
            other => return Err(format!("unknown component {other:?} (use V, S, M (…))")),
        }
    }
    Ok(Value::Object(o))
}

fn parse_duration(t: &str) -> Result<Value, String> {
    let one = match t {
        "instantaneous" | "instant" => json!({"type": "instant"}),
        "special" => json!({"type": "special"}),
        "permanent" => json!({"type": "permanent"}),
        "until dispelled" => json!({"type": "permanent", "ends": ["dispel"]}),
        "until dispelled or triggered" => json!({"type": "permanent", "ends": ["dispel", "trigger"]}),
        _ => {
            let (concentration, rest) = match t.strip_prefix("concentration") {
                Some(r) => (true, r.trim_start_matches(',').trim().trim_start_matches("up to").trim()),
                None => (false, t),
            };
            let (n, unit) = number_and_rest(rest);
            let unit = match singular(unit) {
                u @ ("round" | "minute" | "hour" | "day" | "week" | "year") => u,
                other => return Err(format!("unknown duration {other:?}")),
            };
            let mut o = json!({"type": "timed", "duration": {"type": unit, "amount": n}});
            if concentration {
                o["concentration"] = json!(true);
            }
            o
        }
    };
    Ok(json!([one]))
}

fn parse_classes(t: &str, lib: &Library) -> Result<Value, String> {
    let mut list = Vec::new();
    for name in t.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        // Prefer the 2024 class, then any.
        let found = lib
            .entities()
            .iter()
            .filter(|e| e.kind == Kind::Class && e.name.eq_ignore_ascii_case(name))
            .max_by_key(|e| (e.source == "XPHB", e.source == "PHB"))
            .ok_or_else(|| format!("no class named {name:?}"))?;
        list.push(json!({"name": found.name, "source": found.source}));
    }
    Ok(json!({ "fromClassList": list }))
}

fn parse_coins(t: &str) -> Result<u64, String> {
    let mut total = 0f64;
    for part in t.split([',', '+']).map(str::trim).filter(|p| !p.is_empty()) {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        let n: f64 = digits.parse().map_err(|_| format!("not an amount: {part:?}"))?;
        let mult = match part[digits.len()..].trim() {
            "cp" => 1.0,
            "sp" => 10.0,
            "ep" => 50.0,
            "" | "gp" => 100.0,
            "pp" => 1000.0,
            other => return Err(format!("unknown coin {other:?}")),
        };
        total += n * mult;
    }
    Ok(total.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_read_and_write_as_the_data_does() {
        let lib = Library::new();
        let parsed = |f: Format, t: &str| parse(f, t, None, &lib).unwrap().unwrap();
        // A 2024 background's increases.
        let v = parsed(Format::AbilityIncrease, "+2/+1 or +1/+1/+1 among Int, Wis, Cha");
        assert_eq!(
            v,
            json!([
                {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [2, 1]}}},
                {"choose": {"weighted": {"from": ["int", "wis", "cha"], "weights": [1, 1, 1]}}}
            ])
        );
        assert_eq!(display(Format::AbilityIncrease, Some(&v)), "+2/+1 among Int/Wis/Cha or +1/+1/+1 among Int/Wis/Cha");
        assert_eq!(parsed(Format::AbilityIncrease, "Dexterity +2, Wis +1"), json!([{"dex": 2, "wis": 1}]));
        assert_eq!(
            parsed(Format::AbilityIncrease, "Cha +2; +1 to 2 of Str/Dex/Con"),
            json!([{"cha": 2, "choose": {"from": ["str", "dex", "con"], "count": 2}}])
        );
        assert!(parse(Format::AbilityIncrease, "Luck +2", None, &lib).is_err());

        let skills = Format::Proficiency(Proficiency::Skill);
        let v = parsed(skills, "Insight, Religion");
        assert_eq!(v, json!([{"insight": true, "religion": true}]));
        assert_eq!(display(skills, Some(&v)), "Insight, Religion");
        let v = parsed(skills, "choose 2: Arcana / History / Nature");
        assert_eq!(v, json!([{"choose": {"from": ["arcana", "history", "nature"], "count": 2}}]));
        assert_eq!(parse(skills, &display(skills, Some(&v)), None, &lib).unwrap(), Some(v));
        let langs = Format::Proficiency(Proficiency::Language);
        let v = parsed(langs, "Common, any standard 2");
        assert_eq!(v, json!([{"common": true, "anyStandard": 2}]));
        assert_eq!(display(langs, Some(&v)), "any standard 2, Common");

        assert_eq!(parsed(Format::Size, "Small, Medium"), json!(["S", "M"]));
        assert!(parse(Format::Feats, "Alert", None, &lib).is_err(), "unknown feats are refused");
        assert_eq!(display(Format::Feats, Some(&json!([{"magic initiate; cleric|xphb": true}]))), "Magic Initiate: Cleric");
    }

    fn round_trip(format: Format, text: &str) -> String {
        let lib = Library::new();
        let v = parse(format, text, None, &lib).unwrap();
        display(format, v.as_ref())
    }

    #[test]
    fn spell_fields() {
        assert_eq!(round_trip(Format::Time, "1 bonus action"), "1 bonus action");
        assert_eq!(round_trip(Format::Time, "1 reaction, which you take when hit"), "1 reaction, which you take when hit");
        assert_eq!(round_trip(Format::Time, "10 minutes or 1 action"), "10 minutes or 1 action");
        assert_eq!(round_trip(Format::Range, "90 feet"), "90 feet");
        assert_eq!(round_trip(Format::Range, "Touch"), "Touch");
        assert_eq!(round_trip(Format::Range, "Self (15-foot cone)"), "Self (15-foot cone)");
        assert_eq!(round_trip(Format::Range, "1 mile"), "1 mile");
        assert_eq!(round_trip(Format::Duration, "Concentration, up to 1 minute"), "Concentration, up to 1 minute");
        assert_eq!(round_trip(Format::Duration, "8 hours"), "8 hours");
        assert_eq!(round_trip(Format::Duration, "until dispelled"), "Until dispelled");
        assert_eq!(round_trip(Format::Duration, "Instantaneous"), "Instantaneous");
        assert_eq!(round_trip(Format::Components, "V, S, M (a pinch of salt)"), "V, S, M (a pinch of salt)");
        assert!(parse(Format::Range, "far away", None, &Library::new()).is_err());
    }

    #[test]
    fn components_keep_cost() {
        let prev = json!({"v": true, "m": {"text": "a diamond worth 300+ GP", "cost": 30000, "consume": true}});
        let v = parse(Format::Components, "V, S, M (a diamond worth 300+ GP)", Some(&prev), &Library::new()).unwrap().unwrap();
        assert_eq!(v["m"]["cost"], 30000);
        assert_eq!(v["s"], true);
    }

    #[test]
    fn items_and_choices() {
        assert_eq!(parse_coins("50 gp").unwrap(), 5000);
        assert_eq!(parse_coins("1 gp, 5 sp").unwrap(), 150);
        assert_eq!(round_trip(Format::Coins, "25"), "25 gp");
        assert_eq!(round_trip(Format::Attunement, "by a wizard"), "by a wizard");
        assert_eq!(parse(Format::Attunement, "no", None, &Library::new()).unwrap(), None);
        assert_eq!(parse(Format::School, "evocation", None, &Library::new()).unwrap(), Some(json!("V")));
        assert_eq!(display(Format::ItemType, Some(&json!("WD|XDMG"))), "Wand");
        assert_eq!(format_for(Kind::Item, "value"), Some(Format::Coins));
    }

    #[test]
    fn classes() {
        let mut lib = Library::new();
        lib.load_json(&json!({"class": [{"name": "Wizard", "source": "PHB"}, {"name": "Wizard", "source": "XPHB"}]}));
        let v = parse(Format::Classes, "wizard", None, &lib).unwrap().unwrap();
        assert_eq!(v, json!({"fromClassList": [{"name": "Wizard", "source": "XPHB"}]}));
        assert_eq!(display(Format::Classes, Some(&v)), "Wizard");
        assert!(parse(Format::Classes, "Bard", None, &lib).is_err());
    }
}
