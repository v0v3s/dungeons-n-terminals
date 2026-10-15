//! Writing a feature's rules (an overlay) from the sheet: TOML in the
//! built-in editor, checked as you type against the open character.

use dnt_data::{EntityId, Kind, Library};
use dnt_rules::builder::build;
use dnt_rules::character::Character;
use dnt_rules::overlay::{Overlay, OverlaySet};
use dnt_rules::{builtin, Evaluation};
use edtui::{EditorEventHandler, EditorMode, EditorState, Lines};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where the starting text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The user's own overlay file.
    User,
    /// dnt's built-in overlay (saving makes a user version that replaces it).
    Builtin,
    /// A template: nothing automates this feature yet.
    New,
}

#[derive(Clone)]
pub struct OverlayEditor {
    /// The overlay id, e.g. `classFeature:Brutal Strike|Barbarian|XPHB|9`.
    pub id: String,
    pub name: String,
    pub entity: EntityId,
    pub source: Source,
    pub editor: EditorState,
    handler: EditorEventHandler,
    /// Text when opened, to tell whether there is anything to save.
    original: String,
    /// Problems with the draft (empty: it is valid).
    pub errors: Vec<String>,
    /// What the draft changes on the open character.
    pub changes: Vec<String>,
}

impl std::fmt::Debug for OverlayEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayEditor").field("id", &self.id).field("source", &self.source).finish()
    }
}

pub enum Outcome {
    Continue,
    Close,
    /// Save this text as the feature's overlay (empty: remove it).
    Save(String),
}

/// The overlay id for an entity: `<type>:<reference>` in the data's pipe
/// syntax.
pub fn overlay_id(lib: &Library, entity: EntityId) -> String {
    let e = lib.entity(entity);
    let s = |k: &str| e.data.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let level = e.data.get("level").and_then(|v| v.as_i64()).unwrap_or(0);
    let reference = match e.kind {
        Kind::ClassFeature => format!("{}|{}|{}|{level}|{}", e.name, s("className"), s("classSource"), e.source),
        Kind::SubclassFeature => format!(
            "{}|{}|{}|{}|{}|{level}|{}",
            e.name,
            s("className"),
            s("classSource"),
            s("subclassShortName"),
            s("subclassSource"),
            e.source
        ),
        _ => format!("{}|{}", e.name, e.source),
    };
    format!("{}:{reference}", e.kind.json_key())
}

impl OverlayEditor {
    /// Open the editor for a feature, starting from the user's overlay
    /// (`user`), dnt's built-in one, or a template.
    pub fn open(lib: &Library, entity: EntityId, user: Option<String>) -> OverlayEditor {
        let id = overlay_id(lib, entity);
        let name = lib.entity(entity).name.clone();
        let (source, text) = match user {
            Some(t) => (Source::User, t),
            None => match builtin::source_for(&lib.entity(entity).key) {
                Some(t) => (Source::Builtin, t),
                None => (Source::New, dnt_rules::overlay::template(&id, &name)),
            },
        };
        let text = text.trim_end().to_string();
        OverlayEditor {
            id,
            name,
            entity,
            source,
            editor: EditorState::new(Lines::from(text.as_str())),
            handler: EditorEventHandler::default(),
            original: text,
            errors: Vec::new(),
            changes: Vec::new(),
        }
    }

    pub fn text(&self) -> String {
        self.editor.lines.to_string()
    }

    /// Esc (in normal mode) saves when the text changed and is valid;
    /// ctrl-q closes without saving.
    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('q') {
            return Outcome::Close;
        }
        let done = (ctrl && key.code == KeyCode::Char('s')) || (key.code == KeyCode::Esc && self.editor.mode == EditorMode::Normal);
        if !done {
            self.handler.on_key_event(key, &mut self.editor);
            return Outcome::Continue;
        }
        let text = self.text();
        // Unchanged (including an untouched built-in or template): nothing to save.
        if text.trim_end() == self.original {
            return Outcome::Close;
        }
        if !self.errors.is_empty() {
            return Outcome::Continue;
        }
        Outcome::Save(if text.trim().is_empty() { String::new() } else { text.trim_end().to_string() + "\n" })
    }

    /// Check the draft: parse it, make sure it names this feature, and
    /// compare the character with and without it.
    pub fn check(&mut self, lib: &Library, character: &Character, overlays: &OverlaySet, current: &Evaluation) {
        self.errors.clear();
        self.changes.clear();
        let text = self.text();
        let mut set = overlays.clone();
        let key = lib.entity(self.entity).key.clone();
        if text.trim().is_empty() {
            // Saving nothing brings back the built-in rules.
            set.remove(&key);
            if let Some(b) = builtin::source_for(&key).and_then(|t| Overlay::parse(&t).ok()) {
                set.add(b.0);
            }
        } else {
            match Overlay::parse(&text) {
                Err(e) => {
                    self.errors.push(e);
                    return;
                }
                Ok((overlay, diags)) => {
                    self.errors.extend(diags.iter().map(|d| format!("{}: {}", d.location, d.message)));
                    self.errors.extend(overlay.unresolved(lib).into_iter().map(|id| format!("not in the library: {id}")));
                    if !overlay.features.iter().any(|f| f.keys.contains(&key)) {
                        self.errors.push(format!("no [[feature]] with id = \"{}\"", self.id));
                    }
                    set.remove(&key);
                    set.replace(overlay);
                }
            }
        }
        if !self.errors.is_empty() {
            return;
        }
        let draft = build(character, lib, &set).evaluate();
        let value = |ev: &Evaluation, k: &str| ev.stats.get(k).and_then(|r| r.value.as_ref().ok()).map(|v| v.to_string());
        let mut keys: Vec<&String> = draft.stats.keys().chain(current.stats.keys()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let (before, after) = (value(current, k), value(&draft, k));
            if before != after && !k.starts_with("toggle.") {
                self.changes.push(match (before, after) {
                    (Some(b), Some(a)) => format!("{k}: {b} → {a}"),
                    (None, Some(a)) => format!("{k}: new, {a}"),
                    (Some(b), None) => format!("{k}: {b} → gone"),
                    (None, None) => continue,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_drafts_against_the_character() {
        let mut lib = Library::new();
        lib.load_json(&json!({"feat": [{"name": "Stout", "source": "T", "entries": ["x"]}]}));
        let feat = lib.get(&dnt_data::Key::new(Kind::Feat, &["Stout", "T"])).unwrap();
        assert_eq!(overlay_id(&lib, feat), "feat:Stout|T");
        let c = Character { name: "A".into(), feats: vec!["feat:Stout|T".into()], ..Character::default() };
        let overlays = OverlaySet::default();
        let current = build(&c, &lib, &overlays).evaluate();

        let mut ed = OverlayEditor::open(&lib, feat, None);
        assert_eq!(ed.source, Source::New);
        ed.check(&lib, &c, &overlays, &current);
        assert!(ed.errors.is_empty() && ed.changes.is_empty(), "the template changes nothing: {:?}", ed.errors);

        ed.editor =
            EditorState::new(Lines::from("[[feature]]\nid = \"feat:Stout|T\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"5\"\n"));
        ed.check(&lib, &c, &overlays, &current);
        assert_eq!(ed.changes, ["speed: 30 → 35"]);

        ed.editor = EditorState::new(Lines::from("[[feature]]\nid = \"feat:Other|T\"\n"));
        ed.check(&lib, &c, &overlays, &current);
        assert!(ed.errors.iter().any(|e| e.contains("no [[feature]] with id")), "{:?}", ed.errors);
    }
}
