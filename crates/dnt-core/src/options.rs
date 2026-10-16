//! Lists of choices a frontend offers (pickers, menus, dropdowns): each
//! choice has a label, a hint, an optional warning, and the value to send
//! back in a command.

use dnt_data::{Kind, Library, SourceFilter};
use dnt_rules::character::{Character, Slot};
use dnt_rules::equipment;
use serde::{Deserialize, Serialize};

use crate::session::{item_entity, line_name};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub hint: String,
    /// Why it may not be a good choice (an unmet prerequisite, no slots left).
    pub warning: Option<String>,
    pub value: String,
}

impl Choice {
    pub fn new(label: impl Into<String>, hint: impl Into<String>, value: impl Into<String>) -> Choice {
        Choice { label: label.into(), hint: hint.into(), warning: None, value: value.into() }
    }
}

/// Every item and base item the campaign enables, for adding to an
/// inventory (values are item ids). Generic magic variants are offered as
/// their specific items.
pub fn item_catalog(lib: &Library, filter: &SourceFilter) -> Vec<Choice> {
    let mut items: Vec<Choice> = lib
        .entities()
        .iter()
        .filter(|e| matches!(e.kind, Kind::Item | Kind::BaseItem) && filter.allows(e))
        .map(|e| {
            let rarity = e
                .data
                .get("rarity")
                .or_else(|| e.data.get("inherits").and_then(|i| i.get("rarity")))
                .and_then(|r| r.as_str())
                .filter(|r| !matches!(*r, "none" | "unknown"))
                .unwrap_or("");
            Choice::new(e.name.clone(), format!("{rarity} {}", e.source).trim().to_string(), format!("item:{}|{}", e.name, e.source))
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

/// Where to wear an inventory line: every slot, with what it would take
/// off; the second value is the likely slot's index.
pub fn wear_slots(lib: &Library, c: &Character, line: usize) -> (Vec<Choice>, usize) {
    let l = &c.inventory[line];
    let name = line_name(lib, l);
    let guess = item_entity(lib, l).map(|e| equipment::guess_slot(c, &name, &lib.entity(e).data)).unwrap_or(Slot::Other);
    let choices = Slot::ALL
        .iter()
        .enumerate()
        .map(|(n, &slot)| {
            let occupant = c
                .inventory
                .iter()
                .enumerate()
                .find(|(j, o)| *j != line && slot != Slot::Other && o.equipped && o.slot == Some(slot))
                .map(|(_, o)| line_name(lib, o));
            let label = match slot {
                Slot::Ring1 => "Ring 1".to_string(),
                Slot::Ring2 => "Ring 2".to_string(),
                s => s.label().to_string(),
            };
            let hint = occupant.map(|o| format!("takes off {o}")).unwrap_or_else(|| slot.examples().to_string());
            Choice::new(label, hint, n.to_string())
        })
        .collect();
    (choices, Slot::ALL.iter().position(|s| *s == guess).unwrap_or(0))
}

/// Containers a line can go into (not itself or what it holds), with how
/// full they are; the first choice carries it loose (value `loose`).
pub fn containers_for(lib: &Library, c: &Character, line: usize) -> Vec<Choice> {
    let mut out = vec![Choice::new("Carried loose", "not in a container", "loose")];
    for (t, depth) in equipment::tree(c, &Default::default()) {
        let Some(k) = equipment::container(lib, &c.inventory[t]) else { continue };
        let mut at = Some(t);
        let mut inside_self = false;
        while let Some(p) = at {
            inside_self |= p == line;
            at = equipment::parent(c, p);
        }
        if inside_self {
            continue;
        }
        let fill = equipment::contents_weight(lib, c, t);
        let weightless = if k.weightless { " · weightless" } else { "" };
        let hint = match k.capacity {
            Some(cap) => format!("{fill}/{cap} lb{weightless}"),
            None => format!("{fill} lb{weightless}"),
        };
        out.push(Choice::new(format!("{}{}", "  ".repeat(depth), line_name(lib, &c.inventory[t])), hint, t.to_string()));
    }
    out
}
