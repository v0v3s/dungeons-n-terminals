//! Backups, exports to import elsewhere, and importing any file (see
//! dnt_core::backup and dnt_core::bundle). Files come from and go to what
//! the system's dialogs picked: a path, or a `content://` URI on Android.

use dnt_core::backup::{self, Saved, Shelf};
use dnt_core::bundle::{self, OnClash, Plan};
use dnt_data::EntityId;
use dnt_store::Every;
use serde::Serialize;
use tauri::State;

use crate::library::{status_of, Status};
use crate::{App, Res};

/// Read a picked file.
fn read(handle: &tauri::AppHandle, path: &str) -> Res<Vec<u8>> {
    use tauri_plugin_fs::FsExt;
    if let Some(bytes) = crate::e2e::stashed(path) {
        return Ok(bytes);
    }
    if let Some(file) = crate::e2e::save_path(handle, path) {
        return std::fs::read(file).map_err(|e| format!("Could not read the file: {e}"));
    }
    let file: tauri_plugin_fs::FilePath = path.parse().map_err(|e| format!("{e}"))?;
    handle.fs().read(file).map_err(|e| format!("Could not read the file: {e}"))
}

/// Write a file where the save dialog said.
fn write(handle: &tauri::AppHandle, path: &str, bytes: &[u8]) -> Res<()> {
    use std::io::Write;
    use tauri_plugin_fs::{FsExt, OpenOptions};
    if let Some(file) = crate::e2e::save_path(handle, path) {
        return std::fs::write(file, bytes).map_err(|e| format!("Could not write the file: {e}"));
    }
    let file: tauri_plugin_fs::FilePath = path.parse().map_err(|e| format!("{e}"))?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut out = handle.fs().open(file, opts).map_err(|e| format!("Could not write the file: {e}"))?;
    out.write_all(bytes).map_err(|e| format!("Could not write the file: {e}"))
}

/// A picked file's name, for telling a data file from a zip.
fn name_of(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

#[derive(Serialize)]
pub struct Backups {
    /// `off`, `day`, `week` or `month`.
    every: Every,
    keep: u32,
    /// Where they go.
    folder: String,
    /// Whether that is a folder the user chose.
    chosen: bool,
    list: Vec<Saved>,
}

fn backups_of(app: &App) -> Backups {
    let inner = app.lock();
    let ws = &inner.ws;
    Backups {
        every: ws.config.backup.every,
        keep: ws.config.backup.keep,
        folder: ws.backups.place.clone(),
        chosen: ws.config.backup.folder.is_some(),
        list: backup::list(&ws.backups),
    }
}

#[tauri::command]
pub async fn backups(app: State<'_, App>) -> Res<Backups> {
    Ok(backups_of(&app))
}

#[tauri::command]
pub async fn backup_now(app: State<'_, App>) -> Res<Backups> {
    app.lock().ws.backup_now()?;
    Ok(backups_of(&app))
}

#[tauri::command]
pub async fn backup_schedule(app: State<'_, App>, every: Every, keep: u32) -> Res<Backups> {
    app.lock().ws.set_backup_schedule(every, keep)?;
    Ok(backups_of(&app))
}

/// The automatic backup at the time given (tests: "a week later"). Only in
/// a build with the `e2e` feature.
#[tauri::command]
pub async fn e2e_backup_auto(app: State<'_, App>, now: String) -> Res<Backups> {
    if !cfg!(feature = "e2e") {
        return Err("Not in this build".into());
    }
    {
        let mut inner = app.lock();
        let ws = &mut inner.ws;
        if let Some(Err(e)) = backup::auto(&ws.store, &ws.backups, &now) {
            return Err(e);
        }
        ws.config = ws.store.load_config().map_err(|e| e.to_string())?;
    }
    Ok(backups_of(&app))
}

/// Pick another backups folder: its path (on Android, a folder the
/// system granted), or null.
#[tauri::command]
pub async fn pick_backup_folder(handle: tauri::AppHandle) -> Res<Option<String>> {
    crate::folders::pick(&handle)
}

/// Back up into `folder` from now on (null: the default folder).
#[tauri::command]
pub async fn backup_folder(handle: tauri::AppHandle, app: State<'_, App>, folder: Option<String>) -> Res<Backups> {
    // A test's folder: in the app's cache.
    let folder = folder.map(|f| crate::e2e::save_path(&handle, &f).map_or(f, |p| p.display().to_string()));
    {
        let mut inner = app.lock();
        let ws = &mut inner.ws;
        let mut config = ws.config.clone();
        config.backup.folder = folder.clone();
        let shelf = shelf(&handle, &ws.store, &config)?;
        shelf.fs.create_dir_all(&shelf.dir).map_err(|e| format!("Could not use {}: {e}", shelf.place))?;
        ws.set_backup_folder(folder, shelf)?;
    }
    Ok(backups_of(&app))
}

/// Replace everything with a backup: a picked file, or one of the list.
#[tauri::command]
pub async fn restore_backup(handle: tauri::AppHandle, app: State<'_, App>, path: String) -> Res<Status> {
    let listed = {
        let inner = app.lock();
        let shelf = inner.ws.backups.clone();
        backup::list(&shelf).into_iter().find(|s| s.path.display().to_string() == path).map(|s| (shelf, s.path))
    };
    let bytes = match listed {
        Some((shelf, file)) => shelf.fs.read(&file).map_err(|e| format!("Could not read the backup: {e}"))?,
        None => read(&handle, &path)?,
    };
    let mut inner = app.lock();
    inner.editing = None;
    inner.sheet = None;
    inner.ws.restore(bytes)?;
    inner.library_changed();
    status_of(&inner)
}

/// What a picked file is, before importing it.
#[tauri::command]
pub async fn import_plan(handle: tauri::AppHandle, app: State<'_, App>, path: String) -> Res<Plan> {
    let bytes = read(&handle, &path)?;
    let inner = app.lock();
    bundle::plan(&inner.ws, &bytes, &name_of(&path))
}

#[derive(Serialize)]
pub struct Imported {
    summary: String,
    status: Status,
}

/// Import a picked file (not a backup: see `restore_backup`).
#[tauri::command]
pub async fn import_file(handle: tauri::AppHandle, app: State<'_, App>, path: String, on_clash: OnClash) -> Res<Imported> {
    let bytes = read(&handle, &path)?;
    let mut inner = app.lock();
    inner.editing = None;
    let summary = bundle::apply(&mut inner.ws, bytes, &name_of(&path), on_clash)?;
    inner.library_changed();
    Ok(Imported { summary, status: status_of(&inner)? })
}

/// The open character, with the homebrew and rules they use.
#[tauri::command]
pub async fn export_character(handle: tauri::AppHandle, app: State<'_, App>, path: String) -> Res<String> {
    let (bytes, name) = {
        let inner = app.lock();
        let s = inner.sheet.as_ref().ok_or("No character is open")?;
        (bundle::character(&inner.ws, s)?, s.character().name.clone())
    };
    write(&handle, &path, &bytes)?;
    Ok(format!("Exported {name}"))
}

#[tauri::command]
pub async fn export_campaign(handle: tauri::AppHandle, app: State<'_, App>, slug: String, path: String) -> Res<String> {
    let bytes = bundle::campaign(&app.lock().ws, &slug)?;
    write(&handle, &path, &bytes)?;
    Ok("Exported the campaign".into())
}

#[tauri::command]
pub async fn export_entry(handle: tauri::AppHandle, app: State<'_, App>, id: EntityId, path: String) -> Res<String> {
    let (text, name) = {
        let inner = app.lock();
        let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
        (bundle::entry(&inner.ws, id)?, lib.entity(id).name.clone())
    };
    write(&handle, &path, text.as_bytes())?;
    Ok(format!("Exported {name}"))
}

/// Where backups go on this device: the folder chosen (on Android, one
/// the system's picker granted), else the default one.
pub fn shelf(handle: &tauri::AppHandle, store: &dnt_store::Store, config: &dnt_store::Config) -> Res<Shelf> {
    match config.backup.folder.as_deref() {
        #[cfg(target_os = "android")]
        Some(tree) if tree.starts_with("content://") => {
            let fs = crate::folders::TreeFs::new(handle, tree.to_string())?;
            Ok(Shelf { fs: std::sync::Arc::new(fs), dir: Default::default(), place: crate::folders::place_of(tree) })
        }
        Some(_) => {
            let _ = handle;
            Ok(Shelf::configured(store, config))
        }
        None => Ok(Shelf::at(store, default_folder(store.data_dir()))),
    }
}

/// The default folder: Android's Downloads (`Download/dnt`, kept if the
/// app is removed) when the app may write there; else the data folder's
/// `backups/`.
fn default_folder(data: &std::path::Path) -> std::path::PathBuf {
    let fallback = data.join("backups");
    if !cfg!(target_os = "android") || cfg!(feature = "e2e") {
        return fallback;
    }
    let downloads = std::path::PathBuf::from("/storage/emulated/0/Download/dnt");
    let probe = downloads.join(".dnt-probe");
    let ok = std::fs::create_dir_all(&downloads).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    if ok {
        downloads
    } else {
        fallback
    }
}
