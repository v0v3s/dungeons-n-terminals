//! The Notes section: a folder tree, an `edtui` vim editor and an info
//! column (frontmatter, backlinks, campaign overview).

use std::collections::BTreeSet;

pub use dnt_core::notes::Row;
use dnt_core::notes::{self as core_notes, Target};
use dnt_data::{EntityId, Library, SourceFilter};
use dnt_store::notes::{self, NoteInfo, Notes, Vars};
use edtui::{EditorEventHandler, EditorMode, EditorState, Lines};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::picker::{PickItem, Picked, Picker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Editor,
}

pub struct OpenNote {
    pub path: String,
    pub editor: EditorState,
    /// Text as last saved, to detect changes.
    saved: String,
}

impl OpenNote {
    pub fn text(&self) -> String {
        self.editor.lines.to_string()
    }

    pub fn dirty(&self) -> bool {
        self.text() != self.saved
    }
}

#[derive(Debug, Clone)]
pub enum PromptKind {
    NewNote { folder: String },
    NewFolder { parent: String },
    Rename { path: String },
    Delete { path: String },
}

pub enum Popup {
    Prompt(PromptKind, String),
    /// Searching notes: query text and selected result.
    Search(String, usize),
    /// The editor's `:` command line.
    Command(String),
    /// Create the missing note a link points to, in a chosen folder.
    CreateFromLink(Box<Picker>, String),
    /// Preview of a link target: a compendium entity, or a note's lines.
    Preview(String, Option<EntityId>, Vec<String>),
}

pub enum Action {
    None,
    Status(String),
    /// Show an entity in the compendium.
    Compendium(EntityId),
    /// The folder order changed; save it with the campaign.
    Order(Vec<String>),
}

pub struct NotesView {
    pub notes: Option<Notes>,
    pub expanded: BTreeSet<String>,
    pub cursor: usize,
    pub focus: Focus,
    pub open: Option<OpenNote>,
    pub popup: Option<Popup>,
    pub index: Vec<NoteInfo>,
    /// Previously open notes, for Ctrl-o.
    back: Vec<String>,
    /// Last cursor position per note, restored when reopened.
    cursors: std::collections::HashMap<String, edtui::Index2>,
    handler: EditorEventHandler,
    /// A `g` waiting to see whether `gd` follows.
    pending_g: bool,
    pub campaign: String,
}

impl NotesView {
    pub fn new(notes: Option<Notes>, campaign: String) -> NotesView {
        let expanded = notes.as_ref().map(|n| n.folders().into_iter().collect()).unwrap_or_default();
        let mut v = NotesView {
            notes,
            expanded,
            cursor: 0,
            focus: Focus::Tree,
            open: None,
            popup: None,
            index: Vec::new(),
            back: Vec::new(),
            cursors: Default::default(),
            handler: EditorEventHandler::default(),
            pending_g: false,
            campaign,
        };
        v.reindex();
        v
    }

    pub fn wants_text(&self) -> bool {
        self.focus == Focus::Editor || self.popup.is_some()
    }

    fn reindex(&mut self) {
        self.index = self.notes.as_ref().map(Notes::index).unwrap_or_default();
    }

    fn vars(&self, title: &str) -> Vars {
        core_notes::vars(&self.campaign, title)
    }

    /// The tree flattened into visible rows.
    pub fn rows(&self) -> Vec<Row> {
        self.notes.as_ref().map(|n| core_notes::tree(n, &self.expanded)).unwrap_or_default()
    }

    /// Folder for a new note: the folder under the cursor, or the folder of
    /// the note under the cursor.
    fn target_folder(&self) -> String {
        match self.rows().get(self.cursor) {
            Some(r) if r.entry.is_dir => r.entry.path.clone(),
            Some(r) => r.entry.path.rsplit_once('/').map_or(String::new(), |(p, _)| p.to_string()),
            None => String::new(),
        }
    }

    /// Save the open note if it changed.
    pub fn save(&mut self) -> Option<String> {
        let (Some(n), Some(open)) = (&self.notes, &mut self.open) else { return None };
        if !open.dirty() {
            return None;
        }
        let text = open.text();
        let text = if text.ends_with('\n') { text } else { format!("{text}\n") };
        match n.write(&open.path, &text) {
            Ok(()) => {
                open.saved = open.text();
                let msg = format!("Saved {}", open.path);
                self.reindex();
                Some(msg)
            }
            Err(e) => Some(format!("Could not save: {e}")),
        }
    }

    /// Re-read the open note after its file changed on disk (keeping the
    /// cursor and focus).
    pub fn reload_open(&mut self) {
        let Some((path, cursor)) = self.open.as_ref().map(|o| (o.path.clone(), o.editor.cursor)) else { return };
        let focus = self.focus;
        self.open = None;
        self.cursors.insert(path.clone(), cursor);
        self.open(&path, false);
        self.focus = focus;
        self.reindex();
    }

    /// Open a note in the editor (saving the current one first).
    pub fn open(&mut self, path: &str, remember: bool) -> Action {
        let status = self.save();
        let Some(n) = &self.notes else { return Action::None };
        match n.read(path) {
            Ok(text) => {
                if let Some(prev) = &self.open {
                    self.cursors.insert(prev.path.clone(), prev.editor.cursor);
                    if remember && prev.path != path {
                        self.back.push(prev.path.clone());
                    }
                }
                let text = text.strip_suffix('\n').unwrap_or(&text).to_string();
                let mut editor = EditorState::new(Lines::from(text.as_str()));
                if let Some(c) = self.cursors.get(path) {
                    let row = c.row.min(editor.lines.len().saturating_sub(1));
                    let col = c.col.min(editor.lines.len_col(row).unwrap_or(0));
                    editor.cursor = edtui::Index2::new(row, col);
                }
                self.open = Some(OpenNote { path: path.to_string(), editor, saved: text });
                // Reveal it in the tree.
                let mut acc = String::new();
                for part in path.split('/').collect::<Vec<_>>().split_last().map(|(_, dirs)| dirs.to_vec()).unwrap_or_default() {
                    acc = if acc.is_empty() { part.to_string() } else { format!("{acc}/{part}") };
                    self.expanded.insert(acc.clone());
                }
                if let Some(i) = self.rows().iter().position(|r| r.entry.path == path) {
                    self.cursor = i;
                }
                self.focus = Focus::Editor;
                status.map(Action::Status).unwrap_or(Action::None)
            }
            Err(e) => Action::Status(format!("Could not open: {e}")),
        }
    }

    /// Open (creating from the Character template if needed) a character's note.
    pub fn open_character(&mut self, name: &str) -> Action {
        let Some(n) = &self.notes else { return Action::Status("No active campaign".into()) };
        match core_notes::character_note(n, &self.campaign, name) {
            Ok(path) => {
                self.reindex();
                self.open(&path, true)
            }
            Err(e) => Action::Status(e),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, lib: &Library, filter: &SourceFilter) -> Action {
        if self.notes.is_none() {
            return Action::Status("Create or activate a campaign first (4)".into());
        }
        if self.popup.is_some() {
            return self.popup_key(key);
        }
        match self.focus {
            Focus::Tree => self.tree_key(key),
            Focus::Editor => self.editor_key(key, lib, filter),
        }
    }

    fn popup_key(&mut self, key: KeyEvent) -> Action {
        let Some(popup) = self.popup.take() else { return Action::None };
        match popup {
            Popup::Preview(..) => Action::None,
            Popup::Prompt(kind, mut text) => match key.code {
                KeyCode::Esc => Action::None,
                KeyCode::Enter => self.prompt_done(kind, text),
                KeyCode::Backspace => {
                    text.pop();
                    self.popup = Some(Popup::Prompt(kind, text));
                    Action::None
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    text.push(c);
                    self.popup = Some(Popup::Prompt(kind, text));
                    Action::None
                }
                _ => {
                    self.popup = Some(Popup::Prompt(kind, text));
                    Action::None
                }
            },
            Popup::Search(mut query, mut selected) => {
                let results = self.search(&query);
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => return Action::None,
                    KeyCode::Enter => {
                        return match results.get(selected) {
                            Some(path) => {
                                let path = path.clone();
                                self.open(&path, true)
                            }
                            None => Action::None,
                        };
                    }
                    KeyCode::Down | KeyCode::Tab => selected = (selected + 1).min(results.len().saturating_sub(1)),
                    KeyCode::Up | KeyCode::BackTab => selected = selected.saturating_sub(1),
                    KeyCode::Char('n' | 'j') if ctrl => selected = (selected + 1).min(results.len().saturating_sub(1)),
                    KeyCode::Char('p' | 'k') if ctrl => selected = selected.saturating_sub(1),
                    KeyCode::Char('u') if ctrl => {
                        query.clear();
                        selected = 0;
                    }
                    KeyCode::Backspace => {
                        query.pop();
                        selected = 0;
                    }
                    KeyCode::Char(c) if !ctrl => {
                        query.push(c);
                        selected = 0;
                    }
                    _ => {}
                }
                self.popup = Some(Popup::Search(query, selected));
                Action::None
            }
            Popup::Command(mut cmd) => match key.code {
                KeyCode::Esc => Action::None,
                KeyCode::Enter => self.command(cmd.trim()),
                KeyCode::Backspace => {
                    if cmd.pop().is_some() {
                        self.popup = Some(Popup::Command(cmd));
                    }
                    Action::None
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    cmd.push(c);
                    self.popup = Some(Popup::Command(cmd));
                    Action::None
                }
                _ => {
                    self.popup = Some(Popup::Command(cmd));
                    Action::None
                }
            },
            Popup::CreateFromLink(mut picker, title) => match picker.key(key) {
                Some(Picked::One(folder)) => {
                    let vars = self.vars(&title);
                    let created = self.notes.as_ref().map(|n| n.create(&folder, &vars));
                    match created {
                        Some(Ok(path)) => {
                            self.reindex();
                            self.open(&path, true)
                        }
                        Some(Err(e)) => Action::Status(format!("Could not create: {e}")),
                        None => Action::None,
                    }
                }
                _ if key.code == KeyCode::Esc => Action::None,
                _ => {
                    self.popup = Some(Popup::CreateFromLink(picker, title));
                    Action::None
                }
            },
        }
    }

    fn prompt_done(&mut self, kind: PromptKind, text: String) -> Action {
        let Some(n) = self.notes.clone() else { return Action::None };
        let text = text.trim().to_string();
        let result = match &kind {
            PromptKind::Delete { path } => {
                if text != "y" && !text.is_empty() {
                    return Action::None;
                }
                n.delete(path).map(|_| {
                    if self.open.as_ref().is_some_and(|o| o.path.starts_with(path.as_str())) {
                        self.open = None;
                    }
                    format!("Deleted {path}")
                })
            }
            _ if text.is_empty() => return Action::None,
            PromptKind::NewNote { folder } => n.create(folder, &self.vars(&text)).map(|p| {
                self.reindex();
                let _ = self.open(&p, true);
                format!("Created {p}")
            }),
            PromptKind::NewFolder { parent } => n.create_folder(parent, &text).map(|p| {
                self.expanded.insert(p.clone());
                format!("Created folder {p}")
            }),
            PromptKind::Rename { path } => n.rename(path, &text).map(|p| {
                if let Some(o) = self.open.as_mut().filter(|o| &o.path == path) {
                    o.path = p.clone();
                }
                format!("Renamed to {p}")
            }),
        };
        self.reindex();
        match result {
            Ok(msg) => Action::Status(msg),
            Err(e) => Action::Status(format!("Failed: {e}")),
        }
    }

    fn tree_key(&mut self, key: KeyEvent) -> Action {
        let rows = self.rows();
        let n = rows.len();
        let row = rows.get(self.cursor).cloned();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if n > 0 => self.cursor = (self.cursor + 1).min(n - 1),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') => self.cursor = 0,
            KeyCode::Char('G') => self.cursor = n.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => match row {
                Some(r) if r.entry.is_dir => {
                    if !self.expanded.remove(&r.entry.path) {
                        self.expanded.insert(r.entry.path);
                    }
                }
                Some(r) => return self.open(&r.entry.path, true),
                None => {}
            },
            KeyCode::Char('h') | KeyCode::Left => match row {
                Some(r) if r.entry.is_dir && self.expanded.contains(&r.entry.path) => {
                    self.expanded.remove(&r.entry.path);
                }
                Some(r) => {
                    // Jump to the parent folder.
                    if let Some((parent, _)) = r.entry.path.rsplit_once('/') {
                        if let Some(i) = rows.iter().position(|x| x.entry.path == parent) {
                            self.cursor = i;
                        }
                    }
                }
                None => {}
            },
            KeyCode::Char('a') => {
                self.popup = Some(Popup::Prompt(PromptKind::NewNote { folder: self.target_folder() }, String::new()));
            }
            KeyCode::Char('A') => {
                self.popup = Some(Popup::Prompt(PromptKind::NewFolder { parent: self.target_folder() }, String::new()));
            }
            KeyCode::Char('r') => {
                if let Some(r) = row {
                    self.popup = Some(Popup::Prompt(PromptKind::Rename { path: r.entry.path }, r.entry.name));
                }
            }
            KeyCode::Char('d') => {
                if let Some(r) = row {
                    self.popup = Some(Popup::Prompt(PromptKind::Delete { path: r.entry.path }, String::new()));
                }
            }
            KeyCode::Char('J' | 'K') => {
                // Reorder top-level folders.
                if let Some(r) = row.filter(|r| r.entry.is_dir && r.depth == 0) {
                    let delta = if key.code == KeyCode::Char('J') { 1 } else { -1 };
                    if let Some(n) = self.notes.as_mut() {
                        n.move_folder(&r.entry.name, delta);
                    }
                    if let Some(i) = self.rows().iter().position(|x| x.entry.path == r.entry.path) {
                        self.cursor = i;
                    }
                    return self.order().map(Action::Order).unwrap_or(Action::None);
                }
            }
            KeyCode::Char('s') => {
                let vars = self.vars("");
                let created = self.notes.as_ref().map(|n| n.new_session(&vars));
                if let Some(Ok(p)) = created {
                    self.reindex();
                    return self.open(&p, true);
                }
            }
            KeyCode::Char('i') => return self.open("Inbox.md", true),
            KeyCode::Char('c') => return self.open("Campaign.md", true),
            KeyCode::Char('/') => self.find(),
            KeyCode::Char('p') if ctrl => self.find(),
            KeyCode::Tab if self.open.is_some() => self.focus = Focus::Editor,
            _ => {}
        }
        Action::None
    }

    fn find(&mut self) {
        self.popup = Some(Popup::Search(String::new(), 0));
    }

    /// Paths of notes matching a search query (all notes when empty).
    pub fn search(&self, query: &str) -> Vec<String> {
        core_notes::search(&self.index, query)
    }

    /// Run a `:` command from the editor.
    fn command(&mut self, cmd: &str) -> Action {
        let (name, arg) = cmd.split_once(' ').map_or((cmd, ""), |(n, a)| (n, a.trim()));
        match name {
            "w" | "write" => self.save().map(Action::Status).unwrap_or(Action::Status("No changes".into())),
            "q" | "quit" | "close" | "wq" | "x" => {
                let status = self.save();
                if let Some(o) = self.open.take() {
                    self.cursors.insert(o.path, o.editor.cursor);
                }
                self.focus = Focus::Tree;
                status.map(Action::Status).unwrap_or(Action::None)
            }
            "e" | "edit" | "open" if !arg.is_empty() => match notes::resolve(&self.index, arg) {
                Some(n) => {
                    let path = n.path.clone();
                    self.open(&path, true)
                }
                None => Action::Status(format!("No note called {arg} (:new {arg} creates it)")),
            },
            "new" | "n" if !arg.is_empty() => {
                let folder = self.open.as_ref().and_then(|o| o.path.rsplit_once('/').map(|(f, _)| f.to_string())).unwrap_or_default();
                let vars = self.vars(arg);
                match self.notes.as_ref().map(|n| n.create(&folder, &vars)) {
                    Some(Ok(path)) => {
                        self.reindex();
                        self.open(&path, true)
                    }
                    Some(Err(e)) => Action::Status(format!("Could not create: {e}")),
                    None => Action::None,
                }
            }
            "session" => {
                let vars = self.vars("");
                match self.notes.as_ref().map(|n| n.new_session(&vars)) {
                    Some(Ok(path)) => {
                        self.reindex();
                        self.open(&path, true)
                    }
                    _ => Action::None,
                }
            }
            "inbox" => self.open("Inbox.md", true),
            "hub" | "campaign" => self.open("Campaign.md", true),
            "find" | "f" | "search" => {
                self.popup = Some(Popup::Search(arg.to_string(), 0));
                Action::None
            }
            "" => Action::None,
            other => Action::Status(format!("Unknown command :{other} (w, q, wq, e <note>, new <name>, session, inbox, hub, find)")),
        }
    }

    fn editor_key(&mut self, key: KeyEvent, lib: &Library, filter: &SourceFilter) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(open) = self.open.as_mut() else {
            self.focus = Focus::Tree;
            return Action::None;
        };
        let normal = open.editor.mode == EditorMode::Normal;
        if ctrl && key.code == KeyCode::Char('s') {
            return self.save().map(Action::Status).unwrap_or(Action::Status("No changes".into()));
        }
        if normal {
            if self.pending_g {
                self.pending_g = false;
                if key.code == KeyCode::Char('d') {
                    return self.follow(lib, filter);
                }
                // Not `gd`: give edtui the `g` it did not see.
                self.handler.on_key_event(KeyEvent::from(KeyCode::Char('g')), &mut open.editor);
            } else if key.code == KeyCode::Char('g') && key.modifiers.is_empty() {
                self.pending_g = true;
                return Action::None;
            }
            match key.code {
                KeyCode::Esc => {
                    let status = self.save();
                    self.focus = Focus::Tree;
                    return status.map(Action::Status).unwrap_or(Action::None);
                }
                KeyCode::Char('K') => return self.preview(lib, filter),
                KeyCode::Char(':') => {
                    self.popup = Some(Popup::Command(String::new()));
                    return Action::None;
                }
                KeyCode::Char('o') if ctrl => {
                    if let Some(prev) = self.back.pop() {
                        return self.open(&prev, false);
                    }
                    return Action::None;
                }
                _ => {}
            }
        }
        let Some(open) = self.open.as_mut() else { return Action::None };
        self.handler.on_key_event(key, &mut open.editor);
        Action::None
    }

    /// The `[[link]]` under the editor cursor.
    fn link_under_cursor(&self) -> Option<notes::WikiLink> {
        let open = self.open.as_ref()?;
        core_notes::link_at(&open.text(), open.editor.cursor.row, open.editor.cursor.col)
    }

    fn follow(&mut self, lib: &Library, filter: &SourceFilter) -> Action {
        let Some(link) = self.link_under_cursor() else { return Action::Status("No [[link]] under the cursor".into()) };
        match core_notes::resolve(lib, filter, &self.index, &link) {
            Target::Entity(e) => {
                let _ = self.save();
                Action::Compendium(e)
            }
            Target::NoEntity => Action::Status(format!("Not in the compendium: {}", link.target)),
            Target::Note(path) => self.open(&path, true),
            Target::NoNote(title) => {
                // Offer to create it.
                let mut items = vec![PickItem::new("(top level)", "", "")];
                if let Some(n) = &self.notes {
                    items.extend(n.folders().into_iter().map(|f| PickItem::new(f.clone(), "", f)));
                }
                let picker = Picker::new(format!("Create “{title}” in which folder?"), items);
                self.popup = Some(Popup::CreateFromLink(Box::new(picker), title));
                Action::None
            }
        }
    }

    fn preview(&mut self, lib: &Library, filter: &SourceFilter) -> Action {
        let Some(link) = self.link_under_cursor() else { return Action::Status("No [[link]] under the cursor".into()) };
        let (entity, lines) = match core_notes::resolve(lib, filter, &self.index, &link) {
            Target::Entity(e) => (Some(e), Vec::new()),
            Target::NoEntity => (None, vec!["Not in the compendium.".into()]),
            Target::Note(path) => match self.notes.as_ref().and_then(|n| n.read(&path).ok()) {
                Some(text) => (None, text.lines().map(str::to_string).collect()),
                None => (None, vec!["Could not read the note.".into()]),
            },
            Target::NoNote(_) => (None, vec!["No such note yet — gd creates it.".into()]),
        };
        self.popup = Some(Popup::Preview(link.target, entity, lines));
        Action::None
    }

    /// Quick capture: append a timestamped line to the inbox.
    pub fn capture(&mut self, text: &str) -> String {
        // Keep an open inbox in sync: save it, append, reload it.
        let inbox_open = self.open.as_ref().is_some_and(|o| o.path == "Inbox.md");
        let _ = self.save();
        let Some(n) = &self.notes else { return "No active campaign".into() };
        let msg = core_notes::capture(n, text);
        if msg.is_ok() && inbox_open {
            let focus = self.focus;
            let _ = self.open("Inbox.md", false);
            self.focus = focus;
        }
        self.reindex();
        msg.unwrap_or_else(|e| e)
    }

    /// Change the folder order and return it, for saving in the campaign.
    pub fn order(&self) -> Option<Vec<String>> {
        self.notes.as_ref().map(|n| n.order.clone())
    }
}
