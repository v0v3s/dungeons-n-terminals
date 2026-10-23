//! Writing a feature's rules (an overlay) from the sheet: TOML in the
//! built-in editor, checked as you type against the open character.

use dnt_core::rules;
use dnt_data::{EntityId, Library};
use dnt_rules::character::Character;
use dnt_rules::overlay::OverlaySet;
use dnt_rules::Evaluation;
use edtui::{EditorEventHandler, EditorMode, EditorState, Lines};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Where the starting text came from.
pub use rules::Origin as Source;

/// The overlay id for an entity.
pub use rules::overlay_id;

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

impl OverlayEditor {
    /// Open the editor for a feature, starting from the user's overlay
    /// (`user`), dnt's built-in one, or a template.
    pub fn open(lib: &Library, entity: EntityId, user: Option<String>) -> OverlayEditor {
        let id = overlay_id(lib, entity);
        let name = lib.entity(entity).name.clone();
        let (source, text) = rules::start(lib, entity, user);
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
        Outcome::Save(rules::to_save(&text))
    }

    /// Check the draft against the open character (see [`rules::check`]).
    pub fn check(&mut self, lib: &Library, character: &Character, overlays: &OverlaySet, current: &Evaluation) {
        let checked = rules::check(lib, self.entity, &self.text(), character, overlays, current);
        (self.errors, self.changes) = (checked.errors, checked.changes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_data::Kind;
    use dnt_rules::builder::build;
    use serde_json::json;

    #[test]
    fn checks_the_editors_text() {
        let mut lib = Library::new();
        lib.load_json(&json!({"feat": [{"name": "Stout", "source": "T", "entries": ["x"]}]}));
        let feat = lib.get(&dnt_data::Key::new(Kind::Feat, &["Stout", "T"])).unwrap();
        let c = Character { name: "A".into(), feats: vec!["feat:Stout|T".into()], ..Character::default() };
        let overlays = OverlaySet::default();
        let current = build(&c, &lib, &overlays).evaluate();
        let mut ed = OverlayEditor::open(&lib, feat, None);
        assert_eq!(ed.source, Source::New);
        ed.editor =
            EditorState::new(Lines::from("[[feature]]\nid = \"feat:Stout|T\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"5\"\n"));
        ed.check(&lib, &c, &overlays, &current);
        assert_eq!(ed.changes, ["speed: 30 → 35"]);
    }
}
