//! Editing a homebrew entity field by field. Changes are returned as
//! actions; the app applies them to the homebrew file and reloads.

use dnt_core::entry_fields::{json_text, parse_json};
pub use dnt_core::entry_fields::{Field, Widget};
use dnt_core::homebrew::EntryRef;
use dnt_data::forms::Format;
use dnt_data::{EntityId, Library};
use edtui::{EditorEventHandler, EditorMode, EditorState, Lines};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

pub enum Mode {
    List,
    /// One-line prompt for a text, number or friendly field.
    Prompt {
        field: String,
        text: String,
    },
    /// Pick one of a field's choices.
    Choose {
        field: String,
        options: Vec<(String, Option<Value>)>,
        cursor: usize,
    },
    /// The big editor for entries (Markdown) or JSON.
    Big {
        field: String,
        editor: Box<EditorState>,
        json: bool,
    },
    ConfirmDelete,
    /// Name of a new field to add.
    AddField(String),
}

pub enum Action {
    None,
    Close,
    Status(String),
    /// Set (`Some`) or revert/remove (`None`) a field of the entry.
    Set(String, Option<Value>),
    Delete,
    /// Edit the whole entry as JSON in `$EDITOR`.
    External,
    /// Move the entry to the global homebrew library.
    Promote,
}

pub struct EntryEditor {
    pub entry: EntryRef,
    pub cursor: usize,
    pub mode: Mode,
    handler: EditorEventHandler,
    pub error: Option<String>,
}

fn text_editor(text: &str) -> Box<EditorState> {
    Box::new(EditorState::new(Lines::from(text)))
}

impl EntryEditor {
    pub fn new(entry: EntryRef) -> EntryEditor {
        EntryEditor { entry, cursor: 0, mode: Mode::List, handler: EditorEventHandler::default(), error: None }
    }

    pub fn wants_text(&self) -> bool {
        !matches!(self.mode, Mode::List)
    }

    pub fn entity(&self, lib: &Library) -> Option<EntityId> {
        self.entry.entity(lib)
    }

    pub fn key(&mut self, key: KeyEvent, fields: &[Field], lib: &Library) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match &mut self.mode {
            Mode::ConfirmDelete => {
                self.mode = Mode::List;
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    return Action::Delete;
                }
                Action::None
            }
            Mode::Choose { field, options, cursor } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = Mode::List;
                    Action::None
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    *cursor = (*cursor + 1).min(options.len().saturating_sub(1));
                    Action::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    *cursor = cursor.saturating_sub(1);
                    Action::None
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let (field, value) = (field.clone(), options.get(*cursor).and_then(|(_, v)| v.clone()));
                    self.mode = Mode::List;
                    Action::Set(field, value)
                }
                _ => Action::None,
            },
            Mode::AddField(name) => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::List;
                    Action::None
                }
                KeyCode::Enter => {
                    let name = name.trim().to_string();
                    if name.is_empty() || name.starts_with('_') {
                        self.mode = Mode::List;
                        return Action::None;
                    }
                    self.mode = Mode::Big { editor: text_editor("\"\""), field: name, json: true };
                    Action::None
                }
                KeyCode::Backspace => {
                    name.pop();
                    Action::None
                }
                KeyCode::Char(c) if !ctrl => {
                    name.push(c);
                    Action::None
                }
                _ => Action::None,
            },
            Mode::Prompt { field, text } => match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::List;
                    self.error = None;
                    Action::None
                }
                KeyCode::Enter => {
                    let field = field.clone();
                    let text = text.trim().to_string();
                    let value = match fields.iter().find(|f| f.name == field).map(|f| f.parse(&text, lib)) {
                        Some(Ok(v)) => v,
                        Some(Err(e)) => {
                            self.error = Some(e);
                            return Action::None;
                        }
                        None => Some(Value::String(text)),
                    };
                    self.mode = Mode::List;
                    self.error = None;
                    Action::Set(field, value)
                }
                KeyCode::Backspace => {
                    text.pop();
                    Action::None
                }
                KeyCode::Char('u') if ctrl => {
                    text.clear();
                    Action::None
                }
                KeyCode::Char(c) if !ctrl => {
                    text.push(c);
                    Action::None
                }
                _ => Action::None,
            },
            Mode::Big { field, editor, json } => {
                let save = (ctrl && key.code == KeyCode::Char('s')) || (key.code == KeyCode::Esc && editor.mode == EditorMode::Normal);
                if !save {
                    self.handler.on_key_event(key, editor);
                    return Action::None;
                }
                let text = editor.lines.to_string();
                let parsed = if *json { parse_json(&text) } else { dnt_data::homebrew::markdown_to_entries(&text) };
                match parsed {
                    Ok(v) => {
                        let field = field.clone();
                        self.mode = Mode::List;
                        self.error = None;
                        Action::Set(field, Some(v))
                    }
                    Err(e) => {
                        self.error = Some(e);
                        Action::None
                    }
                }
            }
            Mode::List => {
                let n = fields.len();
                let field = fields.get(self.cursor);
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
                    KeyCode::Char('j') | KeyCode::Down if n > 0 => self.cursor = (self.cursor + 1).min(n - 1),
                    KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
                    KeyCode::Char('g') => self.cursor = 0,
                    KeyCode::Char('G') => self.cursor = n.saturating_sub(1),
                    KeyCode::Char('D') => self.mode = Mode::ConfirmDelete,
                    KeyCode::Char('a') => self.mode = Mode::AddField(String::new()),
                    KeyCode::Char('E') => return Action::External,
                    KeyCode::Char('P') => return Action::Promote,
                    KeyCode::Char('x') => {
                        if let Some(f) = field {
                            if f.changed {
                                return Action::Set(f.name.clone(), None);
                            }
                            return Action::Status(format!("{} is not changed", f.name));
                        }
                    }
                    KeyCode::Char('J') => {
                        // Edit any field as JSON.
                        if let Some(f) = field {
                            self.mode = Mode::Big { field: f.name.clone(), editor: text_editor(&json_text(f.value.as_ref())), json: true };
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                        let Some(f) = field else { return Action::None };
                        if f.name == "name" && self.entry.replaces {
                            return Action::Status(
                                "A replacing version keeps the original's name; clone as a separate copy to rename".into(),
                            );
                        }
                        self.error = None;
                        let (field, text) = (f.name.clone(), f.text());
                        self.mode = match (f.widget, f.choices()) {
                            (Widget::Bool, _) => return Action::Set(field, Some(f.toggled())),
                            (_, Some(options)) => {
                                let cursor = options.iter().position(|(_, v)| v.as_ref() == f.value.as_ref()).unwrap_or(0);
                                Mode::Choose { field, options, cursor }
                            }
                            (Widget::Entries, _) => Mode::Big { field, editor: text_editor(&text), json: false },
                            (Widget::Json, _) => Mode::Big { field, editor: text_editor(&text), json: true },
                            _ => Mode::Prompt { field, text },
                        };
                    }
                    _ => {}
                }
                Action::None
            }
        }
    }

    /// The format of the field being prompted for, for its hint.
    pub fn prompt_format(&self, fields: &[Field]) -> Option<Format> {
        let Mode::Prompt { field, .. } = &self.mode else { return None };
        fields.iter().find(|f| &f.name == field)?.format()
    }
}
