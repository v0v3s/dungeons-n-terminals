//! Editing a homebrew entity field by field. Changes are returned as
//! actions; the app applies them to the homebrew file and reloads.

use dnt_data::forms::{self, Format};
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
    /// Friendly text or a choice list (see `dnt_data::forms`).
    Form(Format),
}

pub struct Field {
    pub name: String,
    pub value: Option<Value>,
    /// The value as shown in the list.
    pub display: String,
    pub widget: Widget,
    /// Stored in the homebrew entry (changed from the original, or new).
    pub changed: bool,
    /// Not one of the kind's common fields.
    pub extra: bool,
}

pub enum Mode {
    List,
    /// One-line prompt for a text, number or friendly field.
    Prompt { field: String, text: String },
    /// Pick one of a field's choices.
    Choose { field: String, options: Vec<(String, Option<Value>)>, cursor: usize },
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
    /// Set (`Some`) or revert/remove (`None`) a field of the entry.
    Set(String, Option<Value>),
    Delete,
    /// Edit the whole entry as JSON in `$EDITOR`.
    External,
    /// Move the entry to the global homebrew library.
    Promote,
}

/// Which homebrew document the entry lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Campaign,
    Global,
}

pub struct EntryEditor {
    /// The entity being edited (its original key for overrides).
    pub key: Key,
    pub kind: Kind,
    /// Whether the entry replaces a book entity.
    pub replaces: bool,
    pub target: Target,
    pub cursor: usize,
    pub mode: Mode,
    handler: EditorEventHandler,
    pub error: Option<String>,
}

/// Common fields per kind, in order; the rest follow alphabetically.
fn order(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Spell => &[
            "name", "level", "school", "time", "range", "components", "duration", "meta", "classes", "entries", "entriesHigherLevel",
        ],
        Kind::Item | Kind::BaseItem | Kind::MagicVariant => &[
            "name", "type", "rarity", "reqAttune", "wondrous", "weight", "value", "ac", "bonusAc", "weaponCategory", "dmg1", "dmg2",
            "dmgType", "bonusWeapon", "property", "entries",
        ],
        Kind::Feat => &["name", "category", "prerequisite", "ability", "entries"],
        _ => &["name", "entries"],
    }
}

fn widget_for(kind: Kind, field: &str, value: Option<&Value>) -> Widget {
    if let Some(f) = forms::format_for(kind, field) {
        return Widget::Form(f);
    }
    match (field, value) {
        ("entries" | "entriesHigherLevel" | "entriesTemplate", _) => Widget::Entries,
        (_, Some(Value::Bool(_))) | ("wondrous", None) => Widget::Bool,
        (_, Some(Value::Number(_))) | ("weight" | "ac", None) => Widget::Number,
        (_, Some(Value::String(_))) | (_, None) => Widget::Text,
        _ => Widget::Json,
    }
}

/// A short one-line rendering of a JSON value.
fn preview(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Array(a) if a.iter().all(|x| x.is_string()) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" · "),
        other => other.to_string(),
    }
}

fn text_editor(text: &str) -> Box<EditorState> {
    Box::new(EditorState::new(Lines::from(text)))
}

impl EntryEditor {
    pub fn new(key: Key, kind: Kind, replaces: bool, target: Target) -> EntryEditor {
        EntryEditor {
            key,
            kind,
            replaces,
            target,
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
        let common: Vec<String> = order(self.kind).iter().map(|s| s.to_string()).collect();
        let mut rest: Vec<String> = data
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k.clone())
            .filter(|k| !k.starts_with('_') && !common.contains(k))
            .filter(|k| !matches!(k.as_str(), "source" | "page" | "srd" | "srd52" | "basicRules" | "basicRules2024" | "hasFluff" | "hasFluffImages"))
            .collect();
        rest.sort();
        let n_common = common.len();
        common
            .into_iter()
            .chain(rest)
            .enumerate()
            .map(|(i, name)| {
                let mut value = data.get(&name).cloned();
                if name == "classes" && value.is_none() && self.kind == Kind::Spell {
                    // Official spells get their classes from a lookup.
                    let list: Vec<Value> =
                        lib.spell_classes(id).iter().map(|c| serde_json::json!({"name": c.name, "source": c.source})).collect();
                    if !list.is_empty() {
                        value = Some(serde_json::json!({"fromClassList": list}));
                    }
                }
                let widget = widget_for(self.kind, &name, value.as_ref());
                let display = match (&value, widget) {
                    (_, Widget::Form(f)) => forms::display(f, value.as_ref()),
                    (None, _) => "—".to_string(),
                    (Some(v), Widget::Entries) => dnt_data::homebrew::entries_to_markdown(v).split_whitespace().collect::<Vec<_>>().join(" "),
                    (Some(v), _) => preview(v),
                };
                Field {
                    widget,
                    display,
                    changed: entry.is_some_and(|e| e.contains_key(&name)) && !(name == "name" && self.replaces),
                    extra: i >= n_common,
                    name,
                    value,
                }
            })
            .collect()
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
                    let f = fields.iter().find(|f| f.name == field);
                    let value = match f.map(|f| f.widget) {
                        Some(Widget::Form(format)) => match forms::parse(format, &text, f.and_then(|f| f.value.as_ref()), lib) {
                            Ok(v) => v,
                            Err(e) => {
                                self.error = Some(e);
                                return Action::None;
                            }
                        },
                        Some(Widget::Number) => match text.parse::<i64>() {
                            Ok(n) => Some(Value::from(n)),
                            Err(_) => match text.parse::<f64>() {
                                Ok(n) => Some(Value::from(n)),
                                Err(_) => {
                                    self.error = Some(format!("{field} needs a number"));
                                    return Action::None;
                                }
                            },
                        },
                        _ => Some(Value::String(text)),
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
                    KeyCode::Char('E') => return Action::External,
                    KeyCode::Char('P') => {
                        return match (self.target, self.replaces) {
                            (Target::Global, _) => Action::Status("Already in the global homebrew".into()),
                            (_, true) => Action::Status("A campaign version stays in its campaign; clone a separate copy to share it".into()),
                            _ => Action::Promote,
                        };
                    }
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
                            self.mode = Mode::Big { field: f.name.clone(), editor: text_editor(&text), json: true };
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                        let Some(f) = field else { return Action::None };
                        if f.name == "name" && self.replaces {
                            return Action::Status("A replacing version keeps the original's name; clone as a separate copy to rename".into());
                        }
                        self.error = None;
                        match f.widget {
                            Widget::Bool => {
                                let on = f.value.as_ref().and_then(Value::as_bool).unwrap_or(false);
                                return Action::Set(f.name.clone(), Some(Value::Bool(!on)));
                            }
                            Widget::Form(format) => match forms::choices(format) {
                                Some(options) => {
                                    let cursor = options.iter().position(|(_, v)| v.as_ref() == f.value.as_ref()).unwrap_or(0);
                                    self.mode = Mode::Choose { field: f.name.clone(), options, cursor };
                                }
                                None => {
                                    let text = if f.value.is_some() { f.display.clone() } else { String::new() };
                                    self.mode = Mode::Prompt { field: f.name.clone(), text };
                                }
                            },
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
                                self.mode = Mode::Big { field: f.name.clone(), editor: text_editor(&md), json: false };
                            }
                            Widget::Json => {
                                let text = serde_json::to_string_pretty(f.value.as_ref().unwrap_or(&Value::Null)).unwrap_or_default();
                                self.mode = Mode::Big { field: f.name.clone(), editor: text_editor(&text), json: true };
                            }
                        }
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
        match fields.iter().find(|f| &f.name == field)?.widget {
            Widget::Form(f) => Some(f),
            _ => None,
        }
    }
}
