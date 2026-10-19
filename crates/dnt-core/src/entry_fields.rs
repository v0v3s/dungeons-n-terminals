//! The fields of a homebrew entry, for editing it field by field: which
//! fields to show and in what order, how each is shown and edited, and
//! turning what was typed back into the field's value.

use dnt_data::forms::{self, Format};
use dnt_data::{Kind, Library};
use serde_json::Value;

use crate::homebrew::EntryRef;

/// How a field is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Widget {
    Text,
    Number,
    Bool,
    /// `entries`-like: Markdown in the big editor.
    Entries,
    /// Anything structured: JSON in the big editor.
    Json,
    /// Friendly text or a choice list (see `dnt_data::forms`).
    Form(Format),
}

pub struct Field {
    pub name: String,
    pub value: Option<Value>,
    /// The value as shown in the list.
    pub display: String,
    pub widget: Widget,
    /// Stored in the homebrew entry (changed from the original, or new).
    pub changed: bool,
    /// Not one of the kind's common fields.
    pub extra: bool,
}

/// Common fields per kind, in order; the rest follow alphabetically.
fn order(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Spell => {
            &["name", "level", "school", "time", "range", "components", "duration", "meta", "classes", "entries", "entriesHigherLevel"]
        }
        Kind::Item | Kind::BaseItem | Kind::MagicVariant => &[
            "name",
            "type",
            "rarity",
            "reqAttune",
            "wondrous",
            "weight",
            "value",
            "ac",
            "bonusAc",
            "weaponCategory",
            "dmg1",
            "dmg2",
            "dmgType",
            "bonusWeapon",
            "property",
            "entries",
        ],
        Kind::Feat => &["name", "category", "prerequisite", "ability", "entries"],
        _ => &["name", "entries"],
    }
}

fn widget_for(kind: Kind, field: &str, value: Option<&Value>) -> Widget {
    if let Some(f) = forms::format_for(kind, field) {
        return Widget::Form(f);
    }
    match (field, value) {
        ("entries" | "entriesHigherLevel" | "entriesTemplate", _) => Widget::Entries,
        (_, Some(Value::Bool(_))) | ("wondrous", None) => Widget::Bool,
        (_, Some(Value::Number(_))) | ("weight" | "ac", None) => Widget::Number,
        (_, Some(Value::String(_))) | (_, None) => Widget::Text,
        _ => Widget::Json,
    }
}

/// A short one-line rendering of a JSON value.
fn preview(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Array(a) if a.iter().all(|x| x.is_string()) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · "),
        other => other.to_string(),
    }
}

/// Fields to show, given the merged entity and the homebrew entry.
pub fn fields(lib: &Library, of: &EntryRef, entry: Option<&serde_json::Map<String, Value>>) -> Vec<Field> {
    let Some(id) = of.entity(lib) else { return Vec::new() };
    let data = &lib.entity(id).data;
    let common: Vec<String> = order(of.kind).iter().map(|s| s.to_string()).collect();
    let mut rest: Vec<String> = data
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k.clone())
        .filter(|k| !k.starts_with('_') && !common.contains(k))
        .filter(|k| {
            !matches!(k.as_str(), "source" | "page" | "srd" | "srd52" | "basicRules" | "basicRules2024" | "hasFluff" | "hasFluffImages")
        })
        .collect();
    rest.sort();
    let n_common = common.len();
    common
        .into_iter()
        .chain(rest)
        .enumerate()
        .map(|(i, name)| {
            let mut value = data.get(&name).cloned();
            if name == "classes" && value.is_none() && of.kind == Kind::Spell {
                // Official spells get their classes from a lookup.
                let list: Vec<Value> =
                    lib.spell_classes(id).iter().map(|c| serde_json::json!({"name": c.name, "source": c.source})).collect();
                if !list.is_empty() {
                    value = Some(serde_json::json!({"fromClassList": list}));
                }
            }
            let widget = widget_for(of.kind, &name, value.as_ref());
            let display = match (&value, widget) {
                (_, Widget::Form(f)) => forms::display(f, value.as_ref()),
                (None, _) => "—".to_string(),
                (Some(v), Widget::Entries) => dnt_data::homebrew::entries_to_markdown(v).split_whitespace().collect::<Vec<_>>().join(" "),
                (Some(v), _) => preview(v),
            };
            Field {
                widget,
                display,
                changed: entry.is_some_and(|e| e.contains_key(&name)) && !(name == "name" && of.replaces),
                extra: i >= n_common,
                name,
                value,
            }
        })
        .collect()
}

impl Field {
    /// The choices to pick from, for fields with a fixed set of values.
    pub fn choices(&self) -> Option<Vec<(String, Option<Value>)>> {
        match self.widget {
            Widget::Form(f) => forms::choices(f),
            _ => None,
        }
    }

    /// The text to start editing from: plain text, friendly text, Markdown
    /// (entries) or pretty JSON.
    pub fn text(&self) -> String {
        match (self.widget, &self.value) {
            (_, None) if self.widget != Widget::Json => String::new(),
            (Widget::Form(_), Some(_)) => self.display.clone(),
            (Widget::Entries, Some(v)) => dnt_data::homebrew::entries_to_markdown(v),
            (Widget::Text | Widget::Number | Widget::Bool, Some(Value::String(s))) => s.clone(),
            (Widget::Text | Widget::Number | Widget::Bool, Some(v)) => v.to_string(),
            (_, v) => json_text(v.as_ref()),
        }
    }

    /// What typed text means for this field.
    pub fn parse(&self, text: &str, lib: &Library) -> Result<Option<Value>, String> {
        match self.widget {
            Widget::Form(format) => forms::parse(format, text.trim(), self.value.as_ref(), lib),
            Widget::Number => {
                let t = text.trim();
                t.parse::<i64>()
                    .map(Value::from)
                    .or_else(|_| t.parse::<f64>().map(Value::from))
                    .map(Some)
                    .map_err(|_| format!("{} needs a number", self.name))
            }
            Widget::Entries => dnt_data::homebrew::markdown_to_entries(text).map(Some),
            Widget::Json => parse_json(text).map(Some),
            Widget::Text | Widget::Bool => Ok(Some(Value::String(text.trim().to_string()))),
        }
    }

    /// A yes/no field switched.
    pub fn toggled(&self) -> Value {
        Value::Bool(!self.value.as_ref().and_then(Value::as_bool).unwrap_or(false))
    }

    /// The hint for typing this field's value, if it has one.
    pub fn format(&self) -> Option<Format> {
        match self.widget {
            Widget::Form(f) => Some(f),
            _ => None,
        }
    }
}

/// Any value as pretty JSON (`null` when unset).
pub fn json_text(v: Option<&Value>) -> String {
    serde_json::to_string_pretty(v.unwrap_or(&Value::Null)).unwrap_or_default()
}

pub fn parse_json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| format!("Not valid JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::homebrew::Doc;
    use serde_json::json;

    #[test]
    fn fields_show_and_parse_values() {
        let mut lib = Library::new();
        lib.load_json(&json!({"item": [{"name": "Lamp", "source": "CS", "weight": 1, "entries": ["Gives {@b light}."]}]}));
        let key = dnt_data::Key::of_entity(Kind::Item, &json!({"name": "Lamp", "source": "CS"})).unwrap();
        let of = EntryRef { key, kind: Kind::Item, replaces: false, doc: Doc::Campaign };
        let stored = json!({"name": "Lamp", "weight": 1});
        let fields = fields(&lib, &of, stored.as_object());
        assert_eq!(fields[0].name, "name", "common fields first");
        let field = |n: &str| fields.iter().find(|f| f.name == n).unwrap();

        let weight = field("weight");
        assert_eq!((weight.widget, weight.changed, weight.text()), (Widget::Number, true, "1".to_string()));
        assert_eq!(weight.parse(" 2.5 ", &lib), Ok(Some(json!(2.5))));
        assert!(weight.parse("heavy", &lib).unwrap_err().contains("number"));

        let entries = field("entries");
        assert_eq!((entries.widget, entries.changed), (Widget::Entries, false));
        assert_eq!(entries.parse(&entries.text(), &lib).unwrap(), Some(json!(["Gives {@b light}."])));

        assert_eq!(field("wondrous").toggled(), json!(true));
        assert!(field("rarity").choices().is_some());
        assert_eq!(json_text(None), "null");
        assert!(parse_json("{").is_err());
    }
}
