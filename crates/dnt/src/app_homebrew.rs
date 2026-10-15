//! Homebrew flows of the app: cloning, creating, editing and promoting
//! entries of the campaign and global homebrew documents.

use std::path::Path;

use dnt_data::homebrew::{self, CloneMode, Template};
use dnt_data::{EntityId, Key, Kind, Library};
use dnt_store::GLOBAL_SOURCE;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::app::{App, Section};
use crate::entry_editor::{self, EntryEditor, Target};

/// Choosing between replacing the original and a separate copy.
pub struct CloneFlow {
    pub entity: EntityId,
    /// Name for a separate copy, while typing it.
    pub name: Option<String>,
    /// 0 = replace, 1 = separate copy.
    pub cursor: usize,
}

/// Creating a new entry: pick a template, then name it.
pub struct NewFlow {
    pub templates: &'static [Template],
    pub cursor: usize,
    /// The name, once a template is chosen.
    pub name: Option<String>,
    /// Add the new item to the open character's inventory.
    pub for_inventory: bool,
}

impl NewFlow {
    pub fn template(&self) -> Template {
        self.templates[self.cursor]
    }
}

impl App {
    /// The homebrew document an editor target lives in (cached).
    pub fn target_doc(&self, target: Target) -> Option<&Value> {
        match target {
            Target::Campaign => self.homebrew.as_ref(),
            Target::Global => self.global_homebrew.as_ref(),
        }
    }

    fn load_doc(&self, target: Target) -> Result<Value, String> {
        match target {
            Target::Campaign => {
                let campaign = self.active_campaign().ok_or("No active campaign")?;
                self.store.homebrew(campaign).map_err(|e| format!("Could not read homebrew: {e}"))
            }
            Target::Global => self.store.global_homebrew().map_err(|e| format!("Could not read global homebrew: {e}")),
        }
    }

    fn save_doc(&mut self, target: Target, doc: Value) -> Result<(), String> {
        match target {
            Target::Campaign => {
                let campaign = self.active_campaign().ok_or("No active campaign")?;
                self.store.save_homebrew(campaign, &doc).map_err(|e| format!("Could not save homebrew: {e}"))?;
                self.homebrew = Some(doc);
                self.pending_reload = true;
            }
            Target::Global => {
                self.store.save_global_homebrew(&doc).map_err(|e| format!("Could not save global homebrew: {e}"))?;
                self.global_homebrew = Some(doc);
                self.pending_base_reload = true;
            }
        }
        Ok(())
    }

    fn open_editor(&mut self, editor: EntryEditor) {
        if self.section != Section::Compendium {
            self.editor_return = Some(self.section);
            self.section = Section::Compendium;
        }
        self.editor = Some(editor);
    }

    fn close_editor(&mut self) {
        self.editor = None;
        if let Some(s) = self.editor_return.take() {
            self.section = s;
        }
    }

    pub(crate) fn start_edit(&mut self, lib: &Library, id: EntityId) {
        let e = lib.entity(id);
        if e.source.eq_ignore_ascii_case(GLOBAL_SOURCE) {
            return self.open_editor(EntryEditor::new(e.key.clone(), e.kind, false, Target::Global));
        }
        let Some(campaign) = self.active_campaign() else {
            self.status = "Editing needs an active campaign (4)".into();
            return;
        };
        let replaces = lib.original(id).is_some();
        if !replaces && !e.source.eq_ignore_ascii_case(&campaign.settings.source) {
            self.status = format!("{} is from a book: press c to clone it into the campaign", e.name);
            return;
        }
        self.open_editor(EntryEditor::new(e.key.clone(), e.kind, replaces, Target::Campaign));
    }

    pub(crate) fn clone_key(&mut self, lib: &Library, key: KeyEvent) {
        let Some(flow) = self.clone_flow.as_mut() else { return };
        if let Some(name) = flow.name.as_mut() {
            match key.code {
                KeyCode::Esc => flow.name = None,
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => name.clear(),
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => name.push(c),
                KeyCode::Enter if !name.trim().is_empty() => {
                    let (id, name) = (flow.entity, name.trim().to_string());
                    self.clone_flow = None;
                    self.do_clone(lib, id, CloneMode::Copy { name });
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => self.clone_flow = None,
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Char('k') | KeyCode::Up | KeyCode::Tab => flow.cursor = 1 - flow.cursor,
            KeyCode::Enter => {
                if flow.cursor == 0 {
                    let id = flow.entity;
                    self.clone_flow = None;
                    self.do_clone(lib, id, CloneMode::Replace);
                } else {
                    flow.name = Some(format!("{} (copy)", lib.entity(flow.entity).name));
                }
            }
            _ => {}
        }
    }

    fn do_clone(&mut self, lib: &Library, id: EntityId, mode: CloneMode) {
        let Some(campaign) = self.active_campaign().cloned() else { return };
        let mut doc = match self.load_doc(Target::Campaign) {
            Ok(d) => d,
            Err(e) => return self.status = e,
        };
        let e = lib.entity(id);
        let (key, replaces) = match &mode {
            CloneMode::Replace => {
                if homebrew::find_entry(&doc, lib, id).is_none() && lib.original(id).is_none() {
                    let entry = homebrew::clone_entry(lib, id, &campaign.settings.source, &mode);
                    homebrew::add_entry(&mut doc, e.kind, entry);
                }
                (e.key.clone(), true)
            }
            CloneMode::Copy { .. } => {
                let entry = homebrew::clone_entry(lib, id, &campaign.settings.source, &mode);
                let Some(key) = Key::of_entity(e.kind, &entry) else { return };
                if lib.get(&key).is_some() {
                    self.status = "An entry with that name already exists".into();
                    return;
                }
                homebrew::add_entry(&mut doc, e.kind, entry);
                (key, false)
            }
        };
        if let Err(err) = self.save_doc(Target::Campaign, doc) {
            return self.status = err;
        }
        self.status =
            if replaces { format!("{} now has a campaign version", e.name) } else { format!("Created a campaign copy of {}", e.name) };
        self.open_editor(EntryEditor::new(key, e.kind, replaces, Target::Campaign));
    }

    pub(crate) fn start_new(&mut self, templates: &'static [Template], for_inventory: bool) {
        if self.active.is_none() {
            self.status = "Creating entries needs an active campaign (4)".into();
            return;
        }
        let name = (templates.len() == 1).then(String::new);
        self.new_flow = Some(NewFlow { templates, cursor: 0, name, for_inventory });
    }

    pub(crate) fn new_key(&mut self, lib: &Library, key: KeyEvent) {
        let Some(flow) = self.new_flow.as_mut() else { return };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let template = flow.template();
        if let Some(name) = flow.name.as_mut() {
            match key.code {
                KeyCode::Esc if flow.templates.len() == 1 => self.new_flow = None,
                KeyCode::Esc => flow.name = None,
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Char('u') if ctrl => name.clear(),
                KeyCode::Char(c) if !ctrl => name.push(c),
                KeyCode::Enter => {
                    let name = match name.trim() {
                        "" => format!("New {}", template.label().to_lowercase()),
                        n => n.to_string(),
                    };
                    let for_inventory = flow.for_inventory;
                    self.new_flow = None;
                    self.do_create(lib, template, &name, for_inventory);
                }
                _ => {}
            }
            return;
        }
        let n = flow.templates.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.new_flow = None,
            KeyCode::Char('j') | KeyCode::Down => flow.cursor = (flow.cursor + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => flow.cursor = (flow.cursor + n - 1) % n,
            KeyCode::Enter | KeyCode::Char(' ') => flow.name = Some(String::new()),
            _ => {}
        }
    }

    fn do_create(&mut self, lib: &Library, template: Template, name: &str, for_inventory: bool) {
        let Some(campaign) = self.active_campaign().cloned() else { return };
        let mut doc = match self.load_doc(Target::Campaign) {
            Ok(d) => d,
            Err(e) => return self.status = e,
        };
        let entry = template.entry(name, &campaign.settings.source);
        let kind = template.kind();
        let Some(key) = Key::of_entity(kind, &entry) else { return };
        if lib.get(&key).is_some() {
            self.status = format!("{name} already exists in this campaign");
            return;
        }
        homebrew::add_entry(&mut doc, kind, entry);
        if let Err(e) = self.save_doc(Target::Campaign, doc) {
            return self.status = e;
        }
        self.status = format!("Created {name}");
        if for_inventory {
            if let Some(sheet) = self.characters.sheet.as_mut() {
                let id = format!("item:{name}|{}", campaign.settings.source);
                dnt_rules::equipment::add_item(&mut sheet.file.character, &id, 1, None);
                sheet.inventory_cursor = sheet.file.character.inventory.len().saturating_sub(1);
                self.status = match self.store.save_character(&sheet.file) {
                    Ok(()) => format!("Created {name} and added it to {}", sheet.file.character.name),
                    Err(e) => format!("Created {name}, but could not save the character: {e}"),
                };
            }
        }
        self.open_editor(EntryEditor::new(key, kind, false, Target::Campaign));
    }

    pub(crate) fn editor_key(&mut self, lib: &Library, key: KeyEvent) {
        let Some(editor) = self.editor.as_ref() else { return };
        let target = editor.target;
        let entry = editor
            .entity(lib)
            .and_then(|id| self.target_doc(target).and_then(|doc| Some((doc, homebrew::find_entry(doc, lib, id)?))))
            .and_then(|(doc, at)| homebrew::entry(doc, &at).cloned());
        let editor = self.editor.as_mut().unwrap();
        let fields = editor.fields(lib, entry.as_ref());
        match editor.key(key, &fields, lib) {
            entry_editor::Action::None => {}
            entry_editor::Action::Close => self.close_editor(),
            entry_editor::Action::Status(s) => self.status = s,
            entry_editor::Action::Set(field, value) => self.edit_entry(lib, |doc, at, editor| {
                let entry = homebrew::entry_mut(doc, at).ok_or("The entry is gone")?;
                let verb = match value {
                    Some(_) => "Saved",
                    None if entry.contains_key("_copy") => "Reverted",
                    None => "Cleared",
                };
                match value {
                    Some(v) => entry.insert(field.clone(), v),
                    None => entry.remove(&field),
                };
                // Renaming a separate entry changes its key.
                if field == "name" && !editor.replaces {
                    if let Some(k) = Key::of_entity(editor.kind, &Value::Object(entry.clone())) {
                        editor.key = k;
                    }
                }
                Ok(format!("{verb} {field}"))
            }),
            entry_editor::Action::Delete => {
                self.edit_entry(lib, |doc, at, editor| {
                    homebrew::delete_entry(doc, at).then_some(()).ok_or("The entry is gone")?;
                    Ok(if editor.replaces { "Deleted the campaign version" } else { "Deleted the entry" }.to_string())
                });
                self.close_editor();
            }
            entry_editor::Action::External => self.external_edit(lib),
            entry_editor::Action::Promote => self.promote(lib),
        }
    }

    /// Change the edited entity's entry in its homebrew document and save.
    fn edit_entry(
        &mut self,
        lib: &Library,
        change: impl FnOnce(&mut Value, &(String, usize), &mut EntryEditor) -> Result<String, &'static str>,
    ) {
        let Some(target) = self.editor.as_ref().map(|e| e.target) else { return };
        let mut doc = match self.load_doc(target) {
            Ok(d) => d,
            Err(e) => return self.status = e,
        };
        let editor = self.editor.as_mut().unwrap();
        let Some(at) = editor.entity(lib).and_then(|id| homebrew::find_entry(&doc, lib, id)) else {
            self.status = "This entry is not in the homebrew".into();
            return;
        };
        match change(&mut doc, &at, editor) {
            Ok(msg) => {
                self.status = match self.save_doc(target, doc) {
                    Ok(()) => msg,
                    Err(e) => e,
                }
            }
            Err(e) => self.status = e.to_string(),
        }
    }

    /// Move a campaign entry to the global homebrew and point the campaign's
    /// characters at it.
    fn promote(&mut self, lib: &Library) {
        let Some(editor) = self.editor.as_ref() else { return };
        let (kind, Some(id)) = (editor.kind, editor.entity(lib)) else { return };
        let (mut cdoc, mut gdoc) = match (self.load_doc(Target::Campaign), self.load_doc(Target::Global)) {
            (Ok(c), Ok(g)) => (c, g),
            (Err(e), _) | (_, Err(e)) => return self.status = e,
        };
        let Some(at) = homebrew::find_entry(&cdoc, lib, id) else { return };
        let Some(mut entry) = homebrew::entry(&cdoc, &at).cloned() else { return };
        let e = lib.entity(id);
        entry.insert("source".into(), Value::String(GLOBAL_SOURCE.into()));
        let entry = Value::Object(entry);
        let Some(new_key) = Key::of_entity(kind, &entry) else { return };
        if lib.get(&new_key).is_some() {
            self.status = format!("The global homebrew already has {}", e.name);
            return;
        }
        homebrew::delete_entry(&mut cdoc, &at);
        homebrew::add_entry(&mut gdoc, kind, entry);
        // Global first: if saving the campaign fails, the entry exists twice
        // rather than not at all.
        if let Err(err) = self.save_doc(Target::Global, gdoc).and_then(|_| self.save_doc(Target::Campaign, cdoc)) {
            return self.status = err;
        }
        let from = format!("{}:{}|{}", kind.json_key(), e.name, e.source);
        let to = format!("{}:{}|{GLOBAL_SOURCE}", kind.json_key(), e.name);
        let characters = self.active_campaign().map(|c| self.store.rewrite_refs(c, &from, &to)).transpose();
        // Note links that name the old source: `[[item:X@CoS]]`.
        self.notes.save();
        let kind_ok = |tag: &str| Kind::for_tag(tag).contains(&kind) || Kind::from_json_key(tag) == Some(kind);
        let notes = self.notes.notes.as_ref().map(|n| n.rewrite_entity_source(&kind_ok, &e.name, &e.source, GLOBAL_SOURCE)).transpose();
        self.notes.reload_open();
        self.status = match (characters, notes) {
            (Ok(c), Ok(n)) => {
                let mut updated = Vec::new();
                if let Some(c @ 1..) = c {
                    updated.push(format!("{c} character file(s)"));
                }
                if let Some(n @ 1..) = n {
                    updated.push(format!("{n} note(s)"));
                }
                if updated.is_empty() {
                    format!("{} is now global homebrew, available in every campaign", e.name)
                } else {
                    format!("{} is now global homebrew ({} updated)", e.name, updated.join(" and "))
                }
            }
            (Err(err), _) | (_, Err(err)) => format!("{} is now global homebrew, but updating references failed: {err}", e.name),
        };
        // The characters on disk changed; the open sheet must not save stale refs.
        self.characters.files = self.active_campaign().map(|c| self.store.characters(c).0).unwrap_or_default();
        if let Some(path) = self.characters.sheet.as_ref().map(|s| s.file.path.clone()) {
            if let Ok(file) = self.store.load_character(&path) {
                self.characters.reload(file, lib, &self.overlays);
            }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.key = new_key;
            editor.target = Target::Global;
        }
    }

    /// Write the entry to a temporary file for $EDITOR.
    fn external_edit(&mut self, lib: &Library) {
        let Some(editor) = self.editor.as_ref() else { return };
        let Some(entry) = editor
            .entity(lib)
            .and_then(|id| self.target_doc(editor.target).and_then(|doc| Some((doc, homebrew::find_entry(doc, lib, id)?))))
            .and_then(|(doc, at)| homebrew::entry(doc, &at).cloned())
        else {
            return;
        };
        let path = std::env::temp_dir().join(format!("dnt-entry-{}.json", std::process::id()));
        let text = serde_json::to_string_pretty(&Value::Object(entry)).expect("JSON serialises") + "\n";
        match std::fs::write(&path, text) {
            Ok(()) => {
                self.entry_edit_file = Some(path.clone());
                self.pending_edit = Some(path);
            }
            Err(e) => self.status = format!("Could not write {}: {e}", path.display()),
        }
    }

    /// Validate and apply an entry edited in $EDITOR.
    pub(crate) fn after_entry_edit(&mut self, lib: &Library, path: &Path) {
        self.entry_edit_file = None;
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let _ = std::fs::remove_file(path);
        let new = match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(o)) => o,
            Ok(_) => return self.status = "The entry must be a JSON object; nothing changed".into(),
            Err(e) => return self.status = format!("Not valid JSON ({e}); nothing changed"),
        };
        self.edit_entry(lib, move |doc, at, editor| {
            let old = homebrew::entry_mut(doc, at).ok_or("The entry is gone")?;
            if new.get("source") != old.get("source") {
                return Err("The source cannot change; nothing changed");
            }
            if !new.get("name").is_some_and(Value::is_string) {
                return Err("The entry needs a name; nothing changed");
            }
            if editor.replaces && (new.get("_copy") != old.get("_copy") || new.get("name") != old.get("name")) {
                return Err("A campaign version keeps its name and _copy; nothing changed");
            }
            *old = new;
            if !editor.replaces {
                if let Some(k) = Key::of_entity(editor.kind, &Value::Object(old.clone())) {
                    editor.key = k;
                }
            }
            Ok("Saved the entry".to_string())
        });
    }
}
