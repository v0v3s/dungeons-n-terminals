//! The dnt app: the commands the web UI calls. Everything game-related is
//! dnt-core; these modules hold the app's state and turn it into JSON.

mod characters;
mod homebrew;
mod library;
mod notes;

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use dnt_core::compendium::Search;
use dnt_core::creation::NewCharacter;
use dnt_core::levelup::LevelUp;
use dnt_core::workspace::Workspace;
use dnt_core::CharacterSession;
use dnt_store::Store;
use tauri::Manager;

/// What the app has open.
pub struct Inner {
    pub ws: Workspace,
    /// The compendium search (rebuilt when the library changes).
    pub search: Option<Search>,
    /// The open character.
    pub sheet: Option<CharacterSession>,
    pub levelup: Option<LevelUp>,
    pub creation: Option<NewCharacter>,
    /// The homebrew entry being edited.
    pub editing: Option<dnt_core::homebrew::EntryRef>,
}

impl Inner {
    /// The library changed (an import, another campaign): drop what was
    /// built on the old one.
    pub fn library_changed(&mut self) {
        self.search = None;
        self.levelup = None;
        self.creation = None;
        if let Some(s) = self.sheet.take() {
            let slug = s.file.slug.clone();
            let _ = characters::open_slug(self, &slug);
        }
    }
}

/// The app's state. It is managed from the start but filled in by setup
/// (opening the store and the library takes a moment, more on a phone),
/// so commands wait for it.
#[derive(Default)]
pub struct App {
    inner: Mutex<Option<Inner>>,
    ready: Condvar,
    /// What setup is doing, for the loading screen.
    stage: Arc<Mutex<String>>,
}

/// The state, once ready.
pub struct Guard<'a>(MutexGuard<'a, Option<Inner>>);

impl Deref for Guard<'_> {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        self.0.as_ref().expect("ready")
    }
}

impl DerefMut for Guard<'_> {
    fn deref_mut(&mut self) -> &mut Inner {
        self.0.as_mut().expect("ready")
    }
}

impl App {
    /// The state, waiting for setup to finish.
    pub fn lock(&self) -> Guard<'_> {
        let guard = self.inner.lock().unwrap();
        let (guard, _) = self.ready.wait_timeout_while(guard, Duration::from_secs(60), |i| i.is_none()).unwrap();
        Guard(guard)
    }
}

pub type Res<T> = Result<T, String>;

#[derive(serde::Serialize)]
pub struct Startup {
    ready: bool,
    /// What is being loaded.
    stage: String,
}

/// Whether setup is done, and what it is doing; answers at once.
#[tauri::command]
fn startup(app: tauri::State<'_, App>) -> Startup {
    let ready = app.inner.try_lock().is_ok_and(|i| i.is_some());
    Startup { ready, stage: app.stage.lock().unwrap().clone() }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(App::default())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            let handle = app.handle().clone();
            // Off the main thread: the window shows while the library loads.
            std::thread::spawn(move || {
                let state = handle.state::<App>();
                let stage = state.stage.clone();
                let report = move |s: &str| *stage.lock().unwrap() = s.to_string();
                report("Opening");
                let mut ws = Workspace::open_reporting(Store::open(dir.join("data"), dir.join("config.toml")), report.clone());
                // The compendium is the first screen: index it now.
                report("Indexing the compendium");
                let start = std::time::Instant::now();
                let search = ws.lib.as_ref().map(|lib| Search::new(lib, ws.filter()));
                ws.progress.done.push(("Indexing the compendium".into(), start.elapsed()));
                *state.inner.lock().unwrap() = Some(Inner { ws, search, sheet: None, levelup: None, creation: None, editing: None });
                state.ready.notify_all();
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            startup,
            library::status,
            library::import_zip,
            library::clear_imported,
            library::reset_app,
            library::sources,
            library::create_campaign,
            library::activate_campaign,
            library::set_campaign_sources,
            library::rename_campaign,
            library::delete_campaign,
            library::search,
            library::entry,
            library::follow,
            homebrew::brew_actions,
            homebrew::brew_original,
            homebrew::brew_open,
            homebrew::brew_clone,
            homebrew::brew_templates,
            homebrew::brew_create,
            homebrew::brew_set,
            homebrew::brew_json,
            homebrew::brew_replace,
            homebrew::brew_delete,
            homebrew::brew_promote,
            homebrew::brew_close,
            characters::roster,
            characters::open_character,
            characters::close_character,
            characters::delete_character,
            characters::sheet,
            characters::command,
            characters::roll,
            characters::cycle_advantage,
            characters::breakdown,
            characters::conditions,
            characters::options,
            characters::pending,
            characters::export_sheet,
            characters::character_text,
            characters::kills_outright,
            characters::rules_open,
            characters::rules_check,
            characters::rules_save,
            characters::character_set_text,
            characters::levelup_start,
            characters::levelup_answer,
            characters::levelup_apply,
            characters::creation_start,
            characters::creation_current,
            characters::creation_cancel,
            characters::creation_answer,
            characters::creation_back,
            characters::creation_preview,
            characters::creation_finish,
            notes::note_tree,
            notes::note_read,
            notes::note_write,
            notes::note_create,
            notes::note_create_folder,
            notes::note_rename,
            notes::note_delete,
            notes::note_search,
            notes::note_resolve,
            notes::note_capture,
            notes::note_new_session,
            notes::note_log,
            notes::note_character_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
