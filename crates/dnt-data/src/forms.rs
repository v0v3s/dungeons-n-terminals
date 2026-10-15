//! Friendly editing of common data fields: each format shows a field as
//! short text ("1 bonus action", "Concentration, up to 1 minute", "50 gp")
//! or a list of choices, and parses the text back into data JSON.

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
            _ => return None,
        },
        (Kind::Feat, "category") => FeatCategory,
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
    }
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
