//! Editing a campaign's homebrew document (a data file with `_meta`, see schemas/): cloning
//! entities, changing and reverting fields, and converting `entries` to and
//! from Markdown for editing.

use serde_json::{json, Map, Value};

use crate::id::Key;
use crate::kind::Kind;
use crate::library::{EntityId, Library};

/// Fields that identify an entity of a kind (besides name and source).
pub(crate) fn identity_fields(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::ClassFeature => &["className", "classSource", "level"],
        Kind::Subclass => &["shortName", "className", "classSource"],
        Kind::SubclassFeature => &["className", "classSource", "subclassShortName", "subclassSource", "level"],
        Kind::Subrace => &["raceName", "raceSource"],
        _ => &[],
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CloneMode {
    /// Replace the original within the campaign.
    Replace,
    /// A new entity under its own name.
    Copy { name: String },
}

/// A new homebrew entry cloning `id`, stored as a `_copy` of it.
pub fn clone_entry(lib: &Library, id: EntityId, source: &str, mode: &CloneMode) -> Value {
    let e = lib.entity(id);
    let d = &e.data;
    let raw_name = d.get("name").cloned().unwrap_or(Value::String(e.name.clone()));
    let mut copy = Map::new();
    copy.insert("name".into(), raw_name.clone());
    copy.insert("source".into(), Value::String(e.source.clone()));
    let mut entry = Map::new();
    for f in identity_fields(e.kind) {
        if let Some(v) = d.get(*f) {
            copy.insert(f.to_string(), v.clone());
            entry.insert(f.to_string(), v.clone());
        }
    }
    let name = match mode {
        CloneMode::Replace => raw_name,
        CloneMode::Copy { name } => Value::String(name.clone()),
    };
    entry.insert("name".into(), name);
    entry.insert("source".into(), Value::String(source.to_string()));
    if e.kind == Kind::SubclassFeature {
        // A separate copy of a subclass feature stays in the same subclass.
        entry.insert("subclassSource".into(), d.get("subclassSource").cloned().unwrap_or(Value::Null));
    }
    if e.kind == Kind::Spell && !matches!(mode, CloneMode::Replace) && d.get("classes").is_none() {
        // Official spells get their classes from a lookup keyed by name; a
        // copy under a new name lists them inline.
        let classes: Vec<Value> = lib.spell_classes(id).iter().map(|c| json!({"name": c.name, "source": c.source})).collect();
        if !classes.is_empty() {
            entry.insert("classes".into(), json!({"fromClassList": classes}));
        }
    }
    entry.insert("_copy".into(), Value::Object(copy));
    if *mode == CloneMode::Replace {
        entry.insert("_dnt".into(), json!({"replaces": true}));
    }
    Value::Object(entry)
}

/// Starting points for new entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Spell,
    MagicItem,
    Weapon,
    Armor,
    Gear,
    Feat,
    OptionalFeature,
    Background,
    Species,
}

impl Template {
    pub const ALL: &'static [Template] = &[
        Template::MagicItem,
        Template::Weapon,
        Template::Armor,
        Template::Gear,
        Template::Spell,
        Template::Feat,
        Template::OptionalFeature,
        Template::Background,
        Template::Species,
    ];
    /// Templates that make items (for the inventory's "Create new item…").
    pub const ITEMS: &'static [Template] = &[Template::MagicItem, Template::Weapon, Template::Armor, Template::Gear];

    pub fn label(self) -> &'static str {
        match self {
            Template::Spell => "Spell",
            Template::MagicItem => "Magic item",
            Template::Weapon => "Weapon",
            Template::Armor => "Armor",
            Template::Gear => "Adventuring gear",
            Template::Feat => "Feat",
            Template::OptionalFeature => "Optional feature",
            Template::Background => "Background",
            Template::Species => "Species",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Template::Spell => "a 1st-level evocation to start from",
            Template::MagicItem => "a wondrous item; set rarity and attunement",
            Template::Weapon => "a simple melee weapon with damage dice",
            Template::Armor => "light armor with an AC",
            Template::Gear => "a mundane item with weight and value",
            Template::Feat => "a general feat",
            Template::OptionalFeature => "an eldritch invocation, metamagic, maneuver…",
            Template::Background => "a background description",
            Template::Species => "a medium species with a 30 ft. speed",
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Template::Spell => Kind::Spell,
            Template::MagicItem | Template::Weapon | Template::Armor | Template::Gear => Kind::Item,
            Template::Feat => Kind::Feat,
            Template::OptionalFeature => Kind::OptionalFeature,
            Template::Background => Kind::Background,
            Template::Species => Kind::Race,
        }
    }

    /// A new entry named `name` in `source`.
    pub fn entry(self, name: &str, source: &str) -> Value {
        let mut v = match self {
            Template::Spell => json!({
                "level": 1, "school": "V",
                "time": [{"number": 1, "unit": "action"}],
                "range": {"type": "point", "distance": {"type": "feet", "amount": 60}},
                "components": {"v": true, "s": true},
                "duration": [{"type": "instant"}],
                "entries": ["Describe what the spell does."]
            }),
            Template::MagicItem => json!({"rarity": "uncommon", "wondrous": true, "entries": ["Describe the item."]}),
            Template::Weapon => json!({
                "type": "M|XPHB", "rarity": "none", "weapon": true, "weaponCategory": "simple",
                "dmg1": "1d6", "dmgType": "S", "weight": 2, "value": 500, "entries": ["Describe the weapon."]
            }),
            Template::Armor => json!({"type": "LA|XPHB", "rarity": "none", "armor": true, "ac": 11, "weight": 10, "value": 1000}),
            Template::Gear => json!({"type": "G|XPHB", "rarity": "none", "weight": 1, "value": 100, "entries": ["Describe the item."]}),
            Template::Feat => json!({"category": "G", "entries": ["Describe the feat."]}),
            Template::OptionalFeature => json!({"featureType": ["EI"], "entries": ["Describe the feature."]}),
            Template::Background => json!({"entries": ["Describe the background."]}),
            Template::Species => json!({"size": ["M"], "speed": 30, "entries": ["Describe the species."]}),
        };
        v["name"] = json!(name);
        v["source"] = json!(source);
        v
    }
}

/// Add an entry to the document's list for `kind`; returns its index.
pub fn add_entry(doc: &mut Value, kind: Kind, entry: Value) -> usize {
    let list =
        doc.as_object_mut().expect("homebrew document is an object").entry(kind.json_key()).or_insert_with(|| Value::Array(Vec::new()));
    let list = list.as_array_mut().expect("homebrew list");
    list.push(entry);
    list.len() - 1
}

/// Where an entity's entry is in a homebrew document: the override that
/// replaces it, or the campaign entity itself.
pub fn find_entry(doc: &Value, lib: &Library, id: EntityId) -> Option<(String, usize)> {
    let e = lib.entity(id);
    let key = e.kind.json_key();
    let list = doc.get(key)?.as_array()?;
    let pos = list.iter().position(|entry| {
        let replaces = entry.get("_dnt").and_then(|d| d.get("replaces")).and_then(Value::as_bool) == Some(true);
        if replaces {
            entry.get("_copy").and_then(|c| Key::of_entity(e.kind, c)).as_ref() == Some(&e.key)
        } else {
            Key::of_entity(e.kind, entry).as_ref() == Some(&e.key)
        }
    })?;
    Some((key.to_string(), pos))
}

pub fn entry_mut<'a>(doc: &'a mut Value, at: &(String, usize)) -> Option<&'a mut Map<String, Value>> {
    doc.get_mut(&at.0)?.get_mut(at.1)?.as_object_mut()
}

pub fn entry<'a>(doc: &'a Value, at: &(String, usize)) -> Option<&'a Map<String, Value>> {
    doc.get(&at.0)?.get(at.1)?.as_object()
}

/// Remove an entry. Returns whether it existed.
pub fn delete_entry(doc: &mut Value, at: &(String, usize)) -> bool {
    match doc.get_mut(&at.0).and_then(Value::as_array_mut) {
        Some(list) if at.1 < list.len() => {
            list.remove(at.1);
            true
        }
        _ => false,
    }
}

/// Whether an entry stores (i.e. changes) a field.
pub fn has_field(doc: &Value, at: &(String, usize), field: &str) -> bool {
    entry(doc, at).is_some_and(|e| e.contains_key(field))
}

// ---- Markdown <-> entries ----

/// Tags that become `[[kind:Name@SOURCE|text]]` links in Markdown.
const LINK_TAGS: &[&str] = &[
    "spell",
    "item",
    "condition",
    "creature",
    "feat",
    "race",
    "background",
    "skill",
    "sense",
    "action",
    "variantrule",
    "optfeature",
    "class",
    "language",
    "disease",
    "status",
];

/// Inline tags → Markdown: `{@b x}` → `**x**`, `{@i x}` → `*x*`,
/// `{@spell Fireball|XPHB}` → `[[spell:Fireball@XPHB]]`; others unchanged.
pub fn tags_to_markdown(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("{@") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let mut depth = 0;
        let mut end = None;
        for (i, c) in after.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            out.push_str(after);
            return out;
        };
        let inner = &after[2..end];
        let (tag, body) = inner.split_once(' ').unwrap_or((inner, ""));
        let converted = match tag {
            "b" | "bold" => format!("**{}**", tags_to_markdown(body)),
            "i" | "italic" => format!("*{}*", tags_to_markdown(body)),
            t if LINK_TAGS.contains(&t) => {
                let parts: Vec<&str> = body.split('|').collect();
                let mut link = format!("[[{t}:{}", parts[0]);
                if let Some(src) = parts.get(1).filter(|s| !s.is_empty()) {
                    link.push('@');
                    link.push_str(src);
                }
                if let Some(text) = parts.get(2).filter(|s| !s.is_empty()) {
                    link.push('|');
                    link.push_str(text);
                }
                link.push_str("]]");
                link
            }
            _ => after[..=end].to_string(),
        };
        out.push_str(&converted);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Markdown → inline tags (the reverse of [`tags_to_markdown`]).
pub fn markdown_to_tags(s: &str) -> String {
    // Links first, so `**` inside display text still converts.
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("[[") {
        let Some(len) = rest[start..].find("]]") else { break };
        out.push_str(&rest[..start]);
        let inner = &rest[start + 2..start + len];
        let (target, text) = inner.split_once('|').map_or((inner, None), |(a, b)| (a, Some(b)));
        match target.split_once(':') {
            Some((tag, name)) if LINK_TAGS.contains(&tag.trim()) => {
                let (name, src) = name.split_once('@').map_or((name, ""), |(n, s)| (n, s));
                let mut t = format!("{{@{} {}", tag.trim(), name.trim());
                if !src.is_empty() || text.is_some() {
                    t.push('|');
                    t.push_str(src.trim());
                }
                if let Some(text) = text {
                    t.push('|');
                    t.push_str(text.trim());
                }
                t.push('}');
                out.push_str(&t);
            }
            // Note links have no meaning in game text; keep the words.
            _ => out.push_str(text.unwrap_or(target)),
        }
        rest = &rest[start + len + 2..];
    }
    out.push_str(rest);
    emphasis(&emphasis(&out, "**", "b"), "*", "i")
}

fn emphasis(s: &str, mark: &str, tag: &str) -> String {
    let parts: Vec<&str> = s.split(mark).collect();
    if parts.len() < 3 {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i % 2 == 1 && i + 1 < parts.len() {
            out.push_str(&format!("{{@{tag} {p}}}"));
        } else {
            if i % 2 == 1 {
                out.push_str(mark);
            }
            out.push_str(p);
        }
    }
    out
}

/// `entries` → Markdown for editing.
pub fn entries_to_markdown(entries: &Value) -> String {
    let mut out = Vec::new();
    write_entries(entries, 2, &mut out);
    out.join("\n\n")
}

/// Closes a section in Markdown when something other than a section follows it.
const SECTION_END: &str = "<!-- /section -->";

fn is_section(e: &Value) -> bool {
    e.get("type").and_then(Value::as_str).unwrap_or(if e.is_object() { "entries" } else { "" }) == "entries"
        && e.get("name").is_some()
        && e.as_object().is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "type" | "name" | "entries")))
}

fn write_entries(v: &Value, depth: usize, out: &mut Vec<String>) {
    let list = v.as_array().map(Vec::as_slice).unwrap_or(std::slice::from_ref(v));
    for (i, e) in list.iter().enumerate() {
        if i > 0 && is_section(&list[i - 1]) && !is_section(e) {
            out.push(SECTION_END.to_string());
        }
        match e {
            Value::String(s) => out.push(tags_to_markdown(s)),
            Value::Object(o) => {
                let ty = o.get("type").and_then(Value::as_str).unwrap_or("entries");
                let simple_list = ty == "list"
                    && o.get("items").and_then(Value::as_array).is_some_and(|items| {
                        items.iter().all(|i| {
                            i.is_string()
                                || (i.get("type").and_then(Value::as_str) == Some("item") && i.get("entry").is_some_and(Value::is_string))
                        })
                    });
                if ty == "entries" && o.keys().all(|k| matches!(k.as_str(), "type" | "name" | "entries")) {
                    if let Some(name) = o.get("name").and_then(Value::as_str) {
                        out.push(format!("{} {}", "#".repeat(depth.min(6)), tags_to_markdown(name)));
                    }
                    if let Some(inner) = o.get("entries") {
                        write_entries(inner, depth + 1, out);
                    }
                } else if simple_list && o.keys().all(|k| matches!(k.as_str(), "type" | "items")) {
                    let lines: Vec<String> = o["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| match i {
                            Value::String(s) => format!("- {}", tags_to_markdown(s)),
                            item => format!(
                                "- **{}** {}",
                                item.get("name").and_then(Value::as_str).unwrap_or(""),
                                tags_to_markdown(item.get("entry").and_then(Value::as_str).unwrap_or(""))
                            ),
                        })
                        .collect();
                    out.push(lines.join("\n"));
                } else {
                    // Anything else round-trips untouched.
                    out.push(format!("```json\n{}\n```", serde_json::to_string_pretty(e).unwrap_or_default()));
                }
            }
            other => out.push(other.to_string()),
        }
    }
}

/// Markdown → `entries`. Fenced ```json blocks must contain valid JSON.
pub fn markdown_to_entries(md: &str) -> Result<Value, String> {
    // Split into blocks: fenced JSON, headings, lists, paragraphs.
    enum Block {
        Heading(usize, String),
        /// End of the innermost open section.
        End,
        Para(String),
        List(Vec<String>),
        Json(Value),
    }
    let mut blocks = Vec::new();
    let mut lines = md.lines().peekable();
    let mut para: Vec<String> = Vec::new();
    let flush = |para: &mut Vec<String>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Para(para.join(" ")));
            para.clear();
        }
    };
    while let Some(line) = lines.next() {
        let t = line.trim();
        if t.starts_with("```") {
            flush(&mut para, &mut blocks);
            let mut body = String::new();
            for l in lines.by_ref() {
                if l.trim().starts_with("```") {
                    break;
                }
                body.push_str(l);
                body.push('\n');
            }
            let v: Value = serde_json::from_str(&body).map_err(|e| format!("JSON block: {e}"))?;
            blocks.push(Block::Json(v));
        } else if t.is_empty() {
            flush(&mut para, &mut blocks);
        } else if t == SECTION_END {
            flush(&mut para, &mut blocks);
            blocks.push(Block::End);
        } else if let Some(h) = heading(t) {
            flush(&mut para, &mut blocks);
            blocks.push(h);
        } else if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut blocks);
            let mut items = vec![item.to_string()];
            while let Some(next) = lines.peek().map(|l| l.trim()) {
                match next.strip_prefix("- ").or_else(|| next.strip_prefix("* ")) {
                    Some(i) => {
                        items.push(i.to_string());
                        lines.next();
                    }
                    None => break,
                }
            }
            blocks.push(Block::List(items));
        } else {
            para.push(t.to_string());
        }
    }
    flush(&mut para, &mut blocks);

    fn heading(t: &str) -> Option<Block> {
        let n = t.chars().take_while(|c| *c == '#').count();
        ((1..=6).contains(&n) && t[n..].starts_with(' ')).then(|| Block::Heading(n, t[n..].trim().to_string()))
    }

    // Nest blocks under headings by level.
    fn build(blocks: &[Block], i: &mut usize, level: usize) -> Vec<Value> {
        let mut out = Vec::new();
        while *i < blocks.len() {
            match &blocks[*i] {
                Block::Heading(l, name) => {
                    if *l <= level {
                        return out;
                    }
                    *i += 1;
                    let inner = build(blocks, i, *l);
                    out.push(json!({"type": "entries", "name": markdown_to_tags(name), "entries": inner}));
                }
                Block::End => {
                    *i += 1;
                    if level > 0 {
                        return out;
                    }
                }
                Block::Para(p) => {
                    out.push(Value::String(markdown_to_tags(p)));
                    *i += 1;
                }
                Block::List(items) => {
                    let items: Vec<Value> = items
                        .iter()
                        .map(|it| {
                            // `**Name** text` is a named item.
                            match it.strip_prefix("**").and_then(|r| r.split_once("**")) {
                                Some((name, rest)) => {
                                    json!({"type": "item", "name": markdown_to_tags(name), "entry": markdown_to_tags(rest.trim())})
                                }
                                None => Value::String(markdown_to_tags(it)),
                            }
                        })
                        .collect();
                    out.push(json!({"type": "list", "items": items}));
                    *i += 1;
                }
                Block::Json(v) => {
                    out.push(v.clone());
                    *i += 1;
                }
            }
        }
        out
    }
    let mut i = 0;
    Ok(Value::Array(build(&blocks, &mut i, 0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_round_trip() {
        let s = "Deal {@damage 8d6} to {@b all} creatures, see {@spell Fireball|XPHB} or {@condition prone||knocked down}.";
        let md = tags_to_markdown(s);
        assert_eq!(md, "Deal {@damage 8d6} to **all** creatures, see [[spell:Fireball@XPHB]] or [[condition:prone|knocked down]].");
        assert_eq!(markdown_to_tags(&md), s);
        assert_eq!(markdown_to_tags("a *b* c [[spell:Shield]] [[Strahd]]"), "a {@i b} c {@spell Shield} Strahd");
    }

    #[test]
    fn entries_round_trip() {
        let entries = json!([
            "First paragraph with {@i style}.",
            {"type": "entries", "name": "Blood Price", "entries": [
                "Inner text.",
                {"type": "list", "items": ["one", {"type": "item", "name": "Two.", "entry": "second"}]}
            ]},
            {"type": "table", "colLabels": ["d6", "Effect"], "rows": [["1", "Boom"]]}
        ]);
        let md = entries_to_markdown(&entries);
        assert!(md.contains("## Blood Price"));
        assert!(md.contains("- **Two.** second"));
        assert!(md.contains("```json"));
        assert_eq!(markdown_to_entries(&md).unwrap(), entries);
        assert!(markdown_to_entries("```json\n{oops\n```").is_err());
    }

    #[test]
    fn templates_load() {
        let mut lib = Library::new();
        let mut doc = json!({});
        for t in Template::ALL {
            add_entry(&mut doc, t.kind(), t.entry(&format!("New {}", t.label()), "CoS-HB"));
        }
        lib.load_json(&doc);
        for t in Template::ALL {
            let key = Key::of_entity(t.kind(), &t.entry(&format!("New {}", t.label()), "CoS-HB")).unwrap();
            assert!(lib.get(&key).is_some(), "{t:?}");
        }
    }

    #[test]
    fn clone_and_find() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "spell": [{"name": "Shield", "source": "XPHB", "level": 1, "entries": ["x"]}],
            "classFeature": [{"name": "Rage", "source": "XPHB", "className": "Barbarian", "classSource": "XPHB", "level": 1, "entries": ["y"]}]
        }));
        let shield = lib.get(&Key::new(Kind::Spell, &["Shield", "XPHB"])).unwrap();
        let rage = lib.get(&Key::new(Kind::ClassFeature, &["Rage", "Barbarian", "XPHB", "1", "XPHB"])).unwrap();
        let mut doc = json!({"_meta": {"sources": [{"json": "CoS-HB"}]}});
        let e = clone_entry(&lib, rage, "CoS-HB", &CloneMode::Replace);
        assert_eq!(e["_copy"], json!({"name": "Rage", "source": "XPHB", "className": "Barbarian", "classSource": "XPHB", "level": 1}));
        add_entry(&mut doc, Kind::ClassFeature, e);
        add_entry(&mut doc, Kind::Spell, clone_entry(&lib, shield, "CoS-HB", &CloneMode::Copy { name: "Ward".into() }));

        let mut with = lib.clone();
        with.apply_campaign(&doc);
        assert_eq!(find_entry(&doc, &with, rage), Some(("classFeature".into(), 0)));
        let ward = with.get(&Key::new(Kind::Spell, &["Ward", "CoS-HB"])).unwrap();
        assert_eq!(with.entity(ward).data["level"], 1, "the copy inherits");
        let at = find_entry(&doc, &with, ward).unwrap();
        entry_mut(&mut doc, &at).unwrap().insert("level".into(), json!(2));
        assert!(has_field(&doc, &at, "level"));
        assert!(delete_entry(&mut doc, &at));
        assert!(find_entry(&doc, &with, ward).is_none());
    }
}
