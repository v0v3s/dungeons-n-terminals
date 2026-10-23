//! The active campaign's notes.

use std::collections::BTreeSet;

use dnt_core::notes::{self as core_notes, Target};
use dnt_data::EntityId;
use dnt_store::notes::{self as store_notes, Notes, WikiLink};
use serde::Serialize;
use tauri::State;

use crate::{App, Inner, Res};

fn notes(inner: &Inner) -> Res<Notes> {
    inner.ws.notes().ok_or_else(|| "Create or activate a campaign first".to_string())
}

fn campaign(inner: &Inner) -> String {
    inner.ws.active_campaign().map(|c| c.settings.name.clone()).unwrap_or_default()
}

#[derive(Serialize)]
pub struct Row {
    path: String,
    name: String,
    is_dir: bool,
    depth: usize,
}

/// The folder tree with these folders open.
#[tauri::command]
pub async fn note_tree(app: State<'_, App>, expanded: Vec<String>) -> Res<Vec<Row>> {
    let inner = app.lock();
    let n = notes(&inner)?;
    let open: BTreeSet<String> = expanded.into_iter().collect();
    Ok(core_notes::tree(&n, &open)
        .into_iter()
        .map(|r| Row { path: r.entry.path, name: r.entry.name, is_dir: r.entry.is_dir, depth: r.depth })
        .collect())
}

#[derive(Serialize)]
pub struct Note {
    path: String,
    title: String,
    text: String,
    /// Notes that link here.
    backlinks: Vec<(String, String)>,
    /// Its ```dnt query blocks, run: what each found.
    queries: Vec<Query>,
}

#[derive(Serialize)]
pub struct Query {
    title: String,
    query: String,
    /// (path, title).
    results: Vec<(String, String)>,
}

#[tauri::command]
pub async fn note_read(app: State<'_, App>, path: String) -> Res<Note> {
    let inner = app.lock();
    let n = notes(&inner)?;
    let text = n.read(&path).map_err(|e| e.to_string())?;
    let index = n.index();
    let info = store_notes::NoteInfo::parse(&path, &text);
    let backlinks = store_notes::backlinks(&index, &info).into_iter().map(|b| (b.path.clone(), b.title.clone())).collect();
    // ```dnt query blocks, run (the hub has a few of its own).
    let queries = core_notes::query_blocks(Some(&info), path == "Campaign.md")
        .into_iter()
        .flat_map(|(title, lines)| {
            let index = &index;
            lines.into_iter().map(move |q| {
                let results = store_notes::NoteQuery::parse(&q).run(index).into_iter().map(|r| (r.path.clone(), r.title.clone())).collect();
                Query { title: if title.is_empty() { q.clone() } else { title.clone() }, query: q, results }
            })
        })
        .collect();
    Ok(Note { title: info.title, path, text, backlinks, queries })
}

#[tauri::command]
pub async fn note_write(app: State<'_, App>, path: String, text: String) -> Res<()> {
    let inner = app.lock();
    let text = if text.ends_with('\n') { text } else { format!("{text}\n") };
    notes(&inner)?.write(&path, &text).map_err(|e| e.to_string())
}

/// A new note in a folder (from the folder's template). Returns its path.
#[tauri::command]
pub async fn note_create(app: State<'_, App>, folder: String, title: String) -> Res<String> {
    let inner = app.lock();
    if title.trim().is_empty() {
        return Err("The note needs a title".into());
    }
    notes(&inner)?.create(&folder, &core_notes::vars(&campaign(&inner), title.trim())).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_create_folder(app: State<'_, App>, parent: String, name: String) -> Res<String> {
    let inner = app.lock();
    notes(&inner)?.create_folder(&parent, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_rename(app: State<'_, App>, path: String, name: String) -> Res<String> {
    let inner = app.lock();
    notes(&inner)?.rename(&path, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn note_delete(app: State<'_, App>, path: String) -> Res<()> {
    let inner = app.lock();
    notes(&inner)?.delete(&path).map_err(|e| e.to_string())
}

/// Notes matching a query (all when empty): (path, title).
#[tauri::command]
pub async fn note_search(app: State<'_, App>, query: String) -> Res<Vec<(String, String)>> {
    let inner = app.lock();
    let index = notes(&inner)?.index();
    Ok(core_notes::search(&index, &query)
        .into_iter()
        .filter_map(|p| index.iter().find(|n| n.path == p).map(|n| (p, n.title.clone())))
        .collect())
}

/// Where a `[[link]]` goes.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Link {
    Entity { id: EntityId },
    Note { path: String },
    Missing { title: String, compendium: bool },
}

#[tauri::command]
pub async fn note_resolve(app: State<'_, App>, target: String) -> Res<Link> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let index = notes(&inner)?.index();
    let link = WikiLink { target: target.clone(), alias: None, line: 0, start: 0, end: 0 };
    Ok(match core_notes::resolve(lib, &inner.ws.filter(), &index, &link) {
        Target::Entity(id) => Link::Entity { id },
        Target::Note(path) => Link::Note { path },
        Target::NoEntity => Link::Missing { title: target, compendium: true },
        Target::NoNote(title) => Link::Missing { title, compendium: false },
    })
}

#[tauri::command]
pub async fn note_capture(app: State<'_, App>, text: String) -> Res<String> {
    let inner = app.lock();
    core_notes::capture(&notes(&inner)?, &text)
}

/// Add a roll to the latest session note. Returns its path, or nothing
/// when there is no session note yet.
#[tauri::command]
pub async fn note_log(app: State<'_, App>, line: String) -> Res<Option<String>> {
    let inner = app.lock();
    notes(&inner)?.log_to_session(&line).map_err(|e| e.to_string())
}

/// Today's session note (a new one). Returns its path.
#[tauri::command]
pub async fn note_new_session(app: State<'_, App>) -> Res<String> {
    let inner = app.lock();
    notes(&inner)?.new_session(&core_notes::vars(&campaign(&inner), "")).map_err(|e| e.to_string())
}

/// A character's note, created if missing. Returns its path.
#[tauri::command]
pub async fn note_character_note(app: State<'_, App>, name: String) -> Res<String> {
    let inner = app.lock();
    core_notes::character_note(&notes(&inner)?, &campaign(&inner), &name)
}
