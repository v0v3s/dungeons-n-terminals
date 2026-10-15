//! Starting equipment (class and background `startingEquipment`) and item
//! helpers for the inventory.

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use serde_json::Value as Json;

use crate::character::{Character, Hand, InventoryItem, Slot};
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
                line.hand = Some(Hand::Off);
                shield = true;
            }
            Some("light" | "medium" | "heavy") if !armor => {
                line.equipped = true;
                line.slot = Some(Slot::Body);
                armor = true;
            }
            _ => {}
        }
    }
}

// ---- containers ----

/// What a container line can hold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Container {
    /// Pounds; `None` for no limit.
    pub capacity: Option<f64>,
    /// The contents do not count toward the carried weight.
    pub weightless: bool,
}

/// The line as a container, from the item's `containerCapacity` and the
/// line's own settings.
pub fn container(lib: &Library, line: &InventoryItem) -> Option<Container> {
    // Liquid containers (only a `volume`) hold no items.
    let cap = line_data(lib, line)
        .and_then(|d| d.get("containerCapacity").or_else(|| d.get("inherits").and_then(|i| i.get("containerCapacity"))))
        .filter(|c| c.get("weight").is_some() || c.get("item").is_some() || c.get("weightless").is_some());
    if !line.container.unwrap_or(cap.is_some()) {
        return None;
    }
    let item_capacity: Option<f64> = cap
        .and_then(|c| c.get("weight"))
        .and_then(Json::as_array)
        .map(|w| w.iter().filter_map(Json::as_f64).sum())
        .filter(|w: &f64| *w > 0.0);
    let capacity = match line.capacity {
        Some(c) if c <= 0.0 => None,
        Some(c) => Some(c),
        None => item_capacity,
    };
    let item_weightless = cap.and_then(|c| c.get("weightless")).and_then(Json::as_bool) == Some(true);
    Some(Container { capacity, weightless: line.weightless.unwrap_or(item_weightless) })
}

/// The lines directly inside a container line.
pub fn contents(c: &Character, i: usize) -> Vec<usize> {
    let Some(id) = c.inventory[i].id.as_deref() else { return Vec::new() };
    (0..c.inventory.len()).filter(|&j| j != i && c.inventory[j].inside.as_deref() == Some(id)).collect()
}

/// The line's parent container, if it is in one that exists.
pub fn parent(c: &Character, i: usize) -> Option<usize> {
    let inside = c.inventory[i].inside.as_deref()?;
    (0..c.inventory.len()).find(|&j| j != i && c.inventory[j].id.as_deref() == Some(inside))
}

/// The weight of what is inside a container (all of it, weightless or not).
pub fn contents_weight(lib: &Library, c: &Character, i: usize) -> f64 {
    // (A float sum of nothing is -0.0.)
    contents(c, i).into_iter().map(|j| line_weight(lib, c, j, 0)).fold(0.0, |a, b| a + b)
}

/// A line's weight with its quantity and the contents that count.
fn line_weight(lib: &Library, c: &Character, i: usize, depth: usize) -> f64 {
    let line = &c.inventory[i];
    let own = weight(lib, line) * line.quantity as f64;
    // Depth guards against an `inside` loop edited into the file.
    if depth > 16 || container(lib, line).is_some_and(|k| k.weightless) {
        return own;
    }
    own + contents(c, i).into_iter().map(|j| line_weight(lib, c, j, depth + 1)).fold(0.0, |a, b| a + b)
}

/// What the character carries: everything not inside a weightless container.
pub fn carried_weight(lib: &Library, c: &Character) -> f64 {
    (0..c.inventory.len()).filter(|&i| parent(c, i).is_none()).map(|i| line_weight(lib, c, i, 0)).fold(0.0, |a, b| a + b)
}

/// Put line `i` into the container line `into` (`None`: carry it loose).
/// Containers get an `id` the first time something goes in.
pub fn put_in(lib: &Library, c: &mut Character, i: usize, into: Option<usize>) -> Result<String, String> {
    let name = line_name(lib, &c.inventory[i]);
    let Some(t) = into else {
        c.inventory[i].inside = None;
        return Ok(format!("{name} is carried loose"));
    };
    if container(lib, &c.inventory[t]).is_none() {
        return Err(format!("{} is not a container", line_name(lib, &c.inventory[t])));
    }
    // Not into itself or anything inside it.
    let mut at = Some(t);
    while let Some(p) = at {
        if p == i {
            return Err(format!("{name} cannot go inside itself"));
        }
        at = parent(c, p);
    }
    let id = match c.inventory[t].id.clone() {
        Some(id) => id,
        None => {
            let base = crate::class_table::slug(&line_name(lib, &c.inventory[t]));
            let mut id = base.clone();
            let mut n = 2;
            while c.inventory.iter().any(|l| l.id.as_deref() == Some(&id)) {
                id = format!("{base}_{n}");
                n += 1;
            }
            c.inventory[t].id = Some(id.clone());
            id
        }
    };
    let line = &mut c.inventory[i];
    line.inside = Some(id);
    // Things in a bag are not worn or held.
    line.equipped = false;
    line.slot = None;
    line.hand = None;
    Ok(format!("Put {name} in {}", line_name(lib, &c.inventory[t])))
}

/// Remove an inventory line; what was inside it moves to where it was.
pub fn remove_line(c: &mut Character, i: usize) -> InventoryItem {
    let removed = c.inventory.remove(i);
    if let Some(id) = &removed.id {
        for l in &mut c.inventory {
            if l.inside.as_deref() == Some(id) {
                l.inside = removed.inside.clone();
            }
        }
    }
    removed
}

/// Unpack an equipment pack: its contents replace it, inside the pack's
/// container (a backpack) if it has one.
pub fn unpack(lib: &Library, c: &mut Character, i: usize) -> Result<String, String> {
    let Some(d) = line_data(lib, &c.inventory[i]) else { return Err("Not a library item".into()) };
    let Some(list) = d.get("packContents").and_then(Json::as_array).cloned() else {
        return Err(format!("{} is not an equipment pack", line_name(lib, &c.inventory[i])));
    };
    let pack = line_name(lib, &c.inventory[i]);
    let inside = c.inventory[i].inside.clone();
    let mut new: Vec<InventoryItem> = Vec::new();
    for entry in &list {
        let (item, name, quantity) = match entry {
            Json::String(r) => (Some(format!("item:{r}")), None, 1),
            Json::Object(o) => {
                let q = o.get("quantity").and_then(Json::as_i64).unwrap_or(1);
                match (o.get("item").and_then(Json::as_str), o.get("special").and_then(Json::as_str)) {
                    (Some(r), _) => (Some(format!("item:{r}")), None, q),
                    (_, Some(text)) => (None, Some(dnt_data::tags::strip(text)), q),
                    _ => continue,
                }
            }
            _ => continue,
        };
        new.push(InventoryItem { item, name, quantity, inside: inside.clone(), ..Default::default() });
    }
    // The first container in the pack holds the rest.
    let holder = new.iter().position(|l| container(lib, l).is_some());
    if c.inventory[i].quantity > 1 {
        c.inventory[i].quantity -= 1;
    } else {
        remove_line(c, i);
    }
    let start = c.inventory.len();
    let count = new.len();
    c.inventory.extend(new);
    if let Some(h) = holder {
        for j in 0..count {
            if j != h {
                put_in(lib, c, start + j, Some(start + h))?;
            }
        }
    }
    Ok(format!("Unpacked {pack}: {count} items"))
}

/// Change a line's container settings from text: a capacity in pounds
/// (`30`), `weightless`, `no limit`, `off` (not a container) or nothing
/// (back to the item's own settings).
pub fn set_container(line: &mut InventoryItem, text: &str) -> Result<(), String> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        line.container = None;
        line.capacity = None;
        line.weightless = None;
        return Ok(());
    }
    if t == "off" || t == "no" {
        line.container = Some(false);
        line.capacity = None;
        line.weightless = None;
        return Ok(());
    }
    let mut capacity = None;
    let mut weightless = None;
    for word in t.replace("no limit", "nolimit").split([' ', ',']).filter(|w| !w.is_empty()) {
        match word.trim_end_matches('.').trim_end_matches("lbs").trim_end_matches("lb") {
            "" | "pounds" => {}
            "weightless" => weightless = Some(true),
            "weighted" => weightless = Some(false),
            "nolimit" | "unlimited" | "none" => capacity = Some(0.0),
            n => capacity = Some(n.parse::<f64>().map_err(|_| format!("not a capacity: {word:?}"))?),
        }
    }
    line.container = Some(true);
    if capacity.is_some() {
        line.capacity = capacity;
    }
    if weightless.is_some() {
        line.weightless = weightless;
    }
    Ok(())
}

/// Lines in display order: loose lines in inventory order, each container
/// followed by its contents (unless folded), with their depth.
pub fn tree(c: &Character, folded: &std::collections::BTreeSet<String>) -> Vec<(usize, usize)> {
    fn walk(c: &Character, i: usize, depth: usize, folded: &std::collections::BTreeSet<String>, out: &mut Vec<(usize, usize)>) {
        out.push((i, depth));
        if depth > 16 || c.inventory[i].id.as_ref().is_some_and(|id| folded.contains(id)) {
            return;
        }
        for j in contents(c, i) {
            walk(c, j, depth + 1, folded, out);
        }
    }
    let mut out = Vec::new();
    for i in (0..c.inventory.len()).filter(|&i| parent(c, i).is_none()) {
        walk(c, i, 0, folded, &mut out);
    }
    out
}

/// Bring older files up to date: equipped weapons and shields are put in
/// hand, other equipped items get their likely slot. Returns whether
/// anything changed.
pub fn normalize(lib: &Library, c: &mut Character) -> bool {
    let mut changed = false;
    for i in 0..c.inventory.len() {
        let line = &c.inventory[i];
        if !line.equipped || line.slot.is_some() || line.hand.is_some() {
            continue;
        }
        let Some(d) = line_data(lib, line) else { continue };
        if is_held(d) {
            let shield = armor_kind(d) == Some("shield");
            let free = |h: Hand| !c.inventory.iter().any(|l| l.hand.is_some_and(|x| if h == Hand::Main { x.uses_main() } else { x.uses_off() }));
            let hand = match (shield, free(Hand::Main), free(Hand::Off)) {
                (true, _, true) => Some(Hand::Off),
                (false, true, _) => Some(Hand::Main),
                (false, false, true) => Some(Hand::Off),
                _ => None,
            };
            let line = &mut c.inventory[i];
            line.equipped = false;
            line.hand = hand;
        } else {
            let name = line_name(lib, line);
            let mut slot = guess_slot(c, &name, d);
            if slot != Slot::Other && c.inventory.iter().any(|l| l.equipped && l.slot == Some(slot)) {
                slot = Slot::Other;
            }
            c.inventory[i].slot = Some(slot);
        }
        changed = true;
    }
    changed
}

/// The item data of an inventory line.
fn line_data<'a>(lib: &'a Library, line: &InventoryItem) -> Option<&'a Json> {
    line.item.as_deref().and_then(|id| resolve_id(lib, id)).map(|e| &lib.entity(e).data)
}

/// Whether an item is held rather than worn: weapons and shields.
pub fn is_held(d: &Json) -> bool {
    armor_kind(d) == Some("shield") || crate::attacks::Weapon::from_item("", d).is_some()
}

/// The slot an item most likely goes in, from its type and name.
pub fn guess_slot(c: &Character, name: &str, d: &Json) -> Slot {
    if matches!(armor_kind(d), Some("light" | "medium" | "heavy")) {
        return Slot::Body;
    }
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["ring of", "ring"]) && !has(&["string", "earring"]) {
        let taken = |s: Slot| c.inventory.iter().any(|l| l.equipped && l.slot == Some(s));
        return if taken(Slot::Ring1) && !taken(Slot::Ring2) { Slot::Ring2 } else { Slot::Ring1 };
    }
    if has(&["cloak", "cape", "mantle", "wings"]) {
        Slot::Shoulders
    } else if has(&["boots", "slippers", "shoes", "sandals"]) {
        Slot::Feet
    } else if has(&["gloves", "gauntlets"]) {
        Slot::Hands
    } else if has(&["bracers", "bracer"]) {
        Slot::Arms
    } else if has(&["belt", "girdle"]) {
        Slot::Waist
    } else if has(&["amulet", "necklace", "periapt", "medallion", "scarab", "talisman", "brooch"]) {
        Slot::Neck
    } else if has(&["goggles", "lenses", "eyes of", "spectacles", "monocle"]) {
        Slot::Eyes
    } else if has(&["helm", "hat", "circlet", "headband", "crown", "cap of", "diadem", "mask"]) {
        Slot::Head
    } else if has(&["robe", "armor", "mail", "vestments"]) {
        Slot::Body
    } else {
        Slot::Other
    }
}

/// Wear inventory line `i` in `slot`, taking off what was there (except in
/// `Other`). Returns the names of items taken off.
pub fn wear(lib: &Library, c: &mut Character, i: usize, slot: Slot) -> Vec<String> {
    let mut off = Vec::new();
    if slot != Slot::Other {
        for (j, other) in c.inventory.iter_mut().enumerate() {
            if j != i && other.equipped && other.slot == Some(slot) {
                other.equipped = false;
                other.slot = None;
                off.push(line_name(lib, other));
            }
        }
    }
    // Only one suit of armor, wherever it was put.
    if line_data(lib, &c.inventory[i]).is_some_and(|d| matches!(armor_kind(d), Some("light" | "medium" | "heavy"))) {
        for j in 0..c.inventory.len() {
            let other_armor = line_data(lib, &c.inventory[j]).is_some_and(|d| matches!(armor_kind(d), Some("light" | "medium" | "heavy")));
            if j != i && c.inventory[j].equipped && other_armor {
                c.inventory[j].equipped = false;
                c.inventory[j].slot = None;
                off.push(line_name(lib, &c.inventory[j]));
            }
        }
    }
    let line = &mut c.inventory[i];
    line.equipped = true;
    line.slot = Some(slot);
    line.hand = None;
    off
}

pub fn take_off(c: &mut Character, i: usize) {
    let line = &mut c.inventory[i];
    line.equipped = false;
    line.slot = None;
}

fn line_name(lib: &Library, line: &InventoryItem) -> String {
    line.name
        .clone()
        .or_else(|| line.item.as_deref().and_then(|id| resolve_id(lib, id)).map(|e| lib.entity(e).name.clone()))
        .unwrap_or_else(|| "?".into())
}

/// Take inventory line `i` in `hand`, or put it away if it already is
/// there. Two-handed weapons take both hands; asking for the other hand on
/// a held line with two or more items holds a pair. Whatever was in the
/// hands needed is put away. Returns what happened, for the status line.
pub fn wield(lib: &Library, c: &mut Character, i: usize, hand: Hand) -> String {
    let d = line_data(lib, &c.inventory[i]);
    let name = line_name(lib, &c.inventory[i]);
    let shield = d.is_some_and(|d| armor_kind(d) == Some("shield"));
    let two_handed = !shield && d.and_then(|d| crate::attacks::Weapon::from_item("", d)).is_some_and(|w| w.has("2H"));
    let current = c.inventory[i].hand;
    let quantity = c.inventory[i].quantity;
    if current == Some(hand) || (two_handed && current.is_some()) {
        c.inventory[i].hand = None;
        return format!("Put away {name}");
    }
    let versatile = d.and_then(|d| crate::attacks::Weapon::from_item("", d)).is_some_and(|w| w.has("V"));
    if hand == Hand::Both && !two_handed && !versatile {
        return format!("{name} is not used with two hands");
    }
    let new = match (current, hand) {
        _ if two_handed => Hand::Both,
        (Some(Hand::Main), Hand::Off) | (Some(Hand::Off), Hand::Main) if quantity >= 2 && !shield => Hand::Pair,
        // Asking for the main hand on a pair (or a two-handed grip): just the main hand.
        (Some(Hand::Pair | Hand::Both), Hand::Main) => Hand::Main,
        (Some(Hand::Pair | Hand::Both), Hand::Off) => Hand::Off,
        (_, h) => h,
    };
    let mut stowed = Vec::new();
    for (j, other) in c.inventory.iter_mut().enumerate() {
        let Some(h) = other.hand.filter(|_| j != i) else { continue };
        let clash_main = new.uses_main() && h.uses_main();
        let clash_off = new.uses_off() && h.uses_off();
        if !clash_main && !clash_off {
            continue;
        }
        // A pair keeps the hand that is still free.
        other.hand = match h {
            Hand::Pair if !clash_main => Some(Hand::Main),
            Hand::Pair if !clash_off => Some(Hand::Off),
            _ => None,
        };
        if other.hand.is_none() {
            stowed.push(line_name(lib, other));
        }
    }
    let line = &mut c.inventory[i];
    line.hand = Some(new);
    line.equipped = false;
    line.slot = None;
    let mut msg = format!("{name}: {}", new.label());
    if !stowed.is_empty() {
        msg.push_str(&format!(" (put away {})", stowed.join(", ")));
    }
    msg
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

    #[test]
    fn slots() {
        let mut lib = Library::new();
        lib.load_json(&json!({"item": [
            {"name": "Cloak of Protection", "source": "T", "wondrous": true},
            {"name": "Cloak of Elvenkind", "source": "T", "wondrous": true},
            {"name": "Ring of Protection", "source": "T"},
            {"name": "Ring of Warmth", "source": "T"},
            {"name": "Chain Mail", "source": "T", "type": "HA|T", "ac": 16},
            {"name": "Bag of Holding", "source": "T", "wondrous": true}
        ]}));
        let mut c = Character::default();
        for n in ["Cloak of Protection", "Cloak of Elvenkind", "Ring of Protection", "Ring of Warmth", "Chain Mail", "Bag of Holding"] {
            add_item(&mut c, &format!("item:{n}|T"), 1, None);
        }
        let guess = |c: &Character, i: usize| {
            let e = resolve_id(&lib, c.inventory[i].item.as_deref().unwrap()).unwrap();
            guess_slot(c, &lib.entity(e).name, &lib.entity(e).data)
        };
        assert_eq!(guess(&c, 0), Slot::Shoulders);
        assert_eq!(guess(&c, 4), Slot::Body);
        assert_eq!(guess(&c, 5), Slot::Other);
        // One cloak at a time.
        wear(&lib, &mut c, 0, Slot::Shoulders);
        assert_eq!(wear(&lib, &mut c, 1, Slot::Shoulders), vec!["Cloak of Protection"]);
        assert!(!c.inventory[0].equipped && c.inventory[1].equipped);
        // Two rings: the second goes to the free ring slot.
        let s = guess(&c, 2);
        wear(&lib, &mut c, 2, s);
        assert_eq!(guess(&c, 3), Slot::Ring2);
    }

    #[test]
    fn containers() {
        let mut lib = Library::new();
        lib.load_json(&json!({"item": [
            {"name": "Backpack", "source": "T", "weight": 5, "containerCapacity": {"weight": [30]}},
            {"name": "Bag of Holding", "source": "T", "weight": 5, "containerCapacity": {"weight": [500], "weightless": true}},
            {"name": "Rope", "source": "T", "weight": 5},
            {"name": "Torch", "source": "T", "weight": 1},
            {"name": "Explorer's Pack", "source": "T", "weight": 55, "packContents": [
                "backpack|t", {"item": "torch|t", "quantity": 10}, "rope|t", {"special": "a {@b tin} cup"}]}
        ]}));
        let mut c = Character::default();
        add_item(&mut c, "item:Explorer's Pack|T", 1, None);
        add_item(&mut c, "item:Bag of Holding|T", 1, None);
        assert_eq!(carried_weight(&lib, &c), 60.0);

        // Unpacking: the backpack holds the rest.
        unpack(&lib, &mut c, 0).unwrap();
        let names: Vec<String> = c.inventory.iter().map(|l| line_name(&lib, l)).collect();
        assert_eq!(names, ["Bag of Holding", "Backpack", "Torch", "Rope", "a tin cup"]);
        let backpack = 1;
        assert_eq!(contents(&c, backpack), vec![2, 3, 4]);
        assert_eq!(contents_weight(&lib, &c, backpack), 15.0);
        assert_eq!(carried_weight(&lib, &c), 5.0 + 5.0 + 15.0);

        // Into the Bag of Holding: its contents weigh nothing.
        put_in(&lib, &mut c, backpack, Some(0)).unwrap();
        assert_eq!(carried_weight(&lib, &c), 5.0);
        assert_eq!(contents_weight(&lib, &c, 0), 20.0);
        // No loops.
        assert!(put_in(&lib, &mut c, 0, Some(backpack)).is_err());
        // Display order: containers then their contents, unless folded.
        let mut folded = std::collections::BTreeSet::new();
        assert_eq!(tree(&c, &folded).iter().map(|(i, d)| (*i, *d)).collect::<Vec<_>>(), [(0, 0), (1, 1), (2, 2), (3, 2), (4, 2)]);
        folded.insert(c.inventory[1].id.clone().unwrap());
        assert_eq!(tree(&c, &folded).len(), 2);

        // Removing the backpack spills its contents into the bag.
        remove_line(&mut c, 1);
        assert!(c.inventory.iter().skip(1).all(|l| l.inside.as_deref() == Some("bag_of_holding")));

        // Settings: any item can become a container.
        let mut rope = c.inventory[2].clone();
        assert!(container(&lib, &rope).is_none());
        set_container(&mut rope, "20 lb weightless").unwrap();
        assert_eq!(container(&lib, &rope), Some(Container { capacity: Some(20.0), weightless: true }));
        set_container(&mut rope, "no limit").unwrap();
        assert_eq!(container(&lib, &rope).unwrap().capacity, None);
        set_container(&mut rope, "").unwrap();
        assert!(container(&lib, &rope).is_none());
        assert!(set_container(&mut rope, "lots").is_err());
    }
}