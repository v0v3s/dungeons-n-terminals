//! Data, campaigns and the compendium.

use dnt_core::compendium::{Category, Search};
use dnt_core::import;
use dnt_data::tags::Link;
use dnt_data::{view, EntityId, Kind};
use serde::Serialize;
use tauri::State;

use crate::{App, Inner, Res};

#[derive(Serialize)]
pub struct CampaignInfo {
    slug: String,
    name: String,
    /// Enabled sources (empty: all).
    sources: Vec<String>,
    /// Enabled imported homebrew.
    homebrew: Vec<String>,
    show_reprinted: bool,
    /// The campaign's homebrew source.
    source: String,
    active: bool,
}

#[derive(Serialize)]
pub struct Status {
    /// Whether there is data to read.
    loaded: bool,
    entries: usize,
    imported: Vec<(String, usize)>,
    /// Imported homebrew: (sources, entities).
    homebrew: (usize, usize),
    campaigns: Vec<CampaignInfo>,
    /// The active campaign's name.
    campaign: Option<String>,
    problems: Vec<String>,
    /// How the last load went: (stage, milliseconds).
    timings: Vec<(String, u64)>,
}

pub fn status_of(inner: &Inner) -> Res<Status> {
    let ws = &inner.ws;
    Ok(Status {
        loaded: ws.has_data(),
        entries: ws.lib.as_ref().map_or(0, |l| l.entities().len()),
        imported: ws.imported.clone(),
        homebrew: (ws.homebrew.len(), ws.homebrew.iter().map(|b| b.entries()).sum()),
        campaigns: ws
            .campaigns
            .iter()
            .enumerate()
            .map(|(i, c)| CampaignInfo {
                slug: c.slug.clone(),
                name: c.settings.name.clone(),
                sources: c.settings.sources.clone(),
                homebrew: c.settings.homebrew.clone(),
                show_reprinted: c.settings.show_reprinted,
                source: c.settings.source.clone(),
                active: ws.active == Some(i),
            })
            .collect(),
        campaign: ws.active_campaign().map(|c| c.settings.name.clone()),
        problems: ws.problems.clone(),
        timings: ws.progress.done.iter().map(|(s, d)| (s.clone(), d.as_millis() as u64)).collect(),
    })
}

#[tauri::command]
pub async fn status(app: State<'_, App>) -> Res<Status> {
    status_of(&app.lock())
}

#[derive(Serialize)]
pub struct Imported {
    summary: String,
    skipped: Vec<(String, String)>,
    status: Status,
}

/// Import a zip picked by the user (a path, or a `content://` URI on Android).
#[tauri::command]
pub async fn import_zip(handle: tauri::AppHandle, app: State<'_, App>, path: String) -> Res<Imported> {
    use tauri_plugin_fs::FsExt;
    let file: tauri_plugin_fs::FilePath = path.parse().map_err(|e| format!("{e}"))?;
    let bytes = handle.fs().read(file).map_err(|e| format!("Could not read the file: {e}"))?;
    let mut inner = app.lock();
    let report = import::import_zip(&inner.ws.store, bytes)?;
    inner.ws.reload();
    inner.library_changed();
    Ok(Imported { summary: report.summary(), skipped: report.skipped, status: status_of(&inner)? })
}

/// Forget imported data: only the homebrew, or all of it (campaigns stay).
#[tauri::command]
pub async fn clear_imported(app: State<'_, App>, homebrew_only: bool) -> Res<Status> {
    let mut inner = app.lock();
    // An entry being edited belongs to the old campaign or data.
    inner.editing = None;
    inner.ws.clear_imported(homebrew_only)?;
    inner.library_changed();
    status_of(&inner)
}

/// Start over: the data, every campaign, character and note.
#[tauri::command]
pub async fn reset_app(app: State<'_, App>) -> Res<Status> {
    let mut inner = app.lock();
    // An entry being edited belongs to the old campaign or data.
    inner.editing = None;
    inner.sheet = None;
    inner.ws.reset()?;
    inner.library_changed();
    status_of(&inner)
}

#[derive(Serialize)]
pub struct Source {
    id: String,
    name: String,
    /// `book`, `adventure`, `homebrew`, …
    group: String,
    published: Option<String>,
}

#[derive(Serialize)]
pub struct Sources {
    sources: Vec<Source>,
    /// Named sets of sources: (name, ids).
    presets: Vec<(&'static str, &'static [&'static str])>,
    /// Imported homebrew, by name.
    homebrew: Vec<import::BrewSource>,
}

/// The sources a campaign can enable.
#[tauri::command]
pub async fn sources(app: State<'_, App>) -> Res<Sources> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let brew = &inner.ws.homebrew;
    let mut sources: Vec<Source> = lib
        .sources()
        // Imported homebrew has its own list, and the campaign's own and the
        // global homebrew are always on.
        .filter(|s| !s.homebrew)
        .filter(|s| !brew.iter().any(|b| b.id.eq_ignore_ascii_case(&s.id)))
        .map(|s| Source {
            id: s.id.clone(),
            name: s.name.clone(),
            group: s.group.clone().unwrap_or_else(|| "book".into()),
            published: s.published.clone(),
        })
        .collect();
    sources.sort_by(|a, b| a.group.cmp(&b.group).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Sources { sources, presets: dnt_store::PRESETS.to_vec(), homebrew: brew.clone() })
}

#[tauri::command]
pub async fn create_campaign(app: State<'_, App>, name: String, sources: Vec<String>) -> Res<Status> {
    let mut inner = app.lock();
    let before = inner.ws.active;
    inner.ws.create_campaign(&name, sources)?;
    if inner.ws.active != before {
        inner.library_changed();
    }
    status_of(&inner)
}

#[tauri::command]
pub async fn activate_campaign(app: State<'_, App>, slug: String) -> Res<Status> {
    let mut inner = app.lock();
    // An entry being edited belongs to the old campaign or data.
    inner.editing = None;
    inner.sheet = None;
    inner.ws.activate(&slug)?;
    inner.library_changed();
    status_of(&inner)
}

#[tauri::command]
pub async fn set_campaign_sources(
    app: State<'_, App>,
    slug: String,
    sources: Vec<String>,
    homebrew: Vec<String>,
    show_reprinted: bool,
) -> Res<Status> {
    let mut inner = app.lock();
    inner.ws.set_sources(&slug, sources, homebrew, show_reprinted)?;
    inner.library_changed();
    status_of(&inner)
}

/// Delete a campaign with its characters, notes and homebrew.
#[tauri::command]
pub async fn delete_campaign(app: State<'_, App>, slug: String) -> Res<Status> {
    let mut inner = app.lock();
    // An entry being edited belongs to the old campaign or data.
    inner.editing = None;
    let was_active = inner.ws.active_campaign().is_some_and(|c| c.slug == slug);
    if was_active {
        inner.sheet = None;
    }
    inner.ws.delete_campaign(&slug)?;
    if was_active {
        inner.library_changed();
    }
    status_of(&inner)
}

#[tauri::command]
pub async fn rename_campaign(app: State<'_, App>, slug: String, name: String) -> Res<Status> {
    let mut inner = app.lock();
    inner.ws.rename_campaign(&slug, &name)?;
    status_of(&inner)
}

/// The colour group of a kind, as the TUI marks it.
pub fn group(kind: Kind) -> &'static str {
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
pub struct Row {
    id: EntityId,
    name: String,
    source: String,
    group: &'static str,
}

#[derive(Serialize)]
pub struct Found {
    rows: Vec<Row>,
    total: usize,
    categories: Vec<(&'static str, usize)>,
    category: usize,
    /// What `#`, `*` and `^` mean in this category.
    hints: &'static [(char, &'static str)],
    suggestions: Vec<(String, usize)>,
    /// The orders results can be in (labels), and the one used.
    orders: Vec<&'static str>,
    order: usize,
}

const MAX_ROWS: usize = 400;

#[tauri::command]
pub async fn search(app: State<'_, App>, category: usize, query: String, order: Option<usize>) -> Res<Found> {
    use dnt_core::compendium::{sorted, Order};
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let filter = inner.ws.filter();
    let s = inner.search.get_or_insert_with(|| Search::new(lib, filter));
    let category = category.min(Category::ORDER.len() - 1);
    if s.category != Category::ORDER[category] {
        s.set_category(lib, Category::ORDER[category]);
    }
    if s.query != query {
        s.set_query(lib, query);
    }
    let order = order.unwrap_or(0).min(Order::ALL.len() - 1);
    let ids = sorted(lib, &s.results, Order::ALL[order]);
    let rows = ids
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
        orders: Order::ALL.iter().map(|o| o.label()).collect(),
        order,
    })
}

#[derive(Serialize)]
pub struct Entry {
    id: EntityId,
    name: String,
    kind: &'static str,
    group: &'static str,
    source: String,
    page: Option<u64>,
    blocks: Vec<dnt_data::doc::Block>,
}

impl Entry {
    pub fn set_blocks(&mut self, blocks: Vec<dnt_data::doc::Block>) {
        self.blocks = blocks;
    }

    /// Say where it is from beyond its source (e.g. "book original").
    pub fn set_source_note(&mut self, note: &str) {
        self.source = format!("{} · {note}", self.source);
    }
}

pub fn entry_of(inner: &Inner, id: EntityId) -> Res<Entry> {
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    if id >= lib.entities().len() {
        return Err(format!("No entry {id}"));
    }
    let e = lib.entity(id);
    Ok(Entry {
        id,
        name: e.name.clone(),
        kind: e.kind.label(),
        group: group(e.kind),
        source: lib.source_info(&e.source).map_or(e.source.clone(), |s| s.name.clone()),
        page: e.data.get("page").and_then(|p| p.as_u64()),
        blocks: view::document(lib, id, &inner.ws.filter()),
    })
}

#[tauri::command]
pub async fn entry(app: State<'_, App>, id: EntityId) -> Res<Entry> {
    entry_of(&app.lock(), id)
}

/// The entry a link in an entry points to.
#[tauri::command]
pub async fn follow(app: State<'_, App>, tag: String, args: String) -> Res<Option<EntityId>> {
    let inner = app.lock();
    Ok(inner.ws.lib.as_ref().and_then(|l| l.resolve(&Link { tag, args })))
}
