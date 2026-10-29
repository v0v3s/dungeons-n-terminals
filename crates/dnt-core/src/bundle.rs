//! Exports to import elsewhere, and importing any file dnt reads.
//!
//! - A character: a zip (see [`crate::backup`] for its `dnt.toml`) with
//!   `character.toml`, their note, the homebrew they use (each source's
//!   entries as a data-format document under `homebrew/`; a homebrew class
//!   or subclass with all its features) and the user's rules for any of it
//!   under `rules/`. Books are not carried: `dnt.toml` lists them.
//! - A campaign: the campaign folder under `campaign/`, the homebrew
//!   sources it turns on and the global homebrew, and the user's rules.
//! - One entry (a spell, a monster, a class with its features): a JSON
//!   document in the data format, which any import of homebrew reads.
//!
//! [`plan`] says what a file is and what importing it would clash with;
//! [`apply`] imports it (a backup goes through [`crate::backup::restore`]).

use std::collections::BTreeSet;

use dnt_data::source::MemorySource;
use dnt_data::{EntityId, Kind, Library};
use dnt_rules::character::Character;
use dnt_rules::overlay::{resolve_id, Overlay};
use dnt_store::{slugify, GLOBAL_SOURCE};
use serde_json::{json, Value};

use crate::backup::{Manifest, ZipIn, ZipOut};
use crate::import;
use crate::session::CharacterSession;
use crate::workspace::Workspace;

/// An entity and what comes with it: a class's or subclass's features.
fn with_parts(lib: &Library, id: EntityId, out: &mut BTreeSet<EntityId>) {
    if !out.insert(id) {
        return;
    }
    let e = lib.entity(id);
    let s = |d: &Value, k: &str| d.get(k).and_then(Value::as_str).unwrap_or("").to_lowercase();
    let (name, source) = (e.name.to_lowercase(), e.source.to_lowercase());
    let parts: Vec<EntityId> = match e.kind {
        Kind::Class => (0..lib.entities().len())
            .filter(|&i| {
                let p = lib.entity(i);
                p.kind == Kind::ClassFeature && s(&p.data, "className") == name && s(&p.data, "classSource") == source
            })
            .collect(),
        Kind::Subclass => {
            let short = s(&e.data, "shortName");
            (0..lib.entities().len())
                .filter(|&i| {
                    let p = lib.entity(i);
                    p.kind == Kind::SubclassFeature && s(&p.data, "subclassShortName") == short && s(&p.data, "subclassSource") == source
                })
                .collect()
        }
        _ => Vec::new(),
    };
    for p in parts {
        out.insert(p);
    }
}

/// Every entity id written anywhere in a TOML value.
fn ids_in(lib: &Library, v: &toml::Value, out: &mut BTreeSet<EntityId>) {
    match v {
        toml::Value::String(s) if s.contains(':') => {
            if let Some(id) = resolve_id(lib, s) {
                with_parts(lib, id, out);
            }
        }
        toml::Value::Array(a) => a.iter().for_each(|x| ids_in(lib, x, out)),
        toml::Value::Table(t) => t.iter().for_each(|(k, x)| {
            // Keys name things too (`[subclasses] "class:Warlock|XPHB" = …`).
            if let Some(id) = k.contains(':').then(|| resolve_id(lib, k)).flatten() {
                with_parts(lib, id, out);
            }
            ids_in(lib, x, out)
        }),
        _ => {}
    }
}

/// What a character uses: everything their file names, and the features
/// they have.
fn related(lib: &Library, s: &CharacterSession) -> BTreeSet<EntityId> {
    let mut out = BTreeSet::new();
    if let Ok(v) = toml::from_str::<toml::Value>(&s.character().to_toml()) {
        ids_in(lib, &v, &mut out);
    }
    for f in &s.build.features {
        with_parts(lib, f.entity, &mut out);
    }
    out
}

/// Homebrew sources here (lowercase): imported ones, the active campaign's,
/// the global one.
fn homebrew_sources(ws: &Workspace) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = ws.homebrew.iter().map(|b| b.id.to_lowercase()).collect();
    out.extend(ws.active_campaign().map(|c| c.settings.source.to_lowercase()).filter(|s| !s.is_empty()));
    out.insert(GLOBAL_SOURCE.to_lowercase());
    out
}

/// A source's `_meta`, as its own homebrew file has it.
fn meta_of(ws: &Workspace, source: &str) -> Value {
    let from = |doc: Value| {
        let sources = doc["_meta"]["sources"].as_array().cloned().unwrap_or_default();
        sources.into_iter().find(|s| s["json"].as_str().is_some_and(|j| j.eq_ignore_ascii_case(source)))
    };
    let found = if source.eq_ignore_ascii_case(GLOBAL_SOURCE) {
        ws.store.global_homebrew().ok().and_then(from)
    } else if let Some(c) = ws.active_campaign().filter(|c| c.settings.source.eq_ignore_ascii_case(source)) {
        ws.store.homebrew(c).ok().and_then(from)
    } else {
        import::load_homebrew(&ws.store, &[source.to_string()]).0.into_iter().find_map(from)
    };
    let full = ws.lib.as_ref().and_then(|l| l.source_info(source)).map_or(source.to_string(), |i| i.name.clone());
    json!({"sources": [found.unwrap_or_else(|| json!({"json": source, "abbreviation": source, "full": full}))]})
}

/// Entities as data-format documents, one per source.
fn documents(ws: &Workspace, lib: &Library, ids: impl IntoIterator<Item = EntityId>) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = Vec::new();
    for id in ids {
        let e = lib.entity(id);
        let at = match out.iter().position(|(s, _)| s.eq_ignore_ascii_case(&e.source)) {
            Some(i) => i,
            None => {
                out.push((e.source.clone(), json!({"_meta": meta_of(ws, &e.source)})));
                out.len() - 1
            }
        };
        let doc = &mut out[at].1;
        match doc.get_mut(e.kind.json_key()).and_then(Value::as_array_mut) {
            Some(list) => list.push(e.data.clone()),
            None => doc[e.kind.json_key()] = json!([e.data.clone()]),
        }
    }
    out
}

/// The user's rules for any of these entities: (file name, text).
fn rules_for(ws: &Workspace, lib: &Library, ids: &BTreeSet<EntityId>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for path in ws.store.overlay_files() {
        let Ok(text) = ws.store.read_text(&path) else { continue };
        let Ok((overlay, _)) = Overlay::parse(&text) else { continue };
        let ours = overlay.features.iter().any(|f| f.keys.iter().any(|k| lib.get(k).is_some_and(|e| ids.contains(&e))));
        if ours {
            out.push((path.file_name().unwrap_or_default().to_string_lossy().into_owned(), text));
        }
    }
    out
}

fn manifest_bytes(m: &Manifest) -> Result<Vec<u8>, String> {
    toml::to_string(m).map(String::into_bytes).map_err(|e| e.to_string())
}

/// A character, to import elsewhere.
pub fn character(ws: &Workspace, s: &CharacterSession) -> Result<Vec<u8>, String> {
    let lib = ws.lib.as_ref().ok_or("No data yet")?;
    let ids = related(lib, s);
    let brew = homebrew_sources(ws);
    let (ours, books): (Vec<EntityId>, Vec<EntityId>) = ids.iter().partition(|&&e| brew.contains(&lib.entity(e).source.to_lowercase()));
    let mut m = Manifest::new("character", &s.character().name);
    m.needs = books.iter().map(|&e| lib.entity(e).source.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    let mut zip = ZipOut::new();
    zip.add("dnt.toml", &manifest_bytes(&m)?)?;
    zip.add("character.toml", s.character().to_toml().as_bytes())?;
    if let Some(note) = s.note_path().and_then(|p| ws.store.read_text(&p).ok()) {
        zip.add("note.md", note.as_bytes())?;
    }
    for (source, doc) in documents(ws, lib, ours) {
        zip.add(&format!("homebrew/{}.json", slugify(&source)), serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?.as_bytes())?;
    }
    for (name, text) in rules_for(ws, lib, &ids) {
        zip.add(&format!("rules/{name}"), text.as_bytes())?;
    }
    zip.finish()
}

/// Every file under a folder, with paths relative to it.
fn tree(ws: &Workspace, dir: &std::path::Path, rel: &str, out: &mut Vec<String>) {
    for (name, is_dir) in ws.store.fs().list(dir).unwrap_or_default() {
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        if path == "exports" || name.ends_with(".tmp") {
            continue;
        }
        if is_dir {
            tree(ws, &dir.join(&name), &path, out);
        } else {
            out.push(path);
        }
    }
}

/// A campaign with all it has, to import elsewhere.
pub fn campaign(ws: &Workspace, slug: &str) -> Result<Vec<u8>, String> {
    let c = ws.campaigns.iter().find(|c| c.slug == slug).ok_or_else(|| format!("No campaign {slug}"))?;
    let mut m = Manifest::new("campaign", &c.settings.name);
    m.needs = c.settings.sources.clone();
    let mut zip = ZipOut::new();
    zip.add("dnt.toml", &manifest_bytes(&m)?)?;
    let mut files = Vec::new();
    tree(ws, &c.dir, "", &mut files);
    files.sort();
    for rel in files {
        let bytes = ws.store.fs().read(&c.dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        zip.add(&format!("campaign/{rel}"), &bytes)?;
    }
    // The homebrew it turns on, and the global homebrew.
    let (docs, problems) = import::load_homebrew(&ws.store, &c.settings.homebrew);
    if let Some(p) = problems.first() {
        return Err(p.clone());
    }
    for (i, doc) in docs.iter().enumerate() {
        let id = c.settings.homebrew.get(i).cloned().unwrap_or_else(|| format!("homebrew-{i}"));
        zip.add(&format!("homebrew/{}.json", slugify(&id)), serde_json::to_string(doc).map_err(|e| e.to_string())?.as_bytes())?;
    }
    if let Ok(global) = ws.store.global_homebrew() {
        if global.as_object().is_some_and(|o| o.keys().any(|k| k != "_meta")) {
            zip.add("homebrew/global.json", serde_json::to_string(&global).map_err(|e| e.to_string())?.as_bytes())?;
        }
    }
    // The user's rules: few and small, so all of them.
    for path in ws.store.overlay_files() {
        if let Ok(text) = ws.store.read_text(&path) {
            zip.add(&format!("rules/{}", path.file_name().unwrap_or_default().to_string_lossy()), text.as_bytes())?;
        }
    }
    zip.finish()
}

/// One entry (with its features, for a class or subclass), as a document
/// in the data format.
pub fn entry(ws: &Workspace, id: EntityId) -> Result<String, String> {
    let lib = ws.lib.as_ref().ok_or("No data yet")?;
    let mut ids = BTreeSet::new();
    with_parts(lib, id, &mut ids);
    let docs = documents(ws, lib, ids);
    // One source: the entry's own (its features share it).
    let (_, doc) = docs.into_iter().next().ok_or("Nothing to export")?;
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

/// A file name for an export: `Wren Ashby.dnt.zip`, `Fireball.json`.
pub fn file_name(name: &str, ext: &str) -> String {
    format!("{}.{ext}", dnt_store::notes::file_safe(name))
}

// ---------------------------------------------------------------- import

/// What a file is, as found before importing it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Plan {
    /// `backup`, `character`, `campaign`, `entries` or `data`.
    pub what: String,
    /// The character, the campaign, or the entries' names.
    pub name: String,
    /// Books it needs that are not here.
    pub missing: Vec<String>,
    /// What of that name is here already (to replace or keep both).
    pub clash: Option<String>,
    /// For entries: all of them are here already (from a book).
    pub already: bool,
}

/// What to do when the import's character or campaign is here already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnClash {
    /// Stop and say so (ask first, with [`plan`]).
    Refuse,
    Replace,
    KeepBoth,
}

fn is_json(bytes: &[u8], file_name: &str) -> bool {
    file_name.to_lowercase().ends_with(".json") || bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
}

/// The books of `needs` the library does not have.
fn missing(ws: &Workspace, needs: &[String]) -> Vec<String> {
    let have = |s: &str| {
        ws.lib.as_ref().is_some_and(|l| l.source_info(s).is_some()) || ws.imported.iter().any(|(id, _)| id.eq_ignore_ascii_case(s))
    };
    needs.iter().filter(|s| !have(s)).cloned().collect()
}

/// The open campaign's character with this name, if any.
fn character_named(ws: &Workspace, name: &str) -> Option<dnt_store::CharacterFile> {
    ws.characters().into_iter().find(|f| f.character.name.eq_ignore_ascii_case(name))
}

/// What a file is and what importing it would clash with.
pub fn plan(ws: &Workspace, bytes: &[u8], file_name: &str) -> Result<Plan, String> {
    let plain = |what: &str, name: String| Plan { what: what.into(), name, missing: Vec::new(), clash: None, already: false };
    if is_json(bytes, file_name) {
        let doc: Value = serde_json::from_slice(bytes).map_err(|e| format!("Not a data file: {e}"))?;
        let lib = ws.lib.as_ref();
        let brew = homebrew_sources(ws);
        let mut names = Vec::new();
        let mut already = true;
        for (k, v) in doc.as_object().into_iter().flatten().filter(|(k, _)| !k.starts_with('_')) {
            let Some(kind) = Kind::from_json_key(k) else { continue };
            for e in v.as_array().into_iter().flatten() {
                names.push(e["name"].as_str().unwrap_or("?").to_string());
                let here = dnt_data::Key::of_entity(kind, e).and_then(|key| lib?.get(&key));
                let book = here.is_some_and(|id| lib.is_some_and(|l| !brew.contains(&l.entity(id).source.to_lowercase())));
                already &= book;
            }
        }
        if names.is_empty() {
            return Err("The file has no entries to import".into());
        }
        let mut p = plain("entries", names.join(", "));
        p.already = already;
        return Ok(p);
    }
    let mut zip = ZipIn::new(bytes.to_vec())?;
    let Some(m) = zip.manifest()? else { return Ok(plain("data", file_name.to_string())) };
    let mut p = plain(&m.kind, m.name.clone());
    p.missing = missing(ws, &m.needs);
    match m.kind.as_str() {
        "backup" => {}
        "character" => {
            ws.active_campaign().ok_or("Open a campaign to import a character into")?;
            p.clash = character_named(ws, &m.name).map(|f| f.character.name);
        }
        "campaign" => {
            p.clash = ws
                .campaigns
                .iter()
                .find(|c| c.settings.name.eq_ignore_ascii_case(&m.name) || c.slug == slugify(&m.name))
                .map(|c| c.settings.name.clone());
        }
        other => return Err(format!("dnt cannot import a {other}: update dnt")),
    }
    Ok(p)
}

/// `Name (2)`, `Name (3)`…: the first not taken.
fn free_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    (2..).map(|n| format!("{name} ({n})")).find(|n| !taken(n)).unwrap_or_else(|| name.to_string())
}

/// Entries of `from` into `into` (same kind and name replaced).
fn merge_entries(into: &mut Value, from: &Value) {
    for (k, v) in from.as_object().into_iter().flatten().filter(|(k, _)| !k.starts_with('_')) {
        let Some(list) = v.as_array() else { continue };
        if !into[k].is_array() {
            into[k] = json!([]);
        }
        let target = into[k].as_array_mut().unwrap();
        for e in list {
            target.retain(|x| x.get("name") != e.get("name"));
            target.push(e.clone());
        }
    }
}

/// The source a homebrew document holds (its first).
fn doc_source(doc: &Value) -> Option<String> {
    doc["_meta"]["sources"][0]["json"].as_str().map(str::to_string)
}

/// Bring homebrew documents in: the global homebrew's and the open
/// campaign's own merge into theirs; other sources import as homebrew and
/// are turned on in `enable_in` (when given). How many sources.
fn take_homebrew(ws: &mut Workspace, docs: Vec<Value>, enable_in: Option<usize>) -> Result<usize, String> {
    let mut files = MemorySource::default();
    let mut enable = Vec::new();
    for (i, doc) in docs.iter().enumerate() {
        let Some(source) = doc_source(doc) else { continue };
        if source.eq_ignore_ascii_case(GLOBAL_SOURCE) {
            let mut global = ws.store.global_homebrew().map_err(|e| e.to_string())?;
            merge_entries(&mut global, doc);
            ws.store.save_global_homebrew(&global).map_err(|e| e.to_string())?;
            continue;
        }
        let own = enable_in.and_then(|c| ws.campaigns.get(c)).filter(|c| c.settings.source.eq_ignore_ascii_case(&source));
        if let Some(c) = own {
            let mut mine = ws.store.homebrew(c).map_err(|e| e.to_string())?;
            merge_entries(&mut mine, doc);
            ws.store.save_homebrew(c, &mine).map_err(|e| e.to_string())?;
            continue;
        }
        files.insert(format!("homebrew-{i}.json"), doc.to_string());
        enable.push(source);
    }
    if !files.files.is_empty() {
        import::import_files(&ws.store, &files)?;
    }
    if let Some(c) = enable_in.and_then(|c| ws.campaigns.get_mut(c)) {
        for s in &enable {
            if !c.settings.homebrew.iter().any(|h| h.eq_ignore_ascii_case(s)) {
                c.settings.homebrew.push(s.clone());
            }
        }
        ws.store.save_campaign(c).map_err(|e| e.to_string())?;
    }
    Ok(docs.len())
}

/// Rules files into the user's rules (replacing ones of the same name).
fn take_rules(ws: &Workspace, zip: &mut ZipIn) -> Result<usize, String> {
    let names: Vec<String> = zip.names().into_iter().filter(|n| n.starts_with("rules/") && n.ends_with(".toml")).collect();
    for n in &names {
        let text = zip.text(n)?;
        let file = ws.store.overlays_dir().join(n.trim_start_matches("rules/"));
        ws.store.write_text(&file, &text).map_err(|e| e.to_string())?;
    }
    Ok(names.len())
}

fn homebrew_docs(zip: &mut ZipIn) -> Result<Vec<Value>, String> {
    let names: Vec<String> = zip.names().into_iter().filter(|n| n.starts_with("homebrew/") && n.ends_with(".json")).collect();
    names.iter().map(|n| serde_json::from_slice(&zip.read(n)?).map_err(|e| format!("{n}: {e}"))).collect()
}

/// What a count adds to a summary: `, 2 homebrew sources`.
fn and(n: usize, what: &str) -> String {
    match n {
        0 => String::new(),
        1 => format!(", 1 {what}"),
        n => format!(", {n} {what}s"),
    }
}

/// Import a file: entries, a character, a campaign, or data and homebrew.
/// A backup is restored with [`crate::backup::restore`] instead.
pub fn apply(ws: &mut Workspace, bytes: Vec<u8>, file_name: &str, on_clash: OnClash) -> Result<String, String> {
    let p = plan(ws, &bytes, file_name)?;
    if p.clash.is_some() && on_clash == OnClash::Refuse {
        return Err(format!("{} is here already: replace it, or keep both", p.name));
    }
    let summary = match p.what.as_str() {
        "entries" if p.already => format!("{} is in your data already", p.name),
        "entries" => {
            let doc: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let active = ws.active;
            take_homebrew(ws, vec![doc], active)?;
            format!("Imported {}", p.name)
        }
        "data" => import::import_zip(&ws.store, bytes)?.summary(),
        "character" => import_character(ws, bytes, &p, on_clash)?,
        "campaign" => import_campaign(ws, bytes, &p, on_clash)?,
        "backup" => return Err("Restore a backup from Data › Restore (it replaces everything)".into()),
        other => return Err(format!("dnt cannot import a {other}")),
    };
    ws.reload();
    Ok(match p.missing.as_slice() {
        [] => summary,
        books => format!("{summary} · needs {} (not here)", books.join(", ")),
    })
}

fn import_character(ws: &mut Workspace, bytes: Vec<u8>, p: &Plan, on_clash: OnClash) -> Result<String, String> {
    let mut zip = ZipIn::new(bytes)?;
    let active = ws.active.ok_or("Open a campaign to import a character into")?;
    let mut c = Character::from_toml(&zip.text("character.toml")?)?;
    let sources = take_homebrew(ws, homebrew_docs(&mut zip)?, Some(active))?;
    let rules = take_rules(ws, &mut zip)?;
    let campaign = ws.campaigns[active].clone();
    let here = p.clash.as_ref().and_then(|n| character_named(ws, n));
    let file = match (here, on_clash) {
        (Some(mut f), OnClash::Replace) => {
            f.character = c;
            ws.store.save_character(&f).map_err(|e| e.to_string())?;
            f
        }
        (here, _) => {
            if here.is_some() {
                let names: Vec<String> = ws.characters().into_iter().map(|f| f.character.name.to_lowercase()).collect();
                c.name = free_name(&c.name, |n| names.contains(&n.to_lowercase()));
            }
            ws.store.create_character(&campaign, c).map_err(|e| e.to_string())?
        }
    };
    if let Ok(note) = zip.text("note.md") {
        let path = campaign.notes_dir().join("Party").join(format!("{}.md", dnt_store::notes::file_safe(&file.character.name)));
        if on_clash == OnClash::Replace || !ws.store.fs().exists(&path) {
            ws.store.write_text(&path, &note).map_err(|e| e.to_string())?;
        }
    }
    Ok(format!("Imported {}{}{}", file.character.name, and(sources, "homebrew source"), and(rules, "rules file")))
}

fn import_campaign(ws: &mut Workspace, bytes: Vec<u8>, p: &Plan, on_clash: OnClash) -> Result<String, String> {
    let mut zip = ZipIn::new(bytes)?;
    let mut settings: dnt_store::CampaignSettings =
        toml::from_str(&zip.text("campaign/campaign.toml")?).map_err(|e| format!("campaign.toml: {e}"))?;
    let here = p.clash.as_ref().and_then(|n| ws.campaigns.iter().find(|c| c.settings.name.eq_ignore_ascii_case(n)).cloned());
    match (&here, on_clash) {
        (Some(c), OnClash::Replace) => ws.store.delete_campaign(c).map_err(|e| e.to_string())?,
        (Some(_), _) => {
            let names: Vec<String> = ws.campaigns.iter().map(|c| c.settings.name.to_lowercase()).collect();
            settings.name = free_name(&settings.name, |n| names.contains(&n.to_lowercase()));
        }
        (None, _) => {}
    }
    let campaign = ws.store.create_campaign(settings.clone()).map_err(|e| e.to_string())?;
    for name in zip.names() {
        let Some(rel) = name.strip_prefix("campaign/") else { continue };
        let bytes = zip.read(&name)?;
        ws.store.fs().write(&campaign.dir.join(rel), &bytes).map_err(|e| format!("{rel}: {e}"))?;
    }
    // Its own settings, with the name it got here.
    ws.store.save_campaign(&campaign).map_err(|e| e.to_string())?;
    let sources = take_homebrew(ws, homebrew_docs(&mut zip)?, None)?;
    let rules = take_rules(ws, &mut zip)?;
    // The list again (sorted by name), with the open campaign still open;
    // with none open, this one.
    let open = ws.active_campaign().map(|c| c.slug.clone());
    ws.campaigns = ws.store.campaigns().0;
    ws.active = open.as_ref().and_then(|s| ws.campaigns.iter().position(|c| &c.slug == s));
    if open.is_none() {
        ws.activate(&campaign.slug)?;
    }
    Ok(format!("Imported the campaign {}{}{}", campaign.settings.name, and(sources, "homebrew source"), and(rules, "rules file")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_store::fs::MemFs;
    use dnt_store::Store;
    use std::sync::Arc;

    /// A device: the book B imported, the homebrew HB1 (a subclass with a
    /// feature) when `brew`, and a campaign that turns it on.
    fn device(brew: bool) -> Workspace {
        let store = Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml");
        let mut ws = Workspace::open(store);
        let mut src = MemorySource::default();
        src.insert(
            "book.json",
            json!({"class": [{"name": "Hexer", "source": "B", "hd": {"number": 1, "faces": 8}, "classFeatures": []}],
                   "feat": [{"name": "Lucky", "source": "B", "entries": ["x"]}]})
            .to_string(),
        );
        if brew {
            src.insert(
                "brew.json",
                json!({"_meta": {"sources": [{"json": "HB1", "abbreviation": "HB1", "full": "Dragons"}]},
                       "subclass": [{"name": "Dragon", "shortName": "Dragon", "source": "HB1", "className": "Hexer", "classSource": "B", "subclassFeatures": []}],
                       "subclassFeature": [{"name": "Draconic Blast", "source": "HB1", "className": "Hexer", "classSource": "B",
                                            "subclassShortName": "Dragon", "subclassSource": "HB1", "level": 1, "entries": ["x"]}],
                       "spell": [{"name": "Unused", "source": "HB1", "level": 0, "entries": ["x"]}]})
                .to_string(),
            );
        }
        import::import_files(&ws.store, &src).unwrap();
        ws.reload();
        let slug = ws.create_campaign("Tidewatch", vec!["B".into()]).unwrap();
        if brew {
            ws.set_sources(&slug, vec!["B".into()], vec!["HB1".into()], false).unwrap();
        }
        ws
    }

    fn wren(ws: &Workspace) -> CharacterSession {
        let c = Character::from_toml(
            "name = \"Wren\"\nfeats = [\"feat:Lucky|B\"]\n\n[[levels]]\nclass = \"class:Hexer|B\"\n\n[subclasses]\n\"class:Hexer|B\" = \"subclass:Dragon|Hexer|B|HB1\"\n",
        )
        .unwrap();
        let file = ws.store.create_character(ws.active_campaign().unwrap(), c).unwrap();
        CharacterSession::new(file, ws.lib.as_ref().unwrap(), &ws.overlays, &ws.filter())
    }

    #[test]
    fn a_character_goes_with_the_homebrew_and_rules_they_use() {
        let ws = device(true);
        let rules = "[[feature]]\nid = \"subclassFeature:Draconic Blast|Hexer|B|Dragon|HB1|1|HB1\"\n";
        ws.store.save_feature_overlay("subclassFeature:Draconic Blast|Hexer|B|Dragon|HB1|1|HB1", rules).unwrap();
        ws.store.save_feature_overlay("feat:Unrelated|X", "[[feature]]\nid = \"feat:Unrelated|X\"\n").unwrap();
        let s = wren(&ws);
        let bytes = character(&ws, &s).unwrap();
        let mut zip = ZipIn::new(bytes.clone()).unwrap();
        let m = zip.manifest().unwrap().unwrap();
        assert_eq!((m.kind.as_str(), m.name.as_str(), m.needs.clone()), ("character", "Wren", vec!["B".to_string()]));
        // The subclass and its feature, not the rest of the homebrew.
        let doc: Value = serde_json::from_slice(&zip.read("homebrew/hb1.json").unwrap()).unwrap();
        assert_eq!(doc["subclass"][0]["name"], "Dragon");
        assert_eq!(doc["subclassFeature"][0]["name"], "Draconic Blast");
        assert!(doc.get("spell").is_none(), "{doc}");
        let rules: Vec<String> = zip.names().into_iter().filter(|n| n.starts_with("rules/")).collect();
        assert_eq!(rules.len(), 1, "{rules:?}");

        // Another device with the book but not the homebrew.
        let mut other = device(false);
        let p = plan(&other, &bytes, "Wren.dnt.zip").unwrap();
        assert_eq!((p.what.as_str(), p.clash.clone(), p.missing.clone()), ("character", None, vec![]));
        let done = apply(&mut other, bytes.clone(), "Wren.dnt.zip", OnClash::Refuse).unwrap();
        assert!(done.starts_with("Imported Wren, 1 homebrew source, 1 rules file"), "{done}");
        let here = other.characters();
        assert_eq!(here.len(), 1);
        let s = CharacterSession::new(here[0].clone(), other.lib.as_ref().unwrap(), &other.overlays, &other.filter());
        assert!(s.build.warnings.iter().all(|w| !w.contains("Dragon")), "{:?}", s.build.warnings);
        assert!(other.active_campaign().unwrap().settings.homebrew.contains(&"HB1".to_string()));

        // Again: here already. Refused unless told; both kept when asked.
        assert_eq!(plan(&other, &bytes, "x").unwrap().clash.as_deref(), Some("Wren"));
        assert!(apply(&mut other, bytes.clone(), "x", OnClash::Refuse).is_err());
        apply(&mut other, bytes.clone(), "x", OnClash::KeepBoth).unwrap();
        let names: Vec<String> = other.characters().into_iter().map(|f| f.character.name).collect();
        assert_eq!(names, ["Wren", "Wren (2)"]);
        apply(&mut other, bytes, "x", OnClash::Replace).unwrap();
        assert_eq!(other.characters().len(), 2);
    }

    #[test]
    fn a_campaign_and_a_single_entry_move_too() {
        let ws = device(true);
        wren(&ws);
        let bytes = campaign(&ws, &ws.active_campaign().unwrap().slug).unwrap();
        let mut other = device(false);
        // A campaign of that name is there: kept both.
        assert_eq!(plan(&other, &bytes, "x").unwrap().clash.as_deref(), Some("Tidewatch"));
        let done = apply(&mut other, bytes, "x", OnClash::KeepBoth).unwrap();
        assert!(done.starts_with("Imported the campaign Tidewatch (2)"), "{done}");
        let c = other.campaigns.iter().find(|c| c.settings.name == "Tidewatch (2)").unwrap();
        assert_eq!(other.store.characters(c).0.len(), 1);
        assert!(other.homebrew.iter().any(|b| b.id == "HB1"));
        assert_eq!(other.active_campaign().unwrap().settings.name, "Tidewatch", "the open one stays open");

        // One entry: a subclass with its feature.
        let lib = ws.lib.as_ref().unwrap();
        let id = resolve_id(lib, "subclass:Dragon|Hexer|B|HB1").unwrap();
        let text = entry(&ws, id).unwrap();
        let mut third = device(false);
        let p = plan(&third, text.as_bytes(), "Dragon.json").unwrap();
        assert_eq!((p.what.as_str(), p.name.as_str(), p.already), ("entries", "Dragon, Draconic Blast", false));
        apply(&mut third, text.into_bytes(), "Dragon.json", OnClash::Refuse).unwrap();
        assert!(resolve_id(third.lib.as_ref().unwrap(), "subclass:Dragon|Hexer|B|HB1").is_some(), "on in the open campaign");
        // A book's entry is here already.
        let lucky = entry(&ws, resolve_id(lib, "feat:Lucky|B").unwrap()).unwrap();
        assert!(plan(&third, lucky.as_bytes(), "Lucky.json").unwrap().already);
    }
}
