//! Converts data `entries` trees into a small, renderer-agnostic document
//! model. The TUI turns [`Block`]s into terminal lines.

use serde_json::Value;

use crate::tags::{self, Link, Span, Style};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: u8, text: Vec<Span> },
    Paragraph(Vec<Span>),
    List(Vec<Vec<Block>>),
    Table(Table),
    /// Boxed content: insets, sidebars, variant rules.
    Inset { title: Option<Vec<Span>>, body: Vec<Block> },
    Quote { body: Vec<Block>, by: Option<String> },
    /// Compact `Label: value` lines, e.g. an entity's header properties.
    Properties(Vec<Vec<Span>>),
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub caption: Option<Vec<Span>>,
    pub header: Vec<Vec<Span>>,
    pub rows: Vec<Vec<Vec<Span>>>,
}

/// Convert an `entries` array (or a single entry) into blocks.
pub fn from_entries(entries: &Value) -> Vec<Block> {
    let mut out = Vec::new();
    push_entries(entries, 1, &mut out);
    out
}

fn push_entries(entries: &Value, depth: u8, out: &mut Vec<Block>) {
    match entries {
        Value::Array(items) => items.iter().for_each(|e| push_entry(e, depth, out)),
        other => push_entry(other, depth, out),
    }
}

fn spans_of(v: &Value) -> Vec<Span> {
    match v {
        Value::String(s) => tags::parse(s),
        Value::Number(n) => vec![Span::plain(n.to_string())],
        Value::Bool(b) => vec![Span::plain(b.to_string())],
        Value::Object(_) => inline_of(v),
        _ => Vec::new(),
    }
}

fn bold(text: &str) -> Span {
    Span::styled(
        text,
        Style {
            bold: true,
            ..Style::default()
        },
    )
}

fn str_field<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn push_entry(entry: &Value, depth: u8, out: &mut Vec<Block>) {
    let obj = match entry {
        Value::String(s) => {
            out.push(Block::Paragraph(tags::parse(s)));
            return;
        }
        Value::Object(_) => entry,
        Value::Number(_) => {
            out.push(Block::Paragraph(spans_of(entry)));
            return;
        }
        _ => return,
    };
    let name = str_field(obj, "name");
    let body = obj.get("entries").or_else(|| obj.get("entry"));
    match str_field(obj, "type").unwrap_or("entries") {
        "entries" | "section" | "options" | "variantSub" => {
            if let Some(name) = name {
                out.push(Block::Heading {
                    level: depth,
                    text: tags::parse(name),
                });
            }
            if let Some(body) = body {
                push_entries(body, depth.saturating_add(1), out);
            }
        }
        "inset" | "insetReadaloud" | "variant" | "variantInner" => {
            let mut inner = Vec::new();
            if let Some(body) = body {
                push_entries(body, depth.saturating_add(1), &mut inner);
            }
            let title = name.map(|n| {
                let prefix = if str_field(obj, "type") == Some("variant") { "Variant: " } else { "" };
                tags::parse(&format!("{prefix}{n}"))
            });
            out.push(Block::Inset { title, body: inner });
        }
        "quote" => {
            let mut inner = Vec::new();
            if let Some(body) = body {
                push_entries(body, depth, &mut inner);
            }
            out.push(Block::Quote {
                body: inner,
                by: str_field(obj, "by").map(tags::strip),
            });
        }
        "list" => {
            let items = obj
                .get("items")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            let mut blocks = Vec::new();
                            push_list_item(item, depth, &mut blocks);
                            blocks
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(Block::List(items));
        }
        "item" | "itemSub" | "itemSpell" => push_list_item(obj, depth, out),
        "table" => out.push(Block::Table(table_of(obj))),
        "tableGroup" => {
            if let Some(tables) = obj.get("tables").and_then(Value::as_array) {
                for t in tables {
                    out.push(Block::Table(table_of(t)));
                }
            }
        }
        "hr" => out.push(Block::Rule),
        "image" | "gallery" => {
            let title = str_field(obj, "title").unwrap_or("image");
            out.push(Block::Paragraph(vec![Span::styled(
                format!("[{title}]"),
                Style {
                    italic: true,
                    ..Style::default()
                },
            )]));
        }
        "abilityDc" | "abilityAttackMod" | "abilityGeneric" => {
            out.push(Block::Paragraph(ability_line(obj)));
        }
        "refClassFeature" | "refSubclassFeature" | "refOptionalfeature" => {
            out.push(Block::Paragraph(inline_of(obj)));
        }
        _ => {
            // Unknown block type: render whatever text-like content it has.
            if let Some(name) = name {
                out.push(Block::Heading {
                    level: depth,
                    text: tags::parse(name),
                });
            }
            if let Some(body) = body {
                push_entries(body, depth.saturating_add(1), out);
            } else {
                let spans = inline_of(obj);
                if !spans.is_empty() {
                    out.push(Block::Paragraph(spans));
                }
            }
        }
    }
}

/// List items: either plain entries, or `{type: item, name, entry/entries}`
/// rendered as a run-in bold heading.
fn push_list_item(item: &Value, depth: u8, out: &mut Vec<Block>) {
    let is_named_item = matches!(str_field(item, "type"), Some("item" | "itemSub" | "itemSpell"));
    if !is_named_item {
        push_entry(item, depth, out);
        return;
    }
    let mut lead = str_field(item, "name").map(|n| {
        let mut s = bold(&tags::strip(n));
        if !n.ends_with(['.', ':', '?', '!']) {
            s.text.push('.');
        }
        s.text.push(' ');
        s
    });
    let body = item.get("entries").or_else(|| item.get("entry"));
    let mut rest = Vec::new();
    if let Some(body) = body {
        push_entries(body, depth.saturating_add(1), &mut rest);
    }
    // Merge the run-in name into the first paragraph.
    match (lead.take(), rest.first_mut()) {
        (Some(lead), Some(Block::Paragraph(spans))) => spans.insert(0, lead),
        (Some(lead), _) => rest.insert(0, Block::Paragraph(vec![lead])),
        (None, _) => {}
    }
    out.extend(rest);
}

fn table_of(obj: &Value) -> Table {
    let header = obj
        .get("colLabels")
        .and_then(Value::as_array)
        .map(|labels| labels.iter().map(spans_of).collect())
        .unwrap_or_default();
    let rows = obj
        .get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let cells = match row {
                        Value::Array(cells) => cells,
                        Value::Object(o) => o.get("row")?.as_array()?,
                        _ => return None,
                    };
                    Some(cells.iter().map(cell_of).collect())
                })
                .collect()
        })
        .unwrap_or_default();
    Table {
        caption: str_field(obj, "caption").map(tags::parse),
        header,
        rows,
    }
}

fn cell_of(cell: &Value) -> Vec<Span> {
    if str_field(cell, "type") == Some("cell") {
        if let Some(roll) = cell.get("roll") {
            if let Some(exact) = roll.get("exact").and_then(Value::as_i64) {
                return vec![Span::plain(exact.to_string())];
            }
            let min = roll.get("min").and_then(Value::as_i64).unwrap_or(0);
            let max = roll.get("max").and_then(Value::as_i64).unwrap_or(0);
            return vec![Span::plain(format!("{min}–{max}"))];
        }
        if let Some(entry) = cell.get("entry") {
            return spans_of(entry);
        }
    }
    spans_of(cell)
}

/// Best-effort single-line rendering of structured inline objects
/// (dice, bonuses, references).
fn inline_of(obj: &Value) -> Vec<Span> {
    let mechanic = Style {
        mechanic: true,
        ..Style::default()
    };
    match str_field(obj, "type") {
        Some("dice") => {
            let text = obj
                .get("toRoll")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .map(|p| {
                            let n = p.get("number").and_then(Value::as_i64).unwrap_or(1);
                            let f = p.get("faces").and_then(Value::as_i64).unwrap_or(0);
                            format!("{n}d{f}")
                        })
                        .collect::<Vec<_>>()
                        .join("+")
                })
                .unwrap_or_default();
            vec![Span::styled(text, mechanic)]
        }
        Some("bonus") => {
            let v = obj.get("value").and_then(Value::as_i64).unwrap_or(0);
            vec![Span::styled(format!("{v:+}"), mechanic)]
        }
        Some("bonusSpeed") => {
            let v = obj.get("value").and_then(Value::as_i64).unwrap_or(0);
            let text = if v == 0 { "—".to_string() } else { format!("{v:+} ft.") };
            vec![Span::plain(text)]
        }
        Some(t @ ("refClassFeature" | "refSubclassFeature" | "refOptionalfeature")) => {
            let (tag, field) = match t {
                "refClassFeature" => ("classFeature", "classFeature"),
                "refSubclassFeature" => ("subclassFeature", "subclassFeature"),
                _ => ("optfeature", "optionalfeature"),
            };
            let args = str_field(obj, field).unwrap_or_default();
            let mut spans = tags::parse(&format!("{{@{tag} {args}}}"));
            spans.insert(0, Span::plain("→ "));
            spans
        }
        Some("link") => vec![Span::plain(str_field(obj, "text").unwrap_or_default())],
        _ => str_field(obj, "text")
            .or_else(|| str_field(obj, "name"))
            .map(tags::parse)
            .unwrap_or_default(),
    }
}

fn ability_line(obj: &Value) -> Vec<Span> {
    let name = str_field(obj, "name").unwrap_or("Ability");
    let attrs = obj
        .get("attributes")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(|s| s.to_uppercase())
                .collect::<Vec<_>>()
                .join(" or ")
        })
        .unwrap_or_default();
    let formula = match str_field(obj, "type") {
        Some("abilityDc") => format!("8 + proficiency bonus + {attrs} modifier"),
        Some("abilityAttackMod") => format!("proficiency bonus + {attrs} modifier"),
        _ => obj.get("text").and_then(Value::as_str).map(tags::strip).unwrap_or_default(),
    };
    let label = match str_field(obj, "type") {
        Some("abilityDc") => format!("{name} save DC = "),
        Some("abilityAttackMod") => format!("{name} attack modifier = "),
        _ => format!("{name} = "),
    };
    vec![bold(&label), Span::plain(formula)]
}

/// All text of a document joined by spaces, for full-text search.
pub fn plain_text(blocks: &[Block]) -> String {
    fn spans(s: &[Span], out: &mut String) {
        for span in s {
            out.push_str(&span.text);
        }
        out.push(' ');
    }
    fn walk(blocks: &[Block], out: &mut String) {
        for b in blocks {
            match b {
                Block::Heading { text, .. } | Block::Paragraph(text) => spans(text, out),
                Block::Properties(lines) => lines.iter().for_each(|l| spans(l, out)),
                Block::List(items) => items.iter().for_each(|i| walk(i, out)),
                Block::Table(t) => {
                    t.header.iter().for_each(|c| spans(c, out));
                    t.rows.iter().flatten().for_each(|c| spans(c, out));
                }
                Block::Inset { title, body } => {
                    if let Some(t) = title {
                        spans(t, out);
                    }
                    walk(body, out);
                }
                Block::Quote { body, .. } => walk(body, out),
                Block::Rule => {}
            }
        }
    }
    let mut out = String::new();
    walk(blocks, &mut out);
    out
}

/// Collect every link in a document, in reading order.
pub fn links(blocks: &[Block]) -> Vec<&Link> {
    fn spans<'a>(s: &'a [Span], out: &mut Vec<&'a Link>) {
        for span in s {
            if let Some(l) = &span.link {
                if out.last().is_none_or(|last| *last != l) {
                    out.push(l);
                }
            }
        }
    }
    fn walk<'a>(blocks: &'a [Block], out: &mut Vec<&'a Link>) {
        for b in blocks {
            match b {
                Block::Heading { text, .. } | Block::Paragraph(text) => spans(text, out),
                Block::Properties(lines) => lines.iter().for_each(|l| spans(l, out)),
                Block::List(items) => items.iter().for_each(|i| walk(i, out)),
                Block::Table(t) => {
                    t.header.iter().for_each(|c| spans(c, out));
                    t.rows.iter().flatten().for_each(|c| spans(c, out));
                }
                Block::Inset { title, body } => {
                    if let Some(t) = title {
                        spans(t, out);
                    }
                    walk(body, out);
                }
                Block::Quote { body, .. } => walk(body, out),
                Block::Rule => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(blocks, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_entries_become_headings() {
        let blocks = from_entries(&json!([{"type": "entries", "name": "Rage", "entries": ["You rage."]}]));
        assert!(matches!(&blocks[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&blocks[1], Block::Paragraph(_)));
    }

    #[test]
    fn list_items_run_in() {
        let blocks = from_entries(&json!([{"type": "list", "items": [
            {"type": "item", "name": "Speed", "entry": "30 feet"}
        ]}]));
        let Block::List(items) = &blocks[0] else { panic!() };
        let Block::Paragraph(spans) = &items[0][0] else { panic!() };
        assert_eq!(spans[0].text, "Speed. ");
        assert!(spans[0].style.bold);
    }

    #[test]
    fn table_cells() {
        let blocks = from_entries(&json!([{"type": "table", "colLabels": ["d6", "Effect"],
            "rows": [[{"type": "cell", "roll": {"min": 1, "max": 3}}, "Nothing"], [6, "{@b Boom}"]]}]));
        let Block::Table(t) = &blocks[0] else { panic!() };
        assert_eq!(t.rows[0][0][0].text, "1–3");
        assert_eq!(t.rows[1][0][0].text, "6");
        assert!(t.rows[1][1][0].style.bold);
    }

    #[test]
    fn ref_features_are_links() {
        let blocks = from_entries(&json!([{"type": "refSubclassFeature",
            "subclassFeature": "Abjuration Savant|Wizard||Abjuration||2"}]));
        assert_eq!(links(&blocks).len(), 1);
    }
}
