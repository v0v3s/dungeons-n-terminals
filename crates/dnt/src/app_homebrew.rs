//! Homebrew flows of the app: cloning, creating, editing and promoting
//! entries of the campaign and global homebrew documents.

use std::path::Path;

use dnt_core::homebrew::{self, Change, Doc};
use dnt_data::homebrew::{CloneMode, Template};
use dnt_data::{EntityId, Library};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::app::{App, Section};
use crate::entry_editor::{self, EntryEditor};

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
    pub fn target_doc(&self, target: Doc) -> Option<&Value> {
        match target {
            Doc::Campaign => self.homebrew.as_ref(),
            Doc::Global => self.global_homebrew.as_ref(),
        }
    }

    fn load_doc(&self, target: Doc) -> Result<Value, String> {
        match target {
            Doc::Campaign => {
                let campaign = self.active_campaign().ok_or("No active campaign")?;
                self.store.homebrew(campaign).map_err(|e| format!("Could not read homebrew: {e}"))
            }
            Doc::Global => self.store.global_homebrew().map_err(|e| format!("Could not read global homebrew: {e}")),
        }
    }

    fn save_doc(&mut self, target: Doc, doc: Value) -> Result<(), String> {
        match target {
            Doc::Campaign => {
                let campaign = self.active_campaign().ok_or("No active campaign")?;
                self.store.save_homebrew(campaign, &doc).map_err(|e| format!("Could not save homebrew: {e}"))?;
                self.homebrew = Some(doc);
                self.pending_reload = true;
            }
            Doc::Global => {
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
        let source = self.active_campaign().map(|c| c.settings.source.clone());
        match homebrew::edit(lib, id, source.as_deref()) {
            Ok(entry) => self.open_editor(EntryEditor::new(entry)),
            Err(e) => self.status = e,
        }
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
        let Some(source) = self.active_campaign().map(|c| c.settings.source.clone()) else { return };
        let done = self.load_doc(Doc::Campaign).and_then(|mut doc| {
            let (entry, msg) = homebrew::clone(lib, &mut doc, id, &source, mode)?;
            self.save_doc(Doc::Campaign, doc)?;
            Ok((entry, msg))
        });
        match done {
            Ok((entry, msg)) => {
                self.status = msg;
                self.open_editor(EntryEditor::new(entry));
            }
            Err(e) => self.status = e,
        }
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
        let Some(source) = self.active_campaign().map(|c| c.settings.source.clone()) else { return };
        let done = self.load_doc(Doc::Campaign).and_then(|mut doc| {
            let (entry, msg) = homebrew::create(lib, &mut doc, template, name, &source)?;
            self.save_doc(Doc::Campaign, doc)?;
            Ok((entry, msg))
        });
        let entry = match done {
            Ok((entry, msg)) => {
                self.status = msg;
                entry
            }
            Err(e) => return self.status = e,
        };
        if for_inventory {
            if let Some(sheet) = self.characters.sheet.as_mut() {
                let id = format!("item:{name}|{source}");
                dnt_rules::equipment::add_item(&mut sheet.session.file.character, &id, 1, None);
                sheet.inventory_cursor = sheet.session.file.character.inventory.len().saturating_sub(1);
                self.status = match self.store.save_character(&sheet.session.file) {
                    Ok(()) => format!("Created {name} and added it to {}", sheet.session.file.character.name),
                    Err(e) => format!("Created {name}, but could not save the character: {e}"),
                };
            }
        }
        self.open_editor(EntryEditor::new(entry));
    }

    pub(crate) fn editor_key(&mut self, lib: &Library, key: KeyEvent) {
        let Some(editor) = self.editor.as_ref() else { return };
        let entry = self.target_doc(editor.entry.doc).and_then(|doc| editor.entry.entry(lib, doc)).cloned();
        let editor = self.editor.as_mut().unwrap();
        let fields = dnt_core::entry_fields::fields(lib, &editor.entry, entry.as_ref());
        match editor.key(key, &fields, lib) {
            entry_editor::Action::None => {}
            entry_editor::Action::Close => self.close_editor(),
            entry_editor::Action::Status(s) => self.status = s,
            entry_editor::Action::Set(field, value) => self.edit_entry(lib, Change::Set { field, value }),
            entry_editor::Action::Delete => {
                self.edit_entry(lib, Change::Delete);
                self.close_editor();
            }
            entry_editor::Action::External => self.external_edit(lib),
            entry_editor::Action::Promote => self.promote(lib),
        }
    }

    /// Change the edited entry in its homebrew document and save.
    fn edit_entry(&mut self, lib: &Library, change: Change) {
        let Some(mut entry) = self.editor.as_ref().map(|e| e.entry.clone()) else { return };
        let done = self.load_doc(entry.doc).and_then(|mut doc| {
            let msg = homebrew::change(lib, &mut doc, &mut entry, change)?;
            self.save_doc(entry.doc, doc)?;
            Ok(msg)
        });
        self.status = done.unwrap_or_else(|e| e);
        if let Some(editor) = self.editor.as_mut() {
            editor.entry = entry;
        }
    }

    /// Move a campaign entry to the global homebrew and point the campaign's
    /// characters at it.
    fn promote(&mut self, lib: &Library) {
        let Some(mut entry) = self.editor.as_ref().map(|e| e.entry.clone()) else { return };
        let (mut cdoc, mut gdoc) = match (self.load_doc(Doc::Campaign), self.load_doc(Doc::Global)) {
            (Ok(c), Ok(g)) => (c, g),
            (Err(e), _) | (_, Err(e)) => return self.status = e,
        };
        let p = match homebrew::promote(lib, &mut cdoc, &mut gdoc, &mut entry) {
            Ok(p) => p,
            Err(e) => return self.status = e,
        };
        // Global first: if saving the campaign fails, the entry exists twice
        // rather than not at all.
        if let Err(err) = self.save_doc(Doc::Global, gdoc).and_then(|_| self.save_doc(Doc::Campaign, cdoc)) {
            return self.status = err;
        }
        // Note links that name the old source: `[[item:X@CoS]]`.
        self.notes.save();
        if let Some(campaign) = self.active_campaign() {
            self.status = homebrew::rewrite_references(&self.store, campaign, self.notes.notes.as_ref(), entry.kind, &p);
        }
        self.notes.reload_open();
        // The characters on disk changed; the open sheet must not save stale refs.
        self.characters.files = self.active_campaign().map(|c| self.store.characters(c).0).unwrap_or_default();
        if let Some(path) = self.characters.sheet.as_ref().map(|s| s.session.file.path.clone()) {
            if let Ok(file) = self.store.load_character(&path) {
                self.characters.reload(file, lib, &self.overlays);
            }
        }
        if let Some(editor) = self.editor.as_mut() {
            editor.entry = entry;
        }
    }

    /// Write the entry to a temporary file for $EDITOR.
    fn external_edit(&mut self, lib: &Library) {
        let Some(editor) = self.editor.as_ref() else { return };
        let Some(entry) = self.target_doc(editor.entry.doc).and_then(|doc| editor.entry.entry(lib, doc)).cloned() else { return };
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
        self.edit_entry(lib, Change::Replace(new));
    }
}
