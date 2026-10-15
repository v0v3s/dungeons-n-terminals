//! Builds a readable document for an entity: a kind-specific header with key
//! properties, followed by its entries.

use serde_json::Value;

use crate::doc::{self, Block, Table};
use crate::kind::Kind;
use crate::library::{EntityId, Library, SourceFilter};
use crate::tags::{self, Span, Style};

/// The full document for an entity. Related entities (e.g. a class's
/// subclasses) are limited to those `filter` allows.
pub fn document(lib: &Library, id: EntityId, filter: &SourceFilter) -> Vec<Block> {
    let e = lib.entity(id);
    let d = &e.data;
    let mut out = Vec::new();

    let mut props: Vec<(&str, Vec<Span>)> = Vec::new();
    let mut lines = Vec::new();
    match e.kind {
        Kind::Spell => spell_props(lib, id, filter, &mut props),
        Kind::Item | Kind::BaseItem | Kind::ItemGroup | Kind::MagicVariant => item_props(d, &mut props),
        Kind::Race | Kind::Subrace => race_props(d, &mut props),
        Kind::Background => background_props(d, &mut props),
        Kind::Feat | Kind::OptionalFeature => feat_props(d, &mut props),
        Kind::Class => class_props(d, &mut props),
        Kind::Subclass | Kind::ClassFeature | Kind::SubclassFeature => {
            if let (Some(class), Some(src)) = (str_of(d, "className"), str_of(d, "classSource")) {
                props.push(("Class", tags::parse(&format!("{{@class {class}|{src}}}"))));
            }
            if let Some(level) = d.get("level").and_then(Value::as_u64) {
                props.push(("Level", vec![Span::plain(level.to_string())]));
            }
        }
        _ => {}
    }
    for (label, value) in props {
        if value.is_empty() {
            continue;
        }
        let mut spans = vec![Span::styled(
            format!("{label}: "),
            Style {
                bold: true,
                ..Style::default()
            },
        )];
        spans.extend(value);
        lines.push(spans);
    }
    if !lines.is_empty() {
        out.push(Block::Properties(lines));
        out.push(Block::Rule);
    }

    let entries = d.get("entries").or_else(|| d.get("inherits").and_then(|i| i.get("entries")));
    if let Some(entries) = entries {
        out.extend(doc::from_entries(entries));
    }
    if let Some(higher) = d.get("entriesHigherLevel") {
        out.extend(doc::from_entries(higher));
    }

    match e.kind {
        Kind::Class => {
            out.push(Block::Table(class_table(d)));
            let subs: Vec<EntityId> = lib
                .subclasses_of(id)
                .into_iter()
                .filter(|&s| filter.source_enabled(&lib.entity(s).source))
                .collect();
            if !subs.is_empty() {
                let title = str_of(d, "subclassTitle").unwrap_or("Subclass");
                let plural = if title.ends_with('s') { "es" } else { "s" };
                out.push(Block::Heading {
                    level: 1,
                    text: vec![Span::plain(format!("{title}{plural}"))],
                });
                let items = subs
                    .into_iter()
                    .map(|s| {
                        let sub = lib.entity(s);
                        let short = str_of(&sub.data, "shortName").unwrap_or(&sub.name);
                        let tag = format!("{{@subclass {short}|{}|{}|{}|{}}}", e.name, e.source, sub.source, sub.name);
                        let mut spans = tags::parse(&tag);
                        spans.push(source_span(&sub.source));
                        vec![Block::Paragraph(spans)]
                    })
                    .collect();
                out.push(Block::List(items));
            }
        }
        Kind::Subclass => {
            if let Some(features) = d.get("subclassFeatures").and_then(Value::as_array) {
                out.push(Block::Heading {
                    level: 1,
                    text: vec![Span::plain("Features")],
                });
                let items = features
                    .iter()
                    .filter_map(feature_ref)
                    .map(|r| {
                        // name|className|classSource|subclassShortName|subclassSource|level
                        let level = r.split('|').nth(5).unwrap_or("");
                        let mut spans = vec![Span::plain(format!("Level {level}: "))];
                        spans.extend(tags::parse(&format!("{{@subclassFeature {r}}}")));
                        vec![Block::Paragraph(spans)]
                    })
                    .collect();
                out.push(Block::List(items));
            }
        }
        _ => {}
    }
    out
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn source_span(source: &str) -> Span {
    Span::styled(
        format!("  {source}"),
        Style {
            muted: true,
            ..Style::default()
        },
    )
}

fn feature_ref(v: &Value) -> Option<&str> {
    v.as_str()
        .or_else(|| str_of(v, "classFeature"))
        .or_else(|| str_of(v, "subclassFeature"))
}

fn plain(s: impl Into<String>) -> Vec<Span> {
    vec![Span::plain(s)]
}

fn ordinal(n: u64) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

pub fn ability_name(code: &str) -> &'static str {
    match code {
        "str" => "Strength",
        "dex" => "Dexterity",
        "con" => "Constitution",
        "int" => "Intelligence",
        "wis" => "Wisdom",
        "cha" => "Charisma",
        _ => "?",
    }
}

pub fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

// ---- spells ----

fn spell_props(lib: &Library, id: EntityId, filter: &SourceFilter, props: &mut Vec<(&'static str, Vec<Span>)>) {
    let d = &lib.entity(id).data;
    let level = d.get("level").and_then(Value::as_u64).unwrap_or(0);
    let school = spell_school(str_of(d, "school").unwrap_or(""));
    let ritual = d.get("meta").and_then(|m| m.get("ritual")).and_then(Value::as_bool) == Some(true);
    let mut header = if level == 0 {
        format!("{school} Cantrip")
    } else {
        format!("{}-level {school}", ordinal(level))
    };
    if ritual {
        header.push_str(" (ritual)");
    }
    props.push(("Level", plain(header)));
    props.push(("Casting Time", plain(spell_time(d))));
    props.push(("Range", plain(spell_range(d))));
    props.push(("Components", spell_components(d)));
    props.push(("Duration", plain(spell_duration(d))));

    let classes: Vec<_> = lib.spell_classes(id).iter().filter(|c| filter.source_enabled(&c.source)).collect();
    if !classes.is_empty() {
        let mut spans = Vec::new();
        for (i, c) in classes.iter().enumerate() {
            if i > 0 {
                spans.push(Span::plain(", "));
            }
            spans.extend(tags::parse(&format!("{{@class {}|{}}}", c.name, c.source)));
        }
        props.push(("Classes", spans));
    }
}

pub fn spell_school(code: &str) -> &'static str {
    match code {
        "A" => "Abjuration",
        "C" => "Conjuration",
        "D" => "Divination",
        "E" => "Enchantment",
        "V" => "Evocation",
        "I" => "Illusion",
        "N" => "Necromancy",
        "T" => "Transmutation",
        "P" => "Psionic",
        _ => "Unknown school",
    }
}

fn spell_time(d: &Value) -> String {
    d.get("time")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| {
            let n = t.get("number").and_then(Value::as_u64).unwrap_or(1);
            let unit = match str_of(t, "unit").unwrap_or("") {
                "bonus" => "bonus action",
                u => u,
            };
            let plural = if n == 1 { "" } else { "s" };
            match str_of(t, "condition") {
                Some(c) => format!("{n} {unit}{plural}, {}", tags::strip(c)),
                None => format!("{n} {unit}{plural}"),
            }
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

fn distance(d: &Value) -> String {
    let amount = d.get("amount").and_then(Value::as_u64);
    match (str_of(d, "type").unwrap_or(""), amount) {
        ("self", _) => "Self".into(),
        ("touch", _) => "Touch".into(),
        ("sight", _) => "Sight".into(),
        ("unlimited", _) => "Unlimited".into(),
        ("feet", Some(1)) => "1 foot".into(),
        ("miles", Some(1)) => "1 mile".into(),
        (unit, Some(n)) => format!("{n} {unit}"),
        (unit, None) => title_case(unit),
    }
}

fn spell_range(d: &Value) -> String {
    let Some(range) = d.get("range") else { return String::new() };
    let dist = range.get("distance").map(distance).unwrap_or_default();
    match str_of(range, "type").unwrap_or("") {
        "point" => dist,
        "special" => "Special".into(),
        shape => {
            let size = range.get("distance").and_then(|x| x.get("amount")).and_then(Value::as_u64).unwrap_or(0);
            format!("Self ({size}-foot {shape})")
        }
    }
}

fn spell_components(d: &Value) -> Vec<Span> {
    let Some(c) = d.get("components") else { return Vec::new() };
    let mut parts = Vec::new();
    for (key, label) in [("v", "V"), ("s", "S"), ("r", "R")] {
        if c.get(key).and_then(Value::as_bool) == Some(true) {
            parts.push(label.to_string());
        }
    }
    match c.get("m") {
        Some(Value::String(m)) => parts.push(format!("M ({m})")),
        Some(Value::Object(m)) => parts.push(format!("M ({})", m.get("text").and_then(Value::as_str).unwrap_or(""))),
        Some(Value::Bool(true)) => parts.push("M".into()),
        _ => {}
    }
    tags::parse(&parts.join(", "))
}

fn spell_duration(d: &Value) -> String {
    d.get("duration")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|dur| match str_of(dur, "type").unwrap_or("") {
            "instant" => "Instantaneous".to_string(),
            "permanent" => {
                let ends: Vec<&str> = dur
                    .get("ends")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                if ends.is_empty() {
                    "Permanent".into()
                } else {
                    format!("Until {}", ends.join(" or ").replace("trigger", "triggered"))
                }
            }
            "special" => "Special".into(),
            "timed" => {
                let inner = dur.get("duration");
                let n = inner.and_then(|i| i.get("amount")).and_then(Value::as_u64).unwrap_or(1);
                let unit = inner.and_then(|i| str_of(i, "type")).unwrap_or("");
                let plural = if n == 1 { "" } else { "s" };
                if dur.get("concentration").and_then(Value::as_bool) == Some(true) {
                    format!("Concentration, up to {n} {unit}{plural}")
                } else {
                    format!("{n} {unit}{plural}")
                }
            }
            other => title_case(other),
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

// ---- items ----

fn item_props(d: &Value, props: &mut Vec<(&'static str, Vec<Span>)>) {
    let d = match d.get("inherits") {
        Some(inherits) => inherits,
        None => d,
    };
    let mut kind = Vec::new();
    if let Some(t) = str_of(d, "type") {
        kind.push(item_type(t.split('|').next().unwrap_or(t)).to_string());
    } else if d.get("wondrous").and_then(Value::as_bool) == Some(true) {
        kind.push("Wondrous item".into());
    }
    if let Some(r) = str_of(d, "rarity").filter(|r| !matches!(*r, "none" | "unknown")) {
        kind.push(r.to_string());
    }
    match d.get("reqAttune") {
        Some(Value::Bool(true)) => kind.push("(requires attunement)".into()),
        Some(Value::String(s)) => kind.push(format!("(requires attunement {})", tags::strip(s))),
        _ => {}
    }
    props.push(("Item", plain(kind.join(", "))));

    if let Some(dmg) = str_of(d, "dmg1") {
        let mut s = format!("{dmg} {}", damage_type(str_of(d, "dmgType").unwrap_or("")));
        if let Some(v) = str_of(d, "dmg2") {
            s.push_str(&format!(" (versatile {v})"));
        }
        props.push(("Damage", plain(s)));
    }
    if let Some(ac) = d.get("ac").and_then(Value::as_i64) {
        props.push(("AC", plain(ac.to_string())));
    }
    if let Some(list) = d.get("property").and_then(Value::as_array) {
        let names: Vec<&str> = list
            .iter()
            .filter_map(|p| p.as_str().or_else(|| str_of(p, "uid")))
            .map(|p| item_property(p.split('|').next().unwrap_or(p)))
            .collect();
        props.push(("Properties", plain(names.join(", "))));
    }
    if let Some(list) = d.get("mastery").and_then(Value::as_array) {
        let mut spans = Vec::new();
        for (i, m) in list.iter().filter_map(|m| m.as_str().or_else(|| str_of(m, "uid"))).enumerate() {
            if i > 0 {
                spans.push(Span::plain(", "));
            }
            spans.extend(tags::parse(&format!("{{@itemMastery {m}}}")));
        }
        props.push(("Mastery", spans));
    }
    if let Some(w) = d.get("weight").and_then(Value::as_f64) {
        props.push(("Weight", plain(format!("{w} lb."))));
    }
    if let Some(cp) = d.get("value").and_then(Value::as_u64) {
        props.push(("Value", plain(format_coins(cp))));
    }
}

fn format_coins(cp: u64) -> String {
    if cp >= 100 && cp.is_multiple_of(100) {
        format!("{} gp", cp / 100)
    } else if cp >= 10 && cp.is_multiple_of(10) {
        format!("{} sp", cp / 10)
    } else {
        format!("{cp} cp")
    }
}

pub fn item_type(code: &str) -> &'static str {
    match code {
        "A" => "Ammunition",
        "AF" => "Ammunition (futuristic)",
        "AT" => "Artisan's tools",
        "EXP" => "Explosive",
        "FD" => "Food and drink",
        "G" => "Adventuring gear",
        "GS" => "Gaming set",
        "HA" => "Heavy armor",
        "INS" => "Musical instrument",
        "LA" => "Light armor",
        "M" => "Melee weapon",
        "MA" => "Medium armor",
        "MNT" => "Mount",
        "P" => "Potion",
        "R" => "Ranged weapon",
        "RD" => "Rod",
        "RG" => "Ring",
        "S" => "Shield",
        "SC" => "Scroll",
        "SCF" => "Spellcasting focus",
        "T" => "Tools",
        "TAH" => "Tack and harness",
        "TG" => "Trade good",
        "VEH" | "SHP" | "AIR" | "SPC" => "Vehicle",
        "WD" => "Wand",
        "$" | "$A" | "$C" | "$G" => "Treasure",
        _ => "Item",
    }
}

fn item_property(code: &str) -> &'static str {
    match code {
        "A" | "AF" => "Ammunition",
        "BF" => "Burst fire",
        "F" => "Finesse",
        "H" => "Heavy",
        "L" => "Light",
        "LD" => "Loading",
        "R" => "Reach",
        "RLD" => "Reload",
        "S" => "Special",
        "T" => "Thrown",
        "2H" => "Two-handed",
        "V" => "Versatile",
        _ => "Other",
    }
}

pub fn damage_type(code: &str) -> &'static str {
    match code {
        "A" => "acid",
        "B" => "bludgeoning",
        "C" => "cold",
        "F" => "fire",
        "O" => "force",
        "L" => "lightning",
        "N" => "necrotic",
        "P" => "piercing",
        "I" => "poison",
        "Y" => "psychic",
        "R" => "radiant",
        "S" => "slashing",
        "T" => "thunder",
        _ => "",
    }
}

// ---- species / backgrounds / feats ----

fn race_props(d: &Value, props: &mut Vec<(&'static str, Vec<Span>)>) {
    if let Some(sizes) = d.get("size").and_then(Value::as_array) {
        let s: Vec<&str> = sizes
            .iter()
            .filter_map(Value::as_str)
            .map(|s| match s {
                "T" => "Tiny",
                "S" => "Small",
                "M" => "Medium",
                "L" => "Large",
                "H" => "Huge",
                "G" => "Gargantuan",
                _ => "Varies",
            })
            .collect();
        props.push(("Size", plain(s.join(" or "))));
    }
    match d.get("speed") {
        Some(Value::Number(n)) => props.push(("Speed", plain(format!("{n} ft.")))),
        Some(Value::Object(o)) => {
            let s: Vec<String> = o
                .iter()
                .filter_map(|(k, v)| {
                    let n = v.as_u64().or_else(|| (v.as_bool() == Some(true)).then_some(0))?;
                    Some(match (k.as_str(), n) {
                        ("walk", n) => format!("{n} ft."),
                        (k, 0) => format!("{k} equal to walking speed"),
                        (k, n) => format!("{k} {n} ft."),
                    })
                })
                .collect();
            props.push(("Speed", plain(s.join(", "))));
        }
        _ => {}
    }
    if let Some(dv) = d.get("darkvision").and_then(Value::as_u64) {
        props.push(("Darkvision", plain(format!("{dv} ft."))));
    }
    if let Some(ab) = d.get("ability").and_then(Value::as_array) {
        let s: Vec<String> = ab.iter().map(ability_bonus).filter(|s| !s.is_empty()).collect();
        props.push(("Ability Scores", plain(s.join("; or "))));
    }
}

fn ability_bonus(v: &Value) -> String {
    let Some(o) = v.as_object() else { return String::new() };
    let mut parts: Vec<String> = o
        .iter()
        .filter_map(|(k, n)| Some(format!("{} {:+}", title_case(k), n.as_i64()?)))
        .collect();
    if let Some(choose) = o.get("choose") {
        if let Some(from) = choose.get("from").and_then(Value::as_array) {
            let count = choose.get("count").and_then(Value::as_u64).unwrap_or(1);
            let amount = choose.get("amount").and_then(Value::as_i64).unwrap_or(1);
            let from: Vec<String> = from.iter().filter_map(Value::as_str).map(title_case).collect();
            parts.push(format!("choose {count} of {} {amount:+}", from.join(", ")));
        } else if let Some(w) = choose.get("weighted") {
            let from: Vec<String> = w
                .get("from")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(title_case)
                .collect();
            let weights: Vec<String> = w
                .get("weights")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64)
                .map(|n| format!("{n:+}"))
                .collect();
            parts.push(format!("{} among {}", weights.join("/"), from.join(", ")));
        }
    }
    parts.join(", ")
}

/// Render `[{a: true, b: true}, {choose: {from, count}}]` style proficiency lists.
fn proficiency_list(v: &Value) -> String {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|alt| {
            let Some(o) = alt.as_object() else {
                return alt.as_str().map(tags::strip).unwrap_or_default();
            };
            let mut parts: Vec<String> = o
                .iter()
                .filter(|(_, v)| v.as_bool() == Some(true))
                .map(|(k, _)| title_case(&tags::strip(k.split('|').next().unwrap_or(k))))
                .collect();
            if let Some(choose) = o.get("choose") {
                let from: Vec<String> = choose
                    .get("from")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(title_case)
                    .collect();
                let n = choose.get("count").and_then(Value::as_u64).unwrap_or(1);
                parts.push(format!("choose {n} from {}", from.join(", ")));
            }
            for (k, n) in o.iter().filter_map(|(k, v)| Some((k, v.as_u64()?))) {
                parts.push(format!("{n} × {}", title_case(k)));
            }
            parts.join(", ")
        })
        .collect::<Vec<_>>()
        .join("; or ")
}

fn background_props(d: &Value, props: &mut Vec<(&'static str, Vec<Span>)>) {
    if let Some(ab) = d.get("ability").and_then(Value::as_array) {
        let s: Vec<String> = ab.iter().map(ability_bonus).filter(|s| !s.is_empty()).collect();
        props.push(("Ability Scores", plain(s.join("; or "))));
    }
    for (key, label) in [
        ("skillProficiencies", "Skills"),
        ("toolProficiencies", "Tools"),
        ("languageProficiencies", "Languages"),
    ] {
        if let Some(v) = d.get(key) {
            props.push((label, plain(proficiency_list(v))));
        }
    }
    if let Some(feats) = d.get("feats").and_then(Value::as_array) {
        let mut spans = Vec::new();
        for f in feats.iter().filter_map(Value::as_object).flat_map(|o| o.keys()) {
            if f.starts_with("any") {
                continue;
            }
            if !spans.is_empty() {
                spans.push(Span::plain(", "));
            }
            // Keys like `magic initiate; cleric|xphb` name a feat version
            // (expanded from `_versions`); display it as "Magic Initiate (Cleric)".
            let (name, source) = f.split_once('|').unwrap_or((f, ""));
            let display = title_case(&name.replace("; ", " (")) + if name.contains(';') { ")" } else { "" };
            spans.extend(tags::parse(&format!("{{@feat {name}|{source}|{display}}}")));
        }
        props.push(("Feat", spans));
    }
}

fn feat_props(d: &Value, props: &mut Vec<(&'static str, Vec<Span>)>) {
    if let Some(cat) = str_of(d, "category") {
        props.push(("Category", plain(feat_category(cat))));
    }
    if let Some(types) = d.get("featureType").and_then(Value::as_array) {
        let s: Vec<&str> = types.iter().filter_map(Value::as_str).map(feature_type).collect();
        props.push(("Type", plain(s.join(", "))));
    }
    if let Some(pre) = d.get("prerequisite").and_then(Value::as_array) {
        props.push(("Prerequisite", tags::parse(&prerequisites(pre))));
    }
}

pub fn feat_category(code: &str) -> &str {
    match code {
        "O" => "Origin",
        "G" => "General",
        "FS" | "FS:P" | "FS:R" => "Fighting Style",
        "EB" => "Epic Boon",
        "D" => "Dragonmark",
        other => other,
    }
}

pub fn feature_type(code: &str) -> &str {
    match code {
        "EI" => "Eldritch Invocation",
        "MM" => "Metamagic",
        "MV:B" => "Battle Master Maneuver",
        "MV" => "Maneuver",
        "FS:F" | "FS:B" | "FS:P" | "FS:R" => "Fighting Style",
        "AI" => "Artificer Infusion",
        "AS" | "AS:V1-UA" | "AS:V2-UA" => "Arcane Shot",
        "ED" => "Elemental Discipline",
        "PB" => "Pact Boon",
        "RN" => "Rune Knight Rune",
        "OTH" => "Other",
        other => other,
    }
}

fn prerequisites(list: &[Value]) -> String {
    list.iter()
        .map(|p| {
            let Some(o) = p.as_object() else { return String::new() };
            let mut parts = Vec::new();
            for (k, v) in o {
                let s = match k.as_str() {
                    "level" => match v {
                        Value::Number(n) => format!("Level {n}"),
                        Value::Object(l) => {
                            let lvl = l.get("level").and_then(Value::as_u64).unwrap_or(0);
                            match l.get("class").and_then(|c| c.get("name")).and_then(Value::as_str) {
                                Some(c) => format!("Level {lvl} {c}"),
                                None => format!("Level {lvl}"),
                            }
                        }
                        _ => continue,
                    },
                    "ability" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_object)
                        .flat_map(|a| a.iter())
                        .map(|(ab, n)| format!("{} {}+", ability_name(ab), n))
                        .collect::<Vec<_>>()
                        .join(" or "),
                    "race" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|r| str_of(r, "name"))
                        .map(title_case)
                        .collect::<Vec<_>>()
                        .join(" or "),
                    "feat" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|f| format!("{{@feat {f}}}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    "spell" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|s| {
                            let (name, suffix) = s.split_once('#').unwrap_or((s, ""));
                            let cantrip = if suffix == "c" { " cantrip" } else { "" };
                            if name.contains('/') {
                                // Free-text alternatives such as `hex/curse`.
                                format!("{}{cantrip}", title_case(&name.replace('/', " or ")))
                            } else {
                                format!("{{@spell {name}}}{cantrip}")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    "pact" => format!("Pact of the {}", v.as_str().unwrap_or("?")),
                    "patron" => format!("{} patron", v.as_str().unwrap_or("?")),
                    "spellcasting" | "spellcasting2020" | "spellcastingFeature" => {
                        "The ability to cast at least one spell".into()
                    }
                    "proficiency" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_object)
                        .flat_map(|a| a.iter())
                        .map(|(kind, what)| format!("{} {kind} proficiency", title_case(what.as_str().unwrap_or(""))))
                        .collect::<Vec<_>>()
                        .join(", "),
                    "other" | "otherSummary" => match v {
                        Value::String(s) => s.clone(),
                        Value::Object(o) => o.get("entry").and_then(Value::as_str).unwrap_or("").to_string(),
                        _ => continue,
                    },
                    "campaign" => v
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|c| format!("{c} campaign"))
                        .collect::<Vec<_>>()
                        .join(" or "),
                    _ => continue,
                };
                if !s.is_empty() {
                    parts.push(s);
                }
            }
            parts.join(", ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; or ")
}

// ---- classes ----

fn class_props(d: &Value, props: &mut Vec<(&'static str, Vec<Span>)>) {
    if let Some(hd) = d.get("hd").and_then(|h| h.get("faces")).and_then(Value::as_u64) {
        props.push(("Hit Die", plain(format!("d{hd}"))));
    }
    if let Some(pa) = d.get("primaryAbility").and_then(Value::as_array) {
        let s: Vec<String> = pa
            .iter()
            .filter_map(Value::as_object)
            .map(|o| o.keys().map(|k| ability_name(k)).collect::<Vec<_>>().join(" and "))
            .collect();
        props.push(("Primary Ability", plain(s.join(" or "))));
    }
    if let Some(saves) = d.get("proficiency").and_then(Value::as_array) {
        let s: Vec<&str> = saves.iter().filter_map(Value::as_str).map(ability_name).collect();
        props.push(("Saving Throws", plain(s.join(", "))));
    }
    if let Some(sp) = d.get("startingProficiencies") {
        for (key, label) in [("armor", "Armor"), ("weapons", "Weapons"), ("tools", "Tools")] {
            if let Some(list) = sp.get(key).and_then(Value::as_array) {
                let s: Vec<String> = list
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string).or_else(|| str_of(v, "proficiency").map(str::to_string)))
                    .collect();
                props.push((label, tags::parse(&s.join(", "))));
            }
        }
        if let Some(skills) = sp.get("skills") {
            props.push(("Skills", plain(proficiency_list(skills))));
        }
    }
    if let Some(ab) = str_of(d, "spellcastingAbility") {
        props.push(("Spellcasting", plain(ability_name(ab))));
    }
}

/// The class progression table: level, proficiency bonus, features and the
/// class's own columns.
pub fn class_table(d: &Value) -> Table {
    let class_name = str_of(d, "name").unwrap_or("");
    let mut header = vec![plain("Level"), plain("PB"), plain("Features")];
    let mut columns: Vec<Vec<Vec<Span>>> = Vec::new();
    for group in d.get("classTableGroups").and_then(Value::as_array).into_iter().flatten() {
        let labels = group.get("colLabels").and_then(Value::as_array).cloned().unwrap_or_default();
        let rows = group
            .get("rows")
            .or_else(|| group.get("rowsSpellProgression"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (ci, label) in labels.iter().enumerate() {
            header.push(label.as_str().map(tags::parse).unwrap_or_default());
            columns.push(
                (0..20)
                    .map(|lvl| {
                        let cell = rows.get(lvl).and_then(|r| r.get(ci));
                        match cell {
                            Some(Value::Number(n)) if n.as_i64() == Some(0) => plain("—"),
                            Some(c) => doc::from_entries(&Value::Array(vec![c.clone()]))
                                .into_iter()
                                .find_map(|b| match b {
                                    Block::Paragraph(s) => Some(s),
                                    _ => None,
                                })
                                .unwrap_or_default(),
                            None => Vec::new(),
                        }
                    })
                    .collect(),
            );
        }
    }

    let mut features: Vec<Vec<Span>> = vec![Vec::new(); 20];
    for f in d.get("classFeatures").and_then(Value::as_array).into_iter().flatten() {
        let Some(r) = feature_ref(f) else { continue };
        let parts: Vec<&str> = r.split('|').collect();
        let Some(level) = parts.get(3).and_then(|l| l.parse::<usize>().ok()).filter(|l| (1..=20).contains(l)) else {
            continue;
        };
        let slot = &mut features[level - 1];
        if !slot.is_empty() {
            slot.push(Span::plain(", "));
        }
        slot.extend(tags::parse(&format!("{{@classFeature {r}}}")));
    }

    let rows = (0..20)
        .map(|lvl| {
            let mut row = vec![
                plain(ordinal(lvl as u64 + 1)),
                plain(format!("+{}", 2 + lvl / 4)),
                features[lvl].clone(),
            ];
            row.extend(columns.iter().map(|c| c[lvl].clone()));
            row
        })
        .collect();
    Table {
        caption: Some(plain(format!("The {class_name}"))),
        header,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coins() {
        assert_eq!(format_coins(1500), "15 gp");
        assert_eq!(format_coins(20), "2 sp");
        assert_eq!(format_coins(5), "5 cp");
    }

    #[test]
    fn ordinals() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(23), "23rd");
    }
}
