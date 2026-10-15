//! Starting equipment (class and background `startingEquipment`) and item
//! helpers for the inventory.

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::{Character, InventoryItem};
use crate::overlay::resolve_id;

/// One entry of a starting-equipment option.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    /// A library item, e.g. `item:Chain Mail|XPHB`.
    Item { id: String, quantity: i64, name: Option<String> },
    /// Coins, in copper pieces.
    Coins(i64),
    /// "Any martial weapon" and the like; resolved by a further choice.
    AnyOf { kind: String, quantity: i64 },
    /// Free text: "sticks of incense".
    Special(String),
}

/// One alternative within a group, e.g. option "A".
#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    pub key: String,
    pub entries: Vec<Entry>,
}

/// Parse starting equipment: a list of groups, each offering one or more
/// alternatives (`_` means always granted). Classes wrap the list in
/// `defaultData`; backgrounds give it directly.
pub fn starting_equipment(v: Option<&Json>) -> Vec<Vec<Alternative>> {
    let groups = v.and_then(|s| s.get("defaultData").or(Some(s))).and_then(Json::as_array);
    let Some(groups) = groups else {
        return Vec::new();
    };
    groups
        .iter()
        .filter_map(Json::as_object)
        .map(|g| {
            let mut alts: Vec<Alternative> = g
                .iter()
                .map(|(key, list)| Alternative {
                    key: key.to_uppercase(),
                    entries: list.as_array().into_iter().flatten().filter_map(entry).collect(),
                })
                .collect();
            alts.sort_by(|a, b| a.key.cmp(&b.key));
            alts
        })
        .filter(|g: &Vec<Alternative>| !g.is_empty())
        .collect()
}

fn entry(e: &Json) -> Option<Entry> {
    let item_id = |s: &str| format!("item:{s}");
    Some(match e {
        Json::String(s) => Entry::Item {
            id: item_id(s),
            quantity: 1,
            name: None,
        },
        Json::Object(o) => {
            let quantity = o.get("quantity").and_then(Json::as_i64).unwrap_or(1);
            if let Some(i) = o.get("item").and_then(Json::as_str) {
                Entry::Item {
                    id: item_id(i),
                    quantity,
                    name: o.get("displayName").and_then(Json::as_str).map(str::to_string),
                }
            } else if let Some(v) = o.get("value").and_then(Json::as_i64) {
                Entry::Coins(v)
            } else if let Some(t) = o.get("equipmentType").and_then(Json::as_str) {
                Entry::AnyOf {
                    kind: t.to_string(),
                    quantity,
                }
            } else {
                Entry::Special(o.get("special").and_then(Json::as_str)?.to_string())
            }
        }
        _ => return None,
    })
}

/// Human-readable name of an "any …" equipment type.
pub fn any_label(kind: &str) -> &'static str {
    match kind {
        "weaponMartial" => "a martial weapon",
        "weaponMartialMelee" => "a martial melee weapon",
        "weaponSimple" => "a simple weapon",
        "weaponSimpleMelee" => "a simple melee weapon",
        "focusSpellcastingArcane" => "an arcane focus",
        "focusSpellcastingDruidic" => "a druidic focus",
        "focusSpellcastingHoly" => "a holy symbol",
        "instrumentMusical" => "a musical instrument",
        "setGaming" => "a gaming set",
        "toolArtisan" => "artisan's tools",
        _ => "an item",
    }
}

fn item_type(e: &Json) -> &str {
    e.get("type").and_then(Json::as_str).and_then(|t| t.split('|').next()).unwrap_or("")
}

/// Base items matching an "any …" equipment type.
pub fn items_of_kind(lib: &Library, kind: &str, filter: &SourceFilter) -> Vec<EntityId> {
    let mut out: Vec<EntityId> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            // Weapons and armor are base items; mundane gear (dice, tools,
            // instruments, holy symbols) lives among the items.
            let mundane = e.data.get("rarity").and_then(Json::as_str).is_none_or(|r| r == "none");
            if !(e.kind == Kind::BaseItem || (e.kind == Kind::Item && mundane)) || !filter.allows(e) {
                return false;
            }
            let d = &e.data;
            let category = d.get("weaponCategory").and_then(Json::as_str);
            let melee = item_type(d) == "M";
            let scf = d.get("scfType").and_then(Json::as_str);
            match kind {
                "weaponMartial" => category == Some("martial"),
                "weaponMartialMelee" => category == Some("martial") && melee,
                "weaponSimple" => category == Some("simple"),
                "weaponSimpleMelee" => category == Some("simple") && melee,
                "focusSpellcastingArcane" => scf == Some("arcane"),
                "focusSpellcastingDruidic" => scf == Some("druid"),
                "focusSpellcastingHoly" => scf == Some("holy"),
                "instrumentMusical" => item_type(d) == "INS",
                "setGaming" => item_type(d) == "GS",
                "toolArtisan" => item_type(d) == "AT",
                _ => false,
            }
        })
        .collect();
    out.sort_by(|&a, &b| lib.entity(a).name.cmp(&lib.entity(b).name));
    out
}

/// A short description of an alternative: "Chain Mail, Greatsword, 8 × Javelin, 4 gp".
pub fn describe(lib: &Library, alt: &Alternative) -> String {
    alt.entries
        .iter()
        .map(|e| match e {
            Entry::Item { id, quantity, name } => {
                let n = name
                    .clone()
                    .or_else(|| resolve_id(lib, id).map(|i| lib.entity(i).name.clone()))
                    .unwrap_or_else(|| id.trim_start_matches("item:").split('|').next().unwrap_or("").to_string());
                if *quantity > 1 {
                    format!("{quantity} × {n}")
                } else {
                    n
                }
            }
            Entry::Coins(cp) => coins(*cp),
            Entry::AnyOf { kind, quantity } if *quantity > 1 => format!("{quantity} × {}", any_label(kind).trim_start_matches("a ").trim_start_matches("an ")),
            Entry::AnyOf { kind, .. } => any_label(kind).to_string(),
            Entry::Special(s) => s.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn coins(cp: i64) -> String {
    match (cp / 100, cp % 100) {
        (gp, 0) => format!("{gp} gp"),
        (0, r) if r % 10 == 0 => format!("{} sp", r / 10),
        (0, r) => format!("{r} cp"),
        (gp, r) if r % 10 == 0 => format!("{gp} gp {} sp", r / 10),
        (gp, r) => format!("{gp} gp {r} cp"),
    }
}

/// Add an alternative's concrete entries to the character (items and
/// coins). `AnyOf` entries must be resolved by the caller first.
pub fn grant(c: &mut Character, alt: &Alternative) {
    for e in &alt.entries {
        match e {
            Entry::Item { id, quantity, name } => add_item(c, id, *quantity, name.clone()),
            Entry::Coins(cp) => c.currency.add_cp(*cp),
            Entry::Special(s) => c.inventory.push(InventoryItem {
                name: Some(s.clone()),
                quantity: 1,
                ..Default::default()
            }),
            Entry::AnyOf { .. } => {}
        }
    }
}

/// Add an item, stacking with an identical line.
pub fn add_item(c: &mut Character, id: &str, quantity: i64, name: Option<String>) {
    match c.inventory.iter_mut().find(|i| i.item.as_deref() == Some(id) && i.name == name) {
        Some(line) => line.quantity += quantity,
        None => c.inventory.push(InventoryItem {
            item: Some(id.to_string()),
            name,
            quantity,
            ..Default::default()
        }),
    }
}

/// Armor category of an item: `light`, `medium`, `heavy`, `shield`.
pub fn armor_kind(d: &Json) -> Option<&'static str> {
    let d = d.get("inherits").unwrap_or(d);
    Some(match item_type(d) {
        "LA" => "light",
        "MA" => "medium",
        "HA" => "heavy",
        "S" => "shield",
        _ => return None,
    })
}

pub fn requires_attunement(d: &Json) -> bool {
    d.get("reqAttune").or_else(|| d.get("inherits").and_then(|i| i.get("reqAttune"))).is_some_and(|v| v.as_bool() != Some(false))
}

/// Weight of one item in pounds.
pub fn weight(lib: &Library, line: &InventoryItem) -> f64 {
    line.weight
        .or_else(|| {
            let e = resolve_id(lib, line.item.as_deref()?)?;
            let d = &lib.entity(e).data;
            d.get("weight").or_else(|| d.get("inherits").and_then(|i| i.get("weight"))).and_then(Json::as_f64)
        })
        .unwrap_or(0.0)
}

/// Equip the first armor and shield found, as a sensible default after
/// starting equipment is granted.
pub fn auto_equip(lib: &Library, c: &mut Character) {
    let mut armor = false;
    let mut shield = false;
    for line in &mut c.inventory {
        let Some(e) = line.item.as_deref().and_then(|id| resolve_id(lib, id)) else { continue };
        match armor_kind(&lib.entity(e).data) {
            Some("shield") if !shield => {
                line.equipped = true;
                shield = true;
            }
            Some("light" | "medium" | "heavy") if !armor => {
                line.equipped = true;
                armor = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_2024_and_2014_shapes() {
        let fighter24 = json!({"defaultData": [{
            "A": [{"item": "chain mail|xphb"}, {"item": "javelin|xphb", "quantity": 8}, {"value": 400}],
            "C": [{"value": 15500}]
        }]});
        let groups = starting_equipment(Some(&fighter24));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0][0].key, "A");
        assert_eq!(groups[0][0].entries[1], Entry::Item { id: "item:javelin|xphb".into(), quantity: 8, name: None });
        assert_eq!(groups[0][1].entries, vec![Entry::Coins(15500)]);

        let fighter14 = json!({"defaultData": [
            {"a": ["chain mail|phb"], "b": ["leather armor|phb", "longbow|phb"]},
            {"a": [{"equipmentType": "weaponMartial"}, "shield|phb"], "b": [{"equipmentType": "weaponMartial", "quantity": 2}]},
            {"_": [{"special": "incense"}]}
        ]});
        let groups = starting_equipment(Some(&fighter14));
        assert_eq!(groups.len(), 3);
        // Backgrounds give the list without `defaultData`.
        let soldier = json!([{"A": [{"item": "spear|xphb"}, {"equipmentType": "setGaming"}], "B": [{"value": 5000}]}]);
        assert_eq!(starting_equipment(Some(&soldier))[0].len(), 2);
        assert_eq!(groups[1][1].entries[0], Entry::AnyOf { kind: "weaponMartial".into(), quantity: 2 });
        assert_eq!(groups[2][0].key, "_");
    }

    #[test]
    fn granting_stacks_items_and_coins() {
        let mut c = Character::default();
        let alt = Alternative {
            key: "A".into(),
            entries: vec![
                Entry::Item { id: "item:dagger|xphb".into(), quantity: 1, name: None },
                Entry::Item { id: "item:dagger|xphb".into(), quantity: 1, name: None },
                Entry::Coins(1550),
                Entry::Special("incense".into()),
            ],
        };
        grant(&mut c, &alt);
        assert_eq!(c.inventory.len(), 2);
        assert_eq!(c.inventory[0].quantity, 2);
        assert_eq!((c.currency.gp, c.currency.sp), (15, 5));
        assert_eq!(coins(1550), "15 gp 5 sp");
        assert_eq!(coins(1507), "15 gp 7 cp");
        assert_eq!(coins(50), "5 sp");
    }
}
