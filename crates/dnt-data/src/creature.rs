//! A creature's stat block (`monster` in the data format): what its
//! fields say, for the Compendium and for a companion's sheet.

use serde_json::Value;

use crate::doc::{self, Block, Table};
use crate::tags::{self, Span, Style};
use crate::view::{ability_name, title_case};

pub const ABILITIES: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];

/// An ability score's modifier.
pub fn modifier(score: i64) -> i64 {
    (score - 10).div_euclid(2)
}

/// `+2`, `-1`.
pub fn signed(n: i64) -> String {
    if n < 0 {
        n.to_string()
    } else {
        format!("+{n}")
    }
}

/// A score (`str` … `cha`), 10 when the block has none.
pub fn score(d: &Value, ability: &str) -> i64 {
    d.get(ability).and_then(Value::as_i64).unwrap_or(10)
}

pub fn size(code: &str) -> &'static str {
    match code {
        "T" => "Tiny",
        "S" => "Small",
        "M" => "Medium",
        "L" => "Large",
        "H" => "Huge",
        "G" => "Gargantuan",
        _ => "Varies",
    }
}

/// `Medium or Small Beast (Cat)`.
pub fn kind_line(d: &Value) -> String {
    let sizes: Vec<&str> = d.get("size").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(size).collect();
    let mut s = sizes.join(" or ");
    let t = creature_type(d);
    if !t.is_empty() {
        s = format!("{s} {t}").trim().to_string();
    }
    let al = alignment(d.get("alignment"));
    if !al.is_empty() {
        s = format!("{s}, {al}");
    }
    s
}

/// `Beast`, `Fiend (Devil)`, `Celestial or Fey`.
pub fn creature_type(d: &Value) -> String {
    match d.get("type") {
        Some(Value::String(t)) => title_case(t),
        Some(Value::Object(o)) => {
            let base = match o.get("type") {
                Some(Value::String(t)) => title_case(t),
                Some(Value::Object(c)) => c
                    .get("choose")
                    .and_then(Value::as_array)
                    .map(|c| c.iter().filter_map(Value::as_str).map(title_case).collect::<Vec<_>>().join(" or "))
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let tags: Vec<String> = o
                .get("tags")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|t| t.as_str().map(str::to_string).or_else(|| t.get("tag").and_then(Value::as_str).map(str::to_string)))
                .map(|t| title_case(&t))
                .collect();
            if tags.is_empty() {
                base
            } else {
                format!("{base} ({})", tags.join(", "))
            }
        }
        _ => String::new(),
    }
}

fn alignment(v: Option<&Value>) -> String {
    let Some(list) = v.and_then(Value::as_array) else { return String::new() };
    let word = |c: &str| match c {
        "L" => "Lawful",
        "N" => "Neutral",
        "C" => "Chaotic",
        "G" => "Good",
        "E" => "Evil",
        "U" => "Unaligned",
        "A" => "Any Alignment",
        _ => "",
    };
    let codes: Vec<&str> = list.iter().filter_map(Value::as_str).collect();
    if codes.len() == list.len() {
        return match codes.as_slice() {
            ["N"] => "Neutral".into(),
            _ => codes.iter().map(|c| word(c)).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" "),
        };
    }
    list.iter()
        .filter_map(|a| {
            a.get("alignment").map(|x| alignment(Some(x))).or_else(|| a.get("special").and_then(Value::as_str).map(str::to_string))
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// The Armor Class: its number (when it has one) and the line to show.
pub fn armor_class(d: &Value) -> (Option<i64>, String) {
    let list = d.get("ac").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut first = None;
    let parts: Vec<String> = list
        .iter()
        .map(|a| match a {
            Value::Number(n) => {
                first = first.or(n.as_i64());
                n.to_string()
            }
            Value::Object(o) => {
                if let Some(s) = o.get("special").and_then(Value::as_str) {
                    return s.to_string();
                }
                let n = o.get("ac").and_then(Value::as_i64).unwrap_or(10);
                first = first.or(Some(n));
                let from: Vec<String> =
                    o.get("from").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(plain_text).collect();
                let mut s = n.to_string();
                if !from.is_empty() {
                    s = format!("{s} ({})", from.join(", "));
                }
                if let Some(c) = o.get("condition").and_then(Value::as_str) {
                    s = format!("{s} {}", plain_text(c));
                }
                s
            }
            _ => String::new(),
        })
        .collect();
    (first, parts.join(", "))
}

/// Hit points: the average (when there is one), the dice formula, and the line.
pub fn hit_points(d: &Value) -> (Option<i64>, Option<String>, String) {
    let hp = d.get("hp");
    if let Some(s) = hp.and_then(|h| h.get("special")).and_then(Value::as_str) {
        return (s.trim().parse().ok(), None, s.to_string());
    }
    let avg = hp.and_then(|h| h.get("average")).and_then(Value::as_i64);
    let formula = hp.and_then(|h| h.get("formula")).and_then(Value::as_str).map(str::to_string);
    let line = match (&avg, &formula) {
        (Some(a), Some(f)) => format!("{a} ({f})"),
        (Some(a), None) => a.to_string(),
        (None, Some(f)) => f.clone(),
        _ => String::new(),
    };
    (avg, formula, line)
}

/// Speeds by mode (`walk`, `fly`, …) in feet, and the line to show.
pub fn speeds(d: &Value) -> (Vec<(String, i64)>, String) {
    let mut out = Vec::new();
    let mut parts = Vec::new();
    let Some(o) = d.get("speed").and_then(Value::as_object) else { return (out, String::new()) };
    for mode in ["walk", "burrow", "climb", "fly", "swim"] {
        let Some(v) = o.get(mode) else { continue };
        let (n, cond) = match v {
            Value::Number(n) => (n.as_i64(), None),
            Value::Object(x) => (x.get("number").and_then(Value::as_i64), x.get("condition").and_then(Value::as_str)),
            _ => (None, None),
        };
        let Some(n) = n else { continue };
        out.push((mode.to_string(), n));
        let mut s = if mode == "walk" { format!("{n} ft.") } else { format!("{} {n} ft.", title_case(mode)) };
        if mode == "fly" && o.get("canHover").and_then(Value::as_bool) == Some(true) {
            s.push_str(" (hover)");
        }
        if let Some(c) = cond {
            s = format!("{s} {}", plain_text(c));
        }
        parts.push(s);
    }
    (out, parts.join(", "))
}

/// The challenge rating, as written (`1/4`, `10`).
pub fn challenge(d: &Value) -> Option<String> {
    match d.get("cr")? {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("cr").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

/// The challenge rating as a number (`1/4` → 0.25).
pub fn challenge_value(d: &Value) -> Option<f64> {
    let cr = challenge(d)?;
    match cr.split_once('/') {
        Some((a, b)) => Some(a.parse::<f64>().ok()? / b.parse::<f64>().ok()?),
        None => cr.parse().ok(),
    }
}

/// The proficiency bonus for a challenge rating.
pub fn proficiency(d: &Value) -> i64 {
    let cr = challenge_value(d).unwrap_or(0.0);
    (2 + ((cr.max(1.0) - 1.0) / 4.0).floor() as i64).max(2)
}

/// Bonuses a block lists (`save` or `skill`): name → bonus.
pub fn bonuses(d: &Value, key: &str) -> Vec<(String, i64)> {
    d.get(key)
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.trim().trim_start_matches('+').parse().ok()?)))
        .collect()
}

/// Damage types or conditions a block lists (`resist`, `immune`,
/// `vulnerable`, `conditionImmune`), as a line.
pub fn damage_list(v: Option<&Value>, key: &str) -> String {
    let Some(list) = v.and_then(Value::as_array) else { return String::new() };
    let mut out = Vec::new();
    for x in list {
        match x {
            Value::String(s) => out.push(title_case(s)),
            Value::Object(o) => {
                if let Some(s) = o.get("special").and_then(Value::as_str) {
                    out.push(plain_text(s));
                    continue;
                }
                let mut s = damage_list(o.get(key).or_else(|| o.get("conditionImmune")), key);
                for side in ["preNote", "note"] {
                    if let Some(n) = o.get(side).and_then(Value::as_str) {
                        s = if side == "preNote" { format!("{n} {s}") } else { format!("{s} {n}") };
                    }
                }
                out.push(plain_text(&s));
            }
            _ => {}
        }
    }
    out.join(", ")
}

fn plain_text(s: &str) -> String {
    tags::parse(s).into_iter().map(|sp| sp.text).collect()
}

/// A run of text lines: `Label: value`.
fn prop(label: &str, value: Vec<Span>) -> Vec<Span> {
    let mut spans = vec![Span::styled(format!("{label}: "), Style { bold: true, ..Style::default() })];
    spans.extend(value);
    spans
}

/// The stat block, as a document.
pub fn document(d: &Value) -> Vec<Block> {
    let mut out = Vec::new();
    let mut lines = Vec::new();
    let line = kind_line(d);
    if !line.is_empty() {
        lines.push(vec![Span::styled(line, Style { italic: true, ..Style::default() })]);
    }
    let (_, ac) = armor_class(d);
    if !ac.is_empty() {
        lines.push(prop("AC", tags::parse(&ac)));
    }
    let (_, _, hp) = hit_points(d);
    if !hp.is_empty() {
        lines.push(prop("HP", tags::parse(&hp)));
    }
    let (_, speed) = speeds(d);
    if !speed.is_empty() {
        lines.push(prop("Speed", vec![Span::plain(speed)]));
    }
    if !lines.is_empty() {
        out.push(Block::Properties(lines));
    }

    // One row per ability: narrow enough for a phone (no sideways scroll).
    let saves = bonuses(d, "save");
    let rows = ABILITIES
        .iter()
        .map(|a| {
            let sc = score(d, a);
            let m = modifier(sc);
            let save = saves.iter().find(|(k, _)| k == a).map_or(m, |(_, b)| *b);
            vec![
                vec![Span::plain(ability_name(a)[..3].to_uppercase())],
                vec![Span::plain(sc.to_string())],
                vec![Span::plain(signed(m))],
                vec![Span::plain(signed(save))],
            ]
        })
        .collect();
    out.push(Block::Table(Table {
        caption: None,
        header: ["", "Score", "Mod", "Save"].iter().map(|h| vec![Span::plain(*h)]).collect(),
        rows,
    }));

    let mut lines = Vec::new();
    let skills: Vec<String> = bonuses(d, "skill").iter().map(|(k, b)| format!("{} {}", title_case(k), signed(*b))).collect();
    if !skills.is_empty() {
        lines.push(prop("Skills", vec![Span::plain(skills.join(", "))]));
    }
    for (label, key) in [
        ("Vulnerabilities", "vulnerable"),
        ("Resistances", "resist"),
        ("Immunities", "immune"),
        ("Condition Immunities", "conditionImmune"),
    ] {
        let s = damage_list(d.get(key), key);
        if !s.is_empty() {
            lines.push(prop(label, vec![Span::plain(s)]));
        }
    }
    let mut senses: Vec<String> =
        d.get("senses").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(plain_text).collect();
    if let Some(p) = d.get("passive").and_then(|p| p.as_i64().map(|n| n.to_string()).or_else(|| p.as_str().map(str::to_string))) {
        senses.push(format!("Passive Perception {p}"));
    }
    if !senses.is_empty() {
        lines.push(prop("Senses", vec![Span::plain(senses.join(", "))]));
    }
    let languages: Vec<String> =
        d.get("languages").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(plain_text).collect();
    lines.push(prop("Languages", vec![Span::plain(if languages.is_empty() { "None".into() } else { languages.join(", ") })]));
    if let Some(cr) = challenge(d) {
        lines.push(prop("CR", vec![Span::plain(format!("{cr} (PB {})", signed(proficiency(d))))]));
    }
    out.push(Block::Properties(lines));

    for (title, key) in [
        ("Traits", "trait"),
        ("Actions", "action"),
        ("Bonus Actions", "bonus"),
        ("Reactions", "reaction"),
        ("Legendary Actions", "legendary"),
    ] {
        let mut entries: Vec<Value> = d.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        if key == "trait" {
            entries.extend(d.get("spellcasting").and_then(Value::as_array).into_iter().flatten().map(spellcasting));
        }
        if entries.is_empty() {
            continue;
        }
        out.push(Block::Heading { level: 1, text: vec![Span::plain(title)] });
        for e in entries {
            out.extend(run_in(&e));
        }
    }
    if let Some(entries) = d.get("entries") {
        out.extend(doc::from_entries(entries));
    }
    out
}

/// A trait or an action: its name run in, bold, before its text
/// (`Pack Tactics. The wolf has…`), so it reads apart from the headings.
fn run_in(e: &Value) -> Vec<Block> {
    let Some(name) = e.get("name").and_then(Value::as_str) else { return doc::from_entries(&Value::Array(vec![e.clone()])) };
    let entries = e.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut lead =
        vec![Span::styled(format!("{}. ", plain_text(name).trim_end_matches('.')), Style { bold: true, italic: true, ..Style::default() })];
    let mut rest = entries.as_slice();
    if let Some(Value::String(first)) = entries.first() {
        lead.extend(tags::parse(first));
        rest = &entries[1..];
    }
    let mut out = vec![Block::Paragraph(lead)];
    if !rest.is_empty() {
        out.extend(doc::from_entries(&Value::Array(rest.to_vec())));
    }
    out
}

/// A `spellcasting` block as plain entries.
fn spellcasting(s: &Value) -> Value {
    let mut entries: Vec<Value> = s.get("headerEntries").and_then(Value::as_array).cloned().unwrap_or_default();
    let list = |v: &Value| v.as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    if let Some(w) = s.get("will") {
        entries.push(Value::String(format!("At will: {}", list(w))));
    }
    for (key, per) in [("daily", "day"), ("rest", "rest")] {
        if let Some(o) = s.get(key).and_then(Value::as_object) {
            for (n, v) in o {
                let each = if n.ends_with('e') { " each" } else { "" };
                entries.push(Value::String(format!("{}/{per}{each}: {}", n.trim_end_matches('e'), list(v))));
            }
        }
    }
    if let Some(o) = s.get("spells").and_then(Value::as_object) {
        for (level, v) in o {
            let slots = v.get("slots").and_then(Value::as_u64).map(|n| format!(" ({n} slots)")).unwrap_or_default();
            let label = if level == "0" { "Cantrips".to_string() } else { format!("Level {level}{slots}") };
            entries.push(Value::String(format!("{label}: {}", v.get("spells").map(list).unwrap_or_default())));
        }
    }
    entries.extend(s.get("footerEntries").and_then(Value::as_array).cloned().unwrap_or_default());
    serde_json::json!({"type": "entries", "name": s.get("name").and_then(Value::as_str).unwrap_or("Spellcasting"), "entries": entries})
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wolf() -> Value {
        json!({
            "name": "Wolf", "source": "T", "size": ["M"], "type": "beast", "alignment": ["U"],
            "ac": [{"ac": 12, "from": ["natural armor"]}], "hp": {"average": 11, "formula": "2d8 + 2"},
            "speed": {"walk": 40, "swim": {"number": 20, "condition": "(in water)"}},
            "str": 14, "dex": 15, "con": 12, "int": 3, "wis": 12, "cha": 6,
            "skill": {"perception": "+5", "stealth": "+4"}, "senses": ["darkvision 60 ft."], "passive": 15, "cr": "1/4",
            "action": [{"name": "Bite", "entries": ["{@atk mw} {@hit 4} to hit, reach 5 ft. {@h}{@damage 1d6 + 2} Piercing damage."]}]
        })
    }

    #[test]
    fn reads_the_numbers() {
        let w = wolf();
        assert_eq!(kind_line(&w), "Medium Beast, Unaligned");
        assert_eq!(armor_class(&w), (Some(12), "12 (natural armor)".into()));
        assert_eq!(hit_points(&w), (Some(11), Some("2d8 + 2".into()), "11 (2d8 + 2)".into()));
        assert_eq!(speeds(&w).1, "40 ft., Swim 20 ft. (in water)");
        assert_eq!((challenge_value(&w), proficiency(&w)), (Some(0.25), 2));
        assert_eq!(bonuses(&w, "skill"), [("perception".to_string(), 5), ("stealth".to_string(), 4)]);
        assert_eq!(modifier(15), 2);
        assert_eq!(modifier(3), -4);
    }

    #[test]
    fn renders_a_stat_block() {
        let text = format!("{:?}", document(&wolf()));
        for want in ["Medium Beast", "natural armor", "Passive Perception 15", "Actions", "Bite. ", "(PB +2)", "Score"] {
            assert!(text.contains(want), "{want}: {text}");
        }
    }
}
