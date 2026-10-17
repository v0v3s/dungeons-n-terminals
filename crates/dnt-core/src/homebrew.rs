//! Your own content: entries in a campaign's homebrew document or in the
//! global one shared by every campaign. Copying book entries, creating new
//! ones, changing, deleting and promoting them. These work on the documents
//! (JSON values); loading, saving and reloading the library is the
//! caller's.

use dnt_data::homebrew::{self, CloneMode, Template};
use dnt_data::{EntityId, Key, Kind, Library};
use dnt_store::GLOBAL_SOURCE;
use serde_json::{Map, Value};

/// Which homebrew document an entry lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Doc {
    Campaign,
    Global,
}

/// An entry being edited.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryRef {
    /// The entity's key (the original's for a campaign version).
    pub key: Key,
    pub kind: Kind,
    /// A campaign version that replaces a book entity.
    pub replaces: bool,
    pub doc: Doc,
}

impl EntryRef {
    pub fn entity(&self, lib: &Library) -> Option<EntityId> {
        lib.get(&self.key)
    }

    /// The entry as stored in `doc` (only the changed fields for a
    /// campaign version).
    pub fn entry<'a>(&self, lib: &Library, doc: &'a Value) -> Option<&'a Map<String, Value>> {
        homebrew::entry(doc, &homebrew::find_entry(doc, lib, self.entity(lib)?)?)
    }
}

/// The entry to edit for an entity: global homebrew, a campaign entry or
/// campaign version; book entities must be cloned first.
pub fn edit(lib: &Library, id: EntityId, campaign_source: Option<&str>) -> Result<EntryRef, String> {
    let e = lib.entity(id);
    if e.source.eq_ignore_ascii_case(GLOBAL_SOURCE) {
        return Ok(EntryRef { key: e.key.clone(), kind: e.kind, replaces: false, doc: Doc::Global });
    }
    let source = campaign_source.ok_or("Editing needs an active campaign (4)")?;
    let replaces = lib.original(id).is_some();
    if !replaces && !e.source.eq_ignore_ascii_case(source) {
        return Err(format!("{} is from a book: press c to clone it into the campaign", e.name));
    }
    Ok(EntryRef { key: e.key.clone(), kind: e.kind, replaces, doc: Doc::Campaign })
}

/// Clone an entity into the campaign document: a version replacing it, or
/// a separate copy under a new name.
pub fn clone(lib: &Library, doc: &mut Value, id: EntityId, source: &str, mode: CloneMode) -> Result<(EntryRef, String), String> {
    let e = lib.entity(id);
    let entry = match &mode {
        CloneMode::Replace => {
            if homebrew::find_entry(doc, lib, id).is_none() && lib.original(id).is_none() {
                homebrew::add_entry(doc, e.kind, homebrew::clone_entry(lib, id, source, &mode));
            }
            EntryRef { key: e.key.clone(), kind: e.kind, replaces: true, doc: Doc::Campaign }
        }
        CloneMode::Copy { .. } => {
            let entry = homebrew::clone_entry(lib, id, source, &mode);
            let key = Key::of_entity(e.kind, &entry).ok_or("The copy needs a name")?;
            if lib.get(&key).is_some() {
                return Err("An entry with that name already exists".into());
            }
            homebrew::add_entry(doc, e.kind, entry);
            EntryRef { key, kind: e.kind, replaces: false, doc: Doc::Campaign }
        }
    };
    let msg =
        if entry.replaces { format!("{} now has a campaign version", e.name) } else { format!("Created a campaign copy of {}", e.name) };
    Ok((entry, msg))
}

/// A new campaign entry from a template.
pub fn create(lib: &Library, doc: &mut Value, template: Template, name: &str, source: &str) -> Result<(EntryRef, String), String> {
    let entry = template.entry(name, source);
    let kind = template.kind();
    let key = Key::of_entity(kind, &entry).ok_or("The entry needs a name")?;
    if lib.get(&key).is_some() {
        return Err(format!("{name} already exists in this campaign"));
    }
    homebrew::add_entry(doc, kind, entry);
    Ok((EntryRef { key, kind, replaces: false, doc: Doc::Campaign }, format!("Created {name}")))
}

/// A change to an entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Set a field, or revert (a campaign version) or remove it (`None`).
    Set {
        field: String,
        value: Option<Value>,
    },
    /// Replace the whole entry (edited as JSON).
    Replace(Map<String, Value>),
    Delete,
}

/// Apply a change to the entry in its document. A renamed entry gets its
/// new key.
pub fn change(lib: &Library, doc: &mut Value, entry: &mut EntryRef, change: Change) -> Result<String, String> {
    let at = entry.entity(lib).and_then(|id| homebrew::find_entry(doc, lib, id)).ok_or("This entry is not in the homebrew")?;
    if change == Change::Delete {
        homebrew::delete_entry(doc, &at);
        return Ok(if entry.replaces { "Deleted the campaign version" } else { "Deleted the entry" }.into());
    }
    let old = homebrew::entry_mut(doc, &at).ok_or("The entry is gone")?;
    let msg = match change {
        Change::Set { field, value } => {
            if field == "name" && entry.replaces {
                return Err("A replacing version keeps the original's name; clone as a separate copy to rename".into());
            }
            let verb = match value {
                Some(_) => "Saved",
                None if old.contains_key("_copy") => "Reverted",
                None => "Cleared",
            };
            match value {
                Some(v) => old.insert(field.clone(), v),
                None => old.remove(&field),
            };
            format!("{verb} {field}")
        }
        Change::Replace(new) => {
            if new.get("source") != old.get("source") {
                return Err("The source cannot change; nothing changed".into());
            }
            if !new.get("name").is_some_and(Value::is_string) {
                return Err("The entry needs a name; nothing changed".into());
            }
            if entry.replaces && (new.get("_copy") != old.get("_copy") || new.get("name") != old.get("name")) {
                return Err("A campaign version keeps its name and _copy; nothing changed".into());
            }
            *old = new;
            "Saved the entry".into()
        }
        Change::Delete => unreachable!(),
    };
    if !entry.replaces {
        if let Some(k) = Key::of_entity(entry.kind, &Value::Object(old.clone())) {
            entry.key = k;
        }
    }
    Ok(msg)
}

/// What promoting changed: references to rewrite from `from` to `to`
/// (`item:Name|Source` ids), and the entry's name and old source.
#[derive(Debug, Clone, PartialEq)]
pub struct Promoted {
    pub from: String,
    pub to: String,
    pub name: String,
    pub old_source: String,
}

/// Move a campaign entry to the global document (both change; save the
/// global one first, so a failure leaves it twice rather than lost).
pub fn promote(lib: &Library, campaign: &mut Value, global: &mut Value, entry: &mut EntryRef) -> Result<Promoted, String> {
    match (entry.doc, entry.replaces) {
        (Doc::Global, _) => return Err("Already in the global homebrew".into()),
        (_, true) => return Err("A campaign version stays in its campaign; clone a separate copy to share it".into()),
        _ => {}
    }
    let id = entry.entity(lib).ok_or("The entry is gone")?;
    let at = homebrew::find_entry(campaign, lib, id).ok_or("This entry is not in the homebrew")?;
    let mut data = homebrew::entry(campaign, &at).cloned().ok_or("The entry is gone")?;
    let e = lib.entity(id);
    data.insert("source".into(), Value::String(GLOBAL_SOURCE.into()));
    let data = Value::Object(data);
    let key = Key::of_entity(entry.kind, &data).ok_or("The entry needs a name")?;
    if lib.get(&key).is_some() {
        return Err(format!("The global homebrew already has {}", e.name));
    }
    homebrew::delete_entry(campaign, &at);
    homebrew::add_entry(global, entry.kind, data);
    *entry = EntryRef { key, kind: entry.kind, replaces: false, doc: Doc::Global };
    let prefix = entry.kind.json_key();
    Ok(Promoted {
        from: format!("{prefix}:{}|{}", e.name, e.source),
        to: format!("{prefix}:{}|{GLOBAL_SOURCE}", e.name),
        name: e.name.clone(),
        old_source: e.source.clone(),
    })
}

/// "X is now global homebrew (2 character file(s) and 1 note(s) updated)".
pub fn promoted_message(name: &str, characters: usize, notes: usize) -> String {
    let mut updated = Vec::new();
    if characters > 0 {
        updated.push(format!("{characters} character file(s)"));
    }
    if notes > 0 {
        updated.push(format!("{notes} note(s)"));
    }
    if updated.is_empty() {
        format!("{name} is now global homebrew, available in every campaign")
    } else {
        format!("{name} is now global homebrew ({} updated)", updated.join(" and "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn library(docs: &[&Value]) -> Library {
        let mut lib = Library::new();
        lib.load_json(&json!({"item": [{"name": "Rope", "source": "B", "weight": 10}]}));
        for d in docs {
            lib.load_json(d);
        }
        lib
    }

    #[test]
    fn book_entries_are_cloned_before_editing() {
        let lib = library(&[]);
        let rope = dnt_rules::overlay::resolve_id(&lib, "item:Rope|B").unwrap();
        assert!(edit(&lib, rope, Some("CS")).unwrap_err().contains("clone"));
        let mut doc = json!({});
        let (entry, msg) = clone(&lib, &mut doc, rope, "CS", CloneMode::Copy { name: "Silk Rope".into() }).unwrap();
        assert_eq!(msg, "Created a campaign copy of Rope");
        assert_eq!((entry.replaces, entry.doc), (false, Doc::Campaign));
        let lib = library(&[&doc]);
        assert_eq!(edit(&lib, entry.entity(&lib).unwrap(), Some("CS")).unwrap(), entry);
        assert!(clone(&lib, &mut doc, rope, "CS", CloneMode::Copy { name: "Silk Rope".into() }).is_err(), "name taken");
    }

    #[test]
    fn entries_are_changed_renamed_and_promoted() {
        let mut doc = json!({});
        let (mut entry, _) = create(&library(&[]), &mut doc, Template::Gear, "Lamp", "CS").unwrap();
        let lib = library(&[&doc]);
        let set = |f: &str, v: Value| Change::Set { field: f.into(), value: Some(v) };
        assert_eq!(change(&lib, &mut doc, &mut entry, set("weight", json!(2))).unwrap(), "Saved weight");
        change(&lib, &mut doc, &mut entry, set("name", json!("Old Lamp"))).unwrap();
        assert_eq!(entry.key, Key::of_entity(Kind::Item, &json!({"name": "Old Lamp", "source": "CS"})).unwrap());

        let lib = library(&[&doc]);
        let mut moved = entry.entry(&lib, &doc).unwrap().clone();
        moved.insert("source".into(), json!("XX"));
        assert!(change(&lib, &mut doc, &mut entry, Change::Replace(moved)).unwrap_err().contains("source"));

        let mut global = json!({});
        let p = promote(&lib, &mut doc, &mut global, &mut entry).unwrap();
        assert_eq!((p.from.as_str(), p.to.as_str()), ("item:Old Lamp|CS", "item:Old Lamp|Homebrew"));
        assert_eq!(entry.doc, Doc::Global);
        assert_eq!(doc["item"].as_array().map(Vec::len), Some(0));
        assert_eq!(global["item"][0]["weight"], json!(2));
        assert!(promote(&library(&[&global]), &mut doc, &mut global, &mut entry).unwrap_err().contains("Already"));
    }
}
