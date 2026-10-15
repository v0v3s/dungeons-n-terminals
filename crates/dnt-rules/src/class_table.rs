//! Free automation from structured data: every column of a class's
//! progression table becomes a level-indexed variable.
//!
//! `Martial Arts` on the Monk → `class.monk.martial_arts` (`1d6` … `1d12`),
//! spell slot columns → `class.wizard.slots.1` … `class.wizard.slots.9`.
//! Homebrew classes get the same treatment, so e.g. a Blood Hunter's
//! "Hemocraft Die" column is usable in overlays without any extra work.

use dnt_data::tags;
use serde_json::Value as Json;

use crate::formula::Formula;
use crate::value::{Dice, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct ClassColumn {
    /// Variable name, e.g. `class.monk.martial_arts`.
    pub var: String,
    /// Column label as shown in the table.
    pub label: String,
    /// Values for levels 1–20.
    pub values: Vec<Value>,
}

impl ClassColumn {
    /// The value at a character level (1-based), clamped to the table.
    pub fn at(&self, level: usize) -> Value {
        let i = level.clamp(1, self.values.len().max(1)) - 1;
        self.values.get(i).cloned().unwrap_or(Value::int(0))
    }
}

/// Identifier-safe slug: `Blood Hunter` → `blood_hunter`.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

/// `1st` → 1, `2nd` → 2, …
fn ordinal(s: &str) -> Option<i64> {
    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
    let rest = &s[digits.len()..];
    (!digits.is_empty() && matches!(rest, "st" | "nd" | "rd" | "th")).then(|| digits.parse().ok())?
}

fn cell_value(cell: &Json) -> Value {
    match cell {
        Json::Number(n) => Value::int(n.as_i64().unwrap_or(0)),
        Json::String(s) => {
            let text = tags::strip(s);
            let text = text.trim();
            if text.is_empty() || matches!(text, "—" | "-" | "–") {
                return Value::int(0);
            }
            if let Some(n) = ordinal(text) {
                return Value::int(n);
            }
            // Numbers, signed bonuses and dice parse as formulas.
            let normalised = text.trim_start_matches('+').replace(" ft.", "");
            match Formula::parse(&normalised).ok().and_then(|f| f.eval(&std::collections::HashMap::new()).ok()) {
                Some(v @ (Value::Num(_) | Value::Dice(_))) => v,
                _ => Value::Str(text.to_string()),
            }
        }
        Json::Object(o) => match o.get("type").and_then(Json::as_str) {
            Some("dice") => {
                let mut dice = Dice::default();
                for part in o.get("toRoll").and_then(Json::as_array).into_iter().flatten() {
                    let n = part.get("number").and_then(Json::as_i64).unwrap_or(1);
                    let f = part.get("faces").and_then(Json::as_u64).unwrap_or(0) as u32;
                    dice = dice.plus(&Dice::new(n, f));
                }
                Value::Dice(dice)
            }
            Some("bonus" | "bonusSpeed") => Value::int(o.get("value").and_then(Json::as_i64).unwrap_or(0)),
            _ => o.get("value").map(cell_value).unwrap_or(Value::int(0)),
        },
        _ => Value::int(0),
    }
}

/// Extract all table columns of a class (its JSON).
pub fn class_columns(class: &Json) -> Vec<ClassColumn> {
    let name = class.get("name").and_then(Json::as_str).unwrap_or("class");
    table_columns(class.get("classTableGroups"), &slug(name))
}

/// Extract a subclass's own table columns (e.g. Eldritch Knight spell
/// slots). They live in the parent class's namespace: `class.fighter.slots.1`.
pub fn subclass_columns(subclass: &Json) -> Vec<ClassColumn> {
    let class = subclass.get("className").and_then(Json::as_str).unwrap_or("class");
    table_columns(subclass.get("subclassTableGroups"), &slug(class))
}

fn table_columns(groups: Option<&Json>, class_slug: &str) -> Vec<ClassColumn> {
    let mut out = Vec::new();
    for group in groups.and_then(Json::as_array).into_iter().flatten() {
        let title = group.get("title").and_then(Json::as_str).map(tags::strip).unwrap_or_default();
        let (rows, is_slots) = match group.get("rowsSpellProgression") {
            Some(rows) => (rows, true),
            None => (group.get("rows").unwrap_or(&Json::Null), title.to_lowercase().contains("spell slots")),
        };
        let rows = rows.as_array().cloned().unwrap_or_default();
        for (ci, label) in group.get("colLabels").and_then(Json::as_array).into_iter().flatten().enumerate() {
            let label = label.as_str().map(tags::strip).unwrap_or_default();
            let col = match (is_slots, ordinal(label.trim())) {
                (true, Some(n)) => format!("slots.{n}"),
                _ => slug(&label),
            };
            if col.is_empty() {
                continue;
            }
            let values = rows.iter().map(|row| row.get(ci).map(cell_value).unwrap_or(Value::int(0))).collect();
            out.push(ClassColumn { var: format!("class.{class_slug}.{col}"), label, values });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slugs() {
        assert_eq!(slug("Blood Hunter"), "blood_hunter");
        assert_eq!(slug("Ki Points"), "ki_points");
        assert_eq!(slug("Sneak Attack"), "sneak_attack");
        assert_eq!(slug("1st"), "_1st");
    }

    #[test]
    fn columns_from_table() {
        let class = json!({
            "name": "Monk",
            "classTableGroups": [
                {"colLabels": ["Martial Arts", "Focus Points", "Unarmored Movement"], "rows": [
                    [{"type": "dice", "toRoll": [{"number": 1, "faces": 6}]}, 0, {"type": "bonusSpeed", "value": 0}],
                    [{"type": "dice", "toRoll": [{"number": 1, "faces": 6}]}, 2, {"type": "bonusSpeed", "value": 10}]
                ]},
                {"title": "Spell Slots per Spell Level", "colLabels": ["{@filter 1st|spells|level=1}", "2nd"],
                 "rowsSpellProgression": [[2, 0], [3, 0]]},
                {"colLabels": ["Sneak Attack", "Slot Level", "Bonus"], "rows": [
                    ["{@dice 1d6}", "1st", "+2"], ["{@dice 2d6}", "—", "+3"]
                ]}
            ]
        });
        let cols = class_columns(&class);
        let get = |var: &str| cols.iter().find(|c| c.var == var).unwrap_or_else(|| panic!("{var} missing"));
        assert_eq!(get("class.monk.martial_arts").at(1).to_string(), "1d6");
        assert_eq!(get("class.monk.focus_points").at(2), Value::int(2));
        assert_eq!(get("class.monk.unarmored_movement").at(2), Value::int(10));
        assert_eq!(get("class.monk.slots.1").at(2), Value::int(3));
        assert_eq!(get("class.monk.sneak_attack").at(2).to_string(), "2d6");
        assert_eq!(get("class.monk.slot_level").at(1), Value::int(1));
        assert_eq!(get("class.monk.slot_level").at(2), Value::int(0));
        assert_eq!(get("class.monk.bonus").at(2), Value::int(3));
        // Levels beyond the table clamp to its last row.
        assert_eq!(get("class.monk.focus_points").at(20), Value::int(2));
    }
}
