//! A campaign's notes as a frontend shows them: the folder tree, what a
//! `[[link]]` points to (a compendium entity or another note), quick
//! capture to the inbox, and each character's note. Reading and writing
//! the files is `dnt_store::notes`.

use std::collections::BTreeSet;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use dnt_store::notes::{self, Entry, NoteInfo, Notes, Vars, WikiLink};

/// A row of the folder tree, flattened.
#[derive(Debug, Clone)]
pub struct Row {
    pub entry: Entry,
    pub depth: usize,
}

/// The tree as visible rows, with `expanded` folders open.
pub fn tree(notes: &Notes, expanded: &BTreeSet<String>) -> Vec<Row> {
    fn walk(notes: &Notes, expanded: &BTreeSet<String>, folder: &str, depth: usize, out: &mut Vec<Row>) {
        for e in notes.list(folder) {
            let open = e.is_dir && expanded.contains(&e.path);
            let path = e.path.clone();
            out.push(Row { entry: e, depth });
            if open {
                walk(notes, expanded, &path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(notes, expanded, "", 0, &mut out);
    out
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Template variables for a new note called `title`.
pub fn vars(campaign: &str, title: &str) -> Vars {
    Vars { title: title.to_string(), date: today(), campaign: campaign.to_string(), character: String::new() }
}

/// Paths of notes matching a search query (all notes when empty).
pub fn search(index: &[NoteInfo], query: &str) -> Vec<String> {
    notes::NoteQuery::parse(query).run(index).into_iter().map(|n| n.path.clone()).collect()
}

/// The `[[link]]` at a cursor (row, and column in characters).
pub fn link_at(text: &str, row: usize, col: usize) -> Option<WikiLink> {
    let line = text.lines().nth(row)?;
    // Links are in bytes.
    let byte = line.char_indices().nth(col).map_or(line.len(), |(b, _)| b);
    notes::links(text).into_iter().find(|l| l.line == row && l.start <= byte && byte < l.end)
}

/// What a link points to.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Entity(EntityId),
    /// A note, by path.
    Note(String),
    /// `[[spell:Nope]]`: a compendium link to nothing.
    NoEntity,
    /// A note that does not exist yet (its title).
    NoNote(String),
}

pub fn resolve(lib: &Library, filter: &SourceFilter, index: &[NoteInfo], link: &WikiLink) -> Target {
    if let Some((kind, name)) = link.compendium() {
        return find_entity(lib, kind, name, filter).map_or(Target::NoEntity, Target::Entity);
    }
    match notes::resolve(index, &link.target) {
        Some(n) => Target::Note(n.path.clone()),
        None => Target::NoNote(link.target.clone()),
    }
}

/// Find a compendium entity for a `[[kind:Name]]` link, preferring ones the
/// campaign enables. `kind` is a tag name (`spell`, `item`, `race`, …).
pub fn find_entity(lib: &Library, kind: &str, name: &str, filter: &SourceFilter) -> Option<EntityId> {
    let kinds: Vec<Kind> = match Kind::for_tag(kind) {
        [] => Kind::from_json_key(kind).into_iter().collect(),
        many => many.to_vec(),
    };
    let (name, source) = match name.split_once('@') {
        Some((n, s)) => (n.trim(), Some(s.trim())),
        None => (name, None),
    };
    let matches: Vec<EntityId> = (0..lib.entities().len())
        .filter(|&i| {
            let e = lib.entity(i);
            kinds.contains(&e.kind) && e.name.eq_ignore_ascii_case(name) && source.is_none_or(|s| e.source.eq_ignore_ascii_case(s))
        })
        .collect();
    matches.iter().copied().find(|&i| filter.allows(lib.entity(i))).or_else(|| matches.first().copied())
}

/// Append a timestamped line to the inbox.
pub fn capture(notes: &Notes, text: &str) -> Result<String, String> {
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    notes.capture(&time, text).map(|()| "Captured to Inbox".into()).map_err(|e| format!("Could not capture: {e}"))
}

/// A character's note (`Party/<name>.md`), created from the Character
/// template when missing. Returns its path.
pub fn character_note(notes: &Notes, campaign: &str, name: &str) -> Result<String, String> {
    let path = format!("Party/{}.md", notes::file_safe(name));
    if !notes.exists(&path) {
        let vars = Vars { character: name.to_string(), ..vars(campaign, name) };
        notes.write(&path, &vars.apply(&notes.template("Character"))).map_err(|e| format!("Could not create note: {e}"))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn links_point_to_entities_and_notes() {
        let mut lib = Library::new();
        lib.load_json(&json!({"spell": [{"name": "Fireball", "source": "A"}, {"name": "Fireball", "source": "B"}]}));
        let text = "See [[spell:Fireball]] and [[Strahd|the count]].";
        let fire = link_at(text, 0, 8).unwrap();
        assert_eq!(fire.target, "spell:Fireball");
        assert!(link_at(text, 0, 1).is_none());
        let only_b = SourceFilter { enabled: Some(["b".to_string()].into()), ..Default::default() };
        match resolve(&lib, &only_b, &[], &fire) {
            Target::Entity(e) => assert_eq!(lib.entity(e).source, "B", "the campaign's version"),
            other => panic!("{other:?}"),
        }
        assert_eq!(find_entity(&lib, "spell", "fireball@a", &only_b).map(|e| lib.entity(e).source.as_str()), Some("A"));
        let strahd = link_at(text, 0, 30).unwrap();
        assert_eq!(resolve(&lib, &only_b, &[], &strahd), Target::NoNote("Strahd".into()));
    }
}
