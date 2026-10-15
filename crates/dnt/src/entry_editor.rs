//! Editing a campaign homebrew entity field by field. Changes are returned
//! as actions; the app applies them to the homebrew file and reloads.

use dnt_data::{EntityId, Key, Kind, Library};
use edtui::{EditorEventHandler, EditorMode, EditorState, Lines};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

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
}

pub struct Field {
    pub name: String,
    pub value: Option<Value>,
    pub widget: Widget,
    /// Stored in the homebrew entry (changed from the original, or new).
    pub changed: bool,
}

pub enum Mode {
    List,
    /// One-line prompt for a text or number field.
    Prompt { field: String, text: String },
    /// The big editor for entries (Markdown) or JSON.
    Big { field: String, editor: Box<EditorState>, json: bool },
    ConfirmDelete,
    /// Name of a new field to add.
    AddField(String),
}

pub enum Action {
    None,
    Close,
    Status(String),
    /// Set (`Some`) or revert (`None`) a field of the entry.
    Set(String, Option<Value>),
    Delete,
}

pub struct EntryEditor {
    /// The entity being edited (its original key for overrides).
    pub key: Key,
    pub kind: Kind,
    /// Whether the entry replaces a book entity.
    pub replaces: bool,
    pub cursor: usize,
    pub mode: Mode,
    handler: EditorEventHandler,
    pub error: Option<String>,
}

/// Preferred field order per kind; the rest follow alphabetically.
fn order(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Spell => &["name", "level", "school", "time", "range", "components", "duration", "meta", "entries", "entriesHigherLevel"],
        Kind::Item | Kind::BaseItem | Kind::MagicVariant => &[
            "name", "type", "rarity", "reqAttune", "wondrous", "weight", "value", "ac", "bonusAc", "dmg1", "dmg2", "dmgType",
            "bonusWeapon", "property", "entries",
        ],
        Kind::Feat => &["name", "category", "prerequisite", "ability", "entries"],
        _ => &["name", "entries"],
    }
}

fn widget_for(field: &str, value: Option<&Value>) -> Widget {
    match (field, value) {
        ("entries" | "entriesHigherLevel" | "entriesTemplate", _) => Widget::Entries,
        (_, Some(Value::Bool(_))) | ("wondrous" | "srd52" | "basicRules2024", None) => Widget::Bool,
        (_, Some(Value::Number(_))) | ("level" | "weight" | "value" | "ac", None) => Widget::Number,
        (_, Some(Value::String(_))) | (_, None) => Widget::Text,
        _ => Widget::Json,
    }
}

impl EntryEditor {
    pub fn new(key: Key, kind: Kind, replaces: bool) -> EntryEditor {
        EntryEditor {
            key,
            kind,
            replaces,
            cursor: 0,
            mode: Mode::List,
            handler: EditorEventHandler::default(),
            error: None,
        }
    }

    pub fn wants_text(&self) -> bool {
        !matches!(self.mode, Mode::List)
    }

    pub fn entity(&self, lib: &Library) -> Option<EntityId> {
        lib.get(&self.key)
    }

    /// Fields to show, given the merged entity and the homebrew entry.
    pub fn fields(&self, lib: &Library, entry: Option<&serde_json::Map<String, Value>>) -> Vec<Field> {
        let Some(id) = self.entity(lib) else { return Vec::new() };
        let data = &lib.entity(id).data;
        let mut names: Vec<String> = order(self.kind).iter().map(|s| s.to_string()).collect();
        let mut rest: Vec<String> = data
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k.clone())
            .filter(|k| !k.starts_with('_') && !names.contains(k))
            .filter(|k| !matches!(k.as_str(), "source" | "page" | "srd" | "srd52" | "basicRules" | "basicRules2024" | "hasFluff" | "hasFluffImages"))
            .collect();
        rest.sort();
        names.extend(rest);
        names
            .into_iter()
            .map(|name| {
                let value = data.get(&name).cloned();
                Field {
                    widget: widget_for(&name, value.as_ref()),
                    changed: entry.is_some_and(|e| e.contains_key(&name)) && !(name == "name" && self.replaces),
                    name,
                    value,
                }
            })
            .collect()
    }

    pub fn key(&mut self, key: KeyEvent, fields: &[Field]) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match &mut self.mode {
            Mode::ConfirmDelete => {
                self.mode = Mode::List;
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    return Action::Delete;
                }
                Action::None
            }
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
                    self.mode = Mode::Big {
                        editor: Box::new(EditorState::new(Lines::from("\"\""))),
                        field: name,
                        json: true,
                    };
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
                    Action::None
                }
                KeyCode::Enter => {
                    let field = field.clone();
                    let text = text.trim().to_string();
                    let number = fields.iter().find(|f| f.name == field).is_some_and(|f| f.widget == Widget::Number);
                    let value = if number {
                        match text.parse::<i64>() {
                            Ok(n) => Value::from(n),
                            Err(_) => match text.parse::<f64>() {
                                Ok(f) => Value::from(f),
                                Err(_) => {
                                    self.error = Some(format!("{field} needs a number"));
                                    return Action::None;
                                }
                            },
                        }
                    } else {
                        Value::String(text)
                    };
                    self.mode = Mode::List;
                    self.error = None;
                    Action::Set(field, Some(value))
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
                let parsed = if *json {
                    serde_json::from_str::<Value>(&text).map_err(|e| format!("Not valid JSON: {e}"))
                } else {
                    dnt_data::homebrew::markdown_to_entries(&text)
                };
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
                            let text = serde_json::to_string_pretty(f.value.as_ref().unwrap_or(&Value::Null)).unwrap_or_default();
                            self.mode = Mode::Big {
                                field: f.name.clone(),
                                editor: Box::new(EditorState::new(Lines::from(text.as_str()))),
                                json: true,
                            };
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                        let Some(f) = field else { return Action::None };
                        if f.name == "name" && self.replaces {
                            return Action::Status("A replacing version keeps the original's name; clone as a separate copy to rename".into());
                        }
                        match f.widget {
                            Widget::Bool => {
                                let on = f.value.as_ref().and_then(Value::as_bool).unwrap_or(false);
                                return Action::Set(f.name.clone(), Some(Value::Bool(!on)));
                            }
                            Widget::Text | Widget::Number => {
                                let text = match &f.value {
                                    Some(Value::String(s)) => s.clone(),
                                    Some(v) => v.to_string(),
                                    None => String::new(),
                                };
                                self.mode = Mode::Prompt { field: f.name.clone(), text };
                            }
                            Widget::Entries => {
                                let md = f.value.as_ref().map(dnt_data::homebrew::entries_to_markdown).unwrap_or_default();
                                self.mode = Mode::Big {
                                    field: f.name.clone(),
                                    editor: Box::new(EditorState::new(Lines::from(md.as_str()))),
                                    json: false,
                                };
                            }
                            Widget::Json => {
                                let text = serde_json::to_string_pretty(f.value.as_ref().unwrap_or(&Value::Null)).unwrap_or_default();
                                self.mode = Mode::Big {
                                    field: f.name.clone(),
                                    editor: Box::new(EditorState::new(Lines::from(text.as_str()))),
                                    json: true,
                                };
                            }
                        }
                    }
                    _ => {}
                }
                Action::None
            }
        }
    }
}
