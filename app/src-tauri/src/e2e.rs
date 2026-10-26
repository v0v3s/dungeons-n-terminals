//! What the end-to-end tests (app/e2e) need from the app, in a build with
//! the `e2e` feature. Nothing can drive the system's own file dialogs, so
//! the tests answer them (src/lib/files.ts) with paths understood here: an
//! import of `e2e:stash` imports a zip the test handed over, a save to
//! `e2e:<name>` writes into the app's cache, where the test reads it back.
//! A test can also put a file in place, as a sync would.

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{Manager, State};

use crate::{App, Res};

/// The zip a test handed over, for an import of `e2e:stash`.
static STASH: Mutex<Option<Vec<u8>>> = Mutex::new(None);

fn enabled() -> Res<()> {
    if cfg!(feature = "e2e") {
        Ok(())
    } else {
        Err("Not in this build".into())
    }
}

/// Hand over a zip for the next import of `e2e:stash`.
#[tauri::command]
pub async fn e2e_stash_zip(bytes: Vec<u8>) -> Res<()> {
    enabled()?;
    *STASH.lock().unwrap() = Some(bytes);
    Ok(())
}

/// The handed-over zip, when `path` is `e2e:stash`.
pub fn stashed(path: &str) -> Option<Vec<u8>> {
    if !cfg!(feature = "e2e") || path != "e2e:stash" {
        return None;
    }
    STASH.lock().unwrap().clone()
}

/// Where a save to `e2e:<name>` goes: the app's cache.
pub fn save_path(handle: &tauri::AppHandle, path: &str) -> Option<PathBuf> {
    let name = path.strip_prefix("e2e:").filter(|_| cfg!(feature = "e2e"))?;
    let dir = handle.path().app_cache_dir().ok()?.join("e2e");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(name))
}

/// Read back a file saved to `e2e:<name>`.
#[tauri::command]
pub async fn e2e_read(handle: tauri::AppHandle, path: String) -> Res<String> {
    enabled()?;
    let file = save_path(&handle, &path).ok_or("Not an e2e: path")?;
    std::fs::read_to_string(file).map_err(|e| e.to_string())
}

/// Put a file in place, as a sync from another device would: a character
/// (`characters/<name>.toml`, in the active campaign) or rules
/// (`overlays/<name>.toml`, read again at once).
#[tauri::command]
pub async fn e2e_put_file(app: State<'_, App>, path: String, text: String) -> Res<()> {
    enabled()?;
    let mut inner = app.lock();
    let file = match path.split_once('/') {
        Some(("characters", name)) => inner.ws.active_campaign().ok_or("No active campaign")?.characters_dir().join(name),
        Some(("overlays", name)) => inner.ws.store.overlays_dir().join(name),
        _ => return Err(format!("Not a place for a test file: {path}")),
    };
    std::fs::create_dir_all(file.parent().unwrap_or(&file)).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| e.to_string())?;
    if path.starts_with("overlays/") {
        inner.ws.reload_rules();
    }
    Ok(())
}
