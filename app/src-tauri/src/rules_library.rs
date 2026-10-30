//! The rules library (github.com/v0v3s/dnt-rules): its tree, and importing
//! rules from it as the user's own.

use std::collections::BTreeSet;

use dnt_core::rules_library::{self as library, RulesLibrary};
use serde::Serialize;
use tauri::State;

use crate::{App, Res};

/// The library's tree, read again (from the web, or the tests' files).
#[tauri::command]
pub async fn rules_library(app: State<'_, App>) -> Res<RulesLibrary> {
    let test = app.lock().rules_test.clone();
    let files = match test {
        Some(files) => files,
        None => tauri::async_runtime::spawn_blocking(|| library::fetch(&library::location())).await.map_err(|e| e.to_string())??,
    };
    let mut inner = app.lock();
    let tree = library::tree(&files, &inner.ws.store);
    inner.rules_files = Some(files);
    Ok(tree)
}

#[derive(Serialize)]
pub struct LibraryImported {
    pub summary: String,
    /// The tree again, the imported rules now the user's.
    pub library: RulesLibrary,
}

/// Import these rules (by id) as the user's; those in `overwrite` replace
/// the user's own where they differ, the others that differ are kept.
#[tauri::command]
pub async fn rules_library_import(app: State<'_, App>, ids: Vec<String>, overwrite: Vec<String>) -> Res<LibraryImported> {
    let mut inner = app.lock();
    let files = inner.rules_files.clone().ok_or("The rules library is not open")?;
    let tree = library::tree(&files, &inner.ws.store);
    let ids: BTreeSet<String> = ids.into_iter().collect();
    let overwrite: BTreeSet<String> = overwrite.into_iter().collect();
    let done = library::import(&inner.ws.store, &tree, &ids, &overwrite)?;
    inner.ws.reload_rules();
    // An open character is read again with them.
    if let Some(slug) = inner.sheet.as_ref().map(|s| s.file.slug.clone()) {
        crate::characters::open_slug(&mut inner, &slug)?;
    }
    let library = library::tree(&files, &inner.ws.store);
    Ok(LibraryImported { summary: done.summary(), library })
}

/// The tests' rules library: its files (`srd/classes.toml` → text).
#[tauri::command]
pub async fn e2e_rules_library(app: State<'_, App>, files: Vec<(String, String)>) -> Res<()> {
    crate::e2e::enabled()?;
    app.lock().rules_test = Some(files);
    Ok(())
}
