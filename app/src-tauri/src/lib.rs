//! The dnt app: the commands the web UI calls. Everything game-related is
//! dnt-core; this file holds the app's state and turns it into JSON.

use std::sync::{Mutex, RwLock};

use dnt_core::compendium::{Category, Search};
use dnt_core::import;
use dnt_data::tags::Link;
use dnt_data::{view, EntityId, Kind, Library, SourceFilter};
use dnt_store::Store;
use serde::Serialize;
use tauri::{Manager, State};

struct App {
    store: Store,
    lib: RwLock<Option<Library>>,
    search: Mutex<Option<Search>>,
}

type Res<T> = Result<T, String>;

#[derive(Serialize)]
struct Status {
    /// Whether any data is loaded.
    loaded: bool,
    entries: usize,
    sources: Vec<(String, usize)>,
}

fn status(app: &App) -> Res<Status> {
    let entries = app.lib.read().unwrap().as_ref().map_or(0, |l| l.entities().len());
    let sources = if import::has_imported(&app.store) { import::sources(&app.store)? } else { Vec::new() };
    Ok(Status { loaded: entries > 0, entries, sources })
}

/// Load the imported data (on start, and after an import).
fn reload(app: &App) -> Res<()> {
    let lib = if import::has_imported(&app.store) { Some(import::load(&app.store)?) } else { None };
    *app.search.lock().unwrap() = lib.as_ref().map(|l| Search::new(l, SourceFilter::default()));
    *app.lib.write().unwrap() = lib;
    Ok(())
}

#[tauri::command]
async fn open_library(app: State<'_, App>) -> Res<Status> {
    if app.lib.read().unwrap().is_none() {
        reload(&app)?;
    }
    status(&app)
}

#[derive(Serialize)]
struct Imported {
    summary: String,
    skipped: Vec<(String, String)>,
    status: Status,
}

/// Import a zip picked by the user (a path, or a `content://` URI on Android).
#[tauri::command]
async fn import_zip(handle: tauri::AppHandle, app: State<'_, App>, path: String) -> Res<Imported> {
    use tauri_plugin_fs::FsExt;
    let file: tauri_plugin_fs::FilePath = path.parse().map_err(|e| format!("{e}"))?;
    let bytes = handle.fs().read(file).map_err(|e| format!("Could not read the file: {e}"))?;
    let report = import::import_zip(&app.store, bytes)?;
    reload(&app)?;
    Ok(Imported { summary: report.summary(), skipped: report.skipped, status: status(&app)? })
}

/// The colour group of a kind, as the TUI marks it.
fn group(kind: Kind) -> &'static str {
    use Kind::*;
    match kind {
        Spell => "spell",
        Class | Subclass => "class",
        ClassFeature | SubclassFeature | OptionalFeature => "feature",
        Race | Subrace => "species",
        Background => "background",
        Feat => "feat",
        Item | BaseItem | ItemGroup | MagicVariant | ItemMastery => "item",
        _ => "rule",
    }
}

#[derive(Serialize)]
struct Row {
    id: EntityId,
    name: String,
    source: String,
    group: &'static str,
}

#[derive(Serialize)]
struct Found {
    rows: Vec<Row>,
    total: usize,
    categories: Vec<(&'static str, usize)>,
    category: usize,
    /// What `#`, `*` and `^` mean in this category.
    hints: &'static [(char, &'static str)],
    suggestions: Vec<(String, usize)>,
}

const MAX_ROWS: usize = 400;

#[tauri::command]
async fn search(app: State<'_, App>, category: usize, query: String) -> Res<Found> {
    let lib = app.lib.read().unwrap();
    let lib = lib.as_ref().ok_or("No data yet")?;
    let mut search = app.search.lock().unwrap();
    let s = search.as_mut().ok_or("No data yet")?;
    let category = category.min(Category::ORDER.len() - 1);
    if s.category != Category::ORDER[category] {
        s.set_category(lib, Category::ORDER[category]);
    }
    if s.query != query {
        s.set_query(lib, query);
    }
    let rows = s
        .results
        .iter()
        .take(MAX_ROWS)
        .map(|&id| {
            let e = lib.entity(id);
            Row { id, name: e.name.clone(), source: e.source.clone(), group: group(e.kind) }
        })
        .collect();
    Ok(Found {
        rows,
        total: s.results.len(),
        categories: Category::ORDER.iter().zip(s.category_counts).map(|(c, n)| (c.label(), n)).collect(),
        category,
        hints: s.category.hints(),
        suggestions: s.suggestions.clone(),
    })
}

#[derive(Serialize)]
struct Entry {
    id: EntityId,
    name: String,
    kind: &'static str,
    group: &'static str,
    source: String,
    page: Option<u64>,
    blocks: Vec<dnt_data::doc::Block>,
}

#[tauri::command]
async fn entry(app: State<'_, App>, id: EntityId) -> Res<Entry> {
    let lib = app.lib.read().unwrap();
    let lib = lib.as_ref().ok_or("No data yet")?;
    if id >= lib.entities().len() {
        return Err(format!("No entry {id}"));
    }
    let e = lib.entity(id);
    let filter = app.search.lock().unwrap().as_ref().map(|s| s.filter.clone()).unwrap_or_default();
    Ok(Entry {
        id,
        name: e.name.clone(),
        kind: e.kind.label(),
        group: group(e.kind),
        source: lib.source_info(&e.source).map_or(e.source.clone(), |s| s.name.clone()),
        page: e.data.get("page").and_then(|p| p.as_u64()),
        blocks: view::document(lib, id, &filter),
    })
}

/// The entry a link in an entry points to.
#[tauri::command]
async fn follow(app: State<'_, App>, tag: String, args: String) -> Res<Option<EntityId>> {
    let lib = app.lib.read().unwrap();
    Ok(lib.as_ref().and_then(|l| l.resolve(&Link { tag, args })))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let store = Store::open(dir.join("data"), dir.join("config.toml"));
            app.manage(App { store, lib: RwLock::new(None), search: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_library, import_zip, search, entry, follow])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
