//! Writing a feature's rules (an overlay): which rules it starts from (the
//! user's, dnt's built-in ones, or a template), and checking a draft
//! against a character: problems with it, and what it changes. The editor
//! around it is the frontend's.

use dnt_data::{EntityId, Kind, Library};
use dnt_rules::builder::build;
use dnt_rules::character::Character;
use dnt_rules::overlay::{Overlay, OverlaySet};
use dnt_rules::{builtin, Evaluation};

/// Where the starting text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The user's own overlay file.
    User,
    /// dnt's built-in overlay (saving makes a user version that replaces it).
    Builtin,
    /// A template: nothing automates this feature yet.
    New,
}

/// The overlay id for an entity: `<type>:<reference>` in the data format's pipe
/// syntax.
/// The built-in rules with the user's own over them, and what is wrong in
/// them, each as `file: what` (checked against `lib` when there is one,
/// and the imported homebrew: rules are for every campaign, whichever
/// homebrew each turns on).
pub fn load(store: &dnt_store::Store, lib: Option<&Library>) -> (OverlaySet, Vec<String>) {
    let (mut set, diags) = builtin::overlays();
    let mut brews = Brews::new(store);
    let mut problems: Vec<String> = diags.iter().map(|d| format!("built-in rules: {d}")).collect();
    for path in store.overlay_files() {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match store.read_text(&path).map_err(|e| e.to_string()).and_then(|t| Overlay::parse(&t)) {
            Ok((overlay, diags)) => {
                problems.extend(diags.iter().map(|d| format!("{name}: {d}")));
                let unknown: Vec<&str> = match lib {
                    Some(l) => overlay
                        .features
                        .iter()
                        .filter(|f| !f.keys.iter().any(|k| l.get(k).is_some()) && !brews.has(&f.id, &f.keys))
                        .map(|f| f.id.as_str())
                        .collect(),
                    None => Vec::new(),
                };
                problems.extend(unknown.into_iter().map(|id| format!("{name}: not in the library: {id}")));
                // A user's overlay replaces the built-in one for its feature.
                set.replace(overlay);
            }
            // A parse error's first line says where; the rest quotes the file.
            Err(e) => problems.push(format!("{name}: {}", e.lines().next().unwrap_or_default())),
        }
    }
    (set, problems)
}

/// The imported homebrew's entities, by source, read only for rules that
/// name one (`…|KibDragonPatron`): rules are for every campaign, whichever
/// homebrew each turns on.
struct Brews<'a> {
    store: &'a dnt_store::Store,
    ids: Vec<String>,
    keys: std::collections::HashMap<String, std::collections::HashSet<dnt_data::Key>>,
}

impl<'a> Brews<'a> {
    fn new(store: &'a dnt_store::Store) -> Brews<'a> {
        let ids = crate::import::homebrew(store).into_iter().map(|b| b.id.to_lowercase()).collect();
        Brews { store, ids, keys: Default::default() }
    }

    /// Whether an imported homebrew has one of the keys (`id`'s source
    /// names the homebrew).
    fn has(&mut self, id: &str, keys: &[dnt_data::Key]) -> bool {
        let source = id.rsplit('|').next().unwrap_or_default().to_lowercase();
        if !self.ids.contains(&source) {
            return false;
        }
        let store = self.store;
        let known = self.keys.entry(source.clone()).or_insert_with(|| {
            let (docs, _) = crate::import::load_homebrew(store, &[source]);
            let mut out = std::collections::HashSet::new();
            for doc in &docs {
                for (k, list) in doc.as_object().into_iter().flatten() {
                    let (Some(kind), Some(list)) = (Kind::from_json_key(k), list.as_array()) else { continue };
                    out.extend(list.iter().filter_map(|e| dnt_data::Key::of_entity(kind, e)));
                }
            }
            out
        });
        keys.iter().any(|k| known.contains(k))
    }
}

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

/// The text to start editing a feature's rules from: the user's overlay
/// (`user`), dnt's built-in one, or a template.
pub fn start(lib: &Library, entity: EntityId, user: Option<String>) -> (Origin, String) {
    let (origin, text) = match user {
        Some(t) => (Origin::User, t),
        None => match builtin::source_for(&lib.entity(entity).key) {
            Some(t) => (Origin::Builtin, t),
            None => (Origin::New, dnt_rules::overlay::template(&overlay_id(lib, entity), &lib.entity(entity).name)),
        },
    };
    (origin, text.trim_end().to_string())
}

/// What a draft does: its problems (none: it can be saved) and what it
/// changes on the character (`stat: before → after`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Checked {
    pub errors: Vec<String>,
    pub changes: Vec<String>,
}

/// Check a draft of a feature's rules: parse it, make sure it names this
/// feature, and compare the character with and without it. An empty draft
/// means the built-in rules (if any).
pub fn check(lib: &Library, entity: EntityId, text: &str, character: &Character, overlays: &OverlaySet, current: &Evaluation) -> Checked {
    let mut out = Checked::default();
    let mut set = overlays.clone();
    let key = lib.entity(entity).key.clone();
    if text.trim().is_empty() {
        // Saving nothing brings back the built-in rules.
        set.remove(&key);
        if let Some(b) = builtin::source_for(&key).and_then(|t| Overlay::parse(&t).ok()) {
            set.add(b.0);
        }
    } else {
        match Overlay::parse(text) {
            Err(e) => {
                out.errors.push(e);
                return out;
            }
            Ok((overlay, diags)) => {
                out.errors.extend(diags.iter().map(|d| format!("{}: {}", d.location, d.message)));
                out.errors.extend(overlay.unresolved(lib).into_iter().map(|id| format!("not in the library: {id}")));
                if !overlay.features.iter().any(|f| f.keys.contains(&key)) {
                    out.errors.push(format!("no [[feature]] with id = \"{}\"", overlay_id(lib, entity)));
                }
                set.remove(&key);
                set.replace(overlay);
            }
        }
    }
    if !out.errors.is_empty() {
        return out;
    }
    let draft = build(character, lib, &set).evaluate();
    let value = |ev: &Evaluation, k: &str| ev.stats.get(k).and_then(|r| r.value.as_ref().ok()).map(|v| v.to_string());
    let mut keys: Vec<&String> = draft.stats.keys().chain(current.stats.keys()).collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        let (before, after) = (value(current, k), value(&draft, k));
        if before != after && !k.starts_with("toggle.") {
            out.changes.push(match (before, after) {
                (Some(b), Some(a)) => format!("{k}: {b} → {a}"),
                (None, Some(a)) => format!("{k}: new, {a}"),
                (Some(b), None) => format!("{k}: {b} → gone"),
                (None, None) => continue,
            });
        }
    }
    out
}

/// The text to save from an editor: empty to remove the user's rules,
/// else with one final newline.
pub fn to_save(text: &str) -> String {
    if text.trim().is_empty() {
        String::new()
    } else {
        text.trim_end().to_string() + "\n"
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

        let (origin, text) = start(&lib, feat, None);
        assert_eq!(origin, Origin::New);
        let checked = check(&lib, feat, &text, &c, &overlays, &current);
        assert!(checked.errors.is_empty() && checked.changes.is_empty(), "the template changes nothing: {:?}", checked.errors);

        let draft = "[[feature]]\nid = \"feat:Stout|T\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"5\"\n";
        assert_eq!(check(&lib, feat, draft, &c, &overlays, &current).changes, ["speed: 30 → 35"]);

        let other = check(&lib, feat, "[[feature]]\nid = \"feat:Other|T\"\n", &c, &overlays, &current);
        assert!(other.errors.iter().any(|e| e.contains("no [[feature]] with id")), "{:?}", other.errors);
        assert_eq!((to_save("  \n"), to_save("x\n\n")), (String::new(), "x\n".to_string()));
    }
}
