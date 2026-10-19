//! Importing data from a zip: a whole data folder (one with `books.json`,
//! whose files are taken as its indexes list them), or loose files of any
//! layout (a homebrew class, a few spells). Entities are merged by who they
//! are (kind, name, source, and the class of a subclass), so importing the
//! same thing again replaces it instead of adding a copy; entities a zip
//! does not have are kept.
//!
//! The imported library is stored under `library/imported/`:
//!
//! ```text
//! books.json           books and adventures
//! spell-sources.json   which classes have which spells
//! sources/<source>.json  every entity of one source
//! homebrew/<source>.json every entity of one homebrew source
//! homebrew/index.json    what each homebrew source is
//! ```
//!
//! Loose files with homebrew metadata (`_meta.sources`) are homebrew: kept
//! apart, listed in the index, and loaded only where a campaign enables
//! them. Files are read one at a time, so a large collection imports
//! without being held in memory at once.

use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::Mutex;

use dnt_data::id::entity_source;
use dnt_data::source::{DataSource, MemorySource};
use dnt_data::{Key, Kind, Library};
use dnt_store::{slugify, Store};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub added: usize,
    pub replaced: usize,
    pub unchanged: usize,
    /// Sources that got entities.
    pub sources: Vec<String>,
    /// Files left out, and why.
    pub skipped: Vec<(String, String)>,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut s = format!("Imported {} new, {} updated, {} unchanged", self.added, self.replaced, self.unchanged);
        match self.sources.len() {
            0 => {}
            1..=6 => s.push_str(&format!(" · sources: {}", self.sources.join(", "))),
            n => s.push_str(&format!(" · {n} sources")),
        }
        if !self.skipped.is_empty() {
            s.push_str(&format!(" · {} file(s) skipped", self.skipped.len()));
        }
        s
    }
}

/// A zip read on demand.
struct ZipSource(Mutex<zip::ZipArchive<Cursor<Vec<u8>>>>);

impl DataSource for ZipSource {
    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        let mut zip = self.0.lock().unwrap();
        let mut file = zip.by_name(path).map_err(std::io::Error::other)?;
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}

/// The files of a folder inside another source, by their path below it.
struct Within<'a, S: DataSource> {
    src: &'a S,
    prefix: &'a str,
}

impl<S: DataSource> DataSource for Within<'_, S> {
    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        self.src.read(&format!("{}{path}", self.prefix))
    }
}

/// Files that look like data but are not entities to import.
fn ignored(name: &str) -> bool {
    let file = name.rsplit('/').next().unwrap_or(name);
    file.starts_with("foundry") || file.starts_with("fluff") || file == "index.json"
}

/// Folders of tooling and generated files (`_generated/`, `.github/`,
/// `node_modules/`): left out without a word.
fn hidden(name: &str) -> bool {
    name.split('/').rev().skip(1).any(|dir| dir.starts_with('_') || dir.starts_with('.') || dir == "node_modules")
}

/// Import a zip into the store's imported library.
pub fn import_zip(store: &Store, bytes: Vec<u8>) -> Result<Report, String> {
    let zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Not a zip file: {e}"))?;
    let names = zip.file_names().map(|n| n.map(|n| n.into_owned())).collect::<Result<Vec<String>, _>>();
    let names: Vec<String> = names.map_err(|e| format!("Unreadable zip: {e}"))?.into_iter().filter(|n| !n.ends_with('/')).collect();
    import(store, &names, &ZipSource(Mutex::new(zip)))
}

/// Import files already read (as from a folder), by their paths.
pub fn import_files(store: &Store, files: &MemorySource) -> Result<Report, String> {
    let names: Vec<String> = files.files.keys().cloned().collect();
    import(store, &names, files)
}

fn import(store: &Store, names: &[String], src: &impl DataSource) -> Result<Report, String> {
    // A data folder: the shallowest folder with books.json.
    let root = names.iter().filter(|n| n.rsplit('/').next() == Some("books.json")).min_by_key(|n| n.matches('/').count());
    let mut incoming = Incoming::default();
    let mut report = Report::default();
    let mut taken: Vec<String> = Vec::new();
    if let Some(root) = root {
        let prefix = root.trim_end_matches("books.json");
        let src = Within { src, prefix };
        let files = Library::data_files(&src).map_err(|e| e.to_string())?;
        for (path, parsed) in Library::parse_files(&src, &files) {
            match parsed {
                Ok(doc) => incoming.docs.push(doc),
                // A data folder without some optional file is fine.
                Err(e) => report.skipped.push((format!("{prefix}{path}"), e.to_string())),
            }
            taken.push(format!("{prefix}{path}"));
        }
        for (file, key) in [("books.json", "book"), ("adventures.json", "adventure")] {
            if let Ok(doc) = read(&src, file) {
                incoming.books.push((key, doc));
            }
            taken.push(format!("{prefix}{file}"));
        }
        if let Ok(doc) = read(&src, "spells/sources.json") {
            incoming.spell_sources.push(doc);
        }
        taken.push(format!("{prefix}spells/sources.json"));
    }

    // Loose files outside the data folder; homebrew is stored as it comes.
    let in_root = |n: &str| root.is_some_and(|r| n.starts_with(r.trim_end_matches("books.json")));
    let mut brew = Brew::open(store);
    for name in names {
        if taken.contains(name) || in_root(name) || hidden(name) {
            continue;
        }
        if !name.ends_with(".json") || ignored(name) {
            report.skipped.push((name.clone(), "not a data file".into()));
            continue;
        }
        match read(src, name) {
            Ok(doc) if has_entities(&doc) && is_homebrew(&doc) => brew.merge(store, doc, &mut report)?,
            Ok(doc) if has_entities(&doc) => incoming.docs.push(doc),
            Ok(_) => report.skipped.push((name.clone(), "nothing to import".into())),
            Err(e) => report.skipped.push((name.clone(), e)),
        }
    }
    if incoming.docs.is_empty() && incoming.books.is_empty() && !brew.changed {
        return Err("The zip has no data to import".into());
    }
    if !incoming.docs.is_empty() || !incoming.books.is_empty() {
        let mut lib = Imported::open(store)?;
        lib.merge(incoming, &mut report);
        lib.save(store)?;
    }
    brew.save(store)?;
    report.sources.sort_by_key(|s| s.to_lowercase());
    report.sources.dedup();
    Ok(report)
}

/// Whether a document is homebrew: it says which sources it holds.
fn is_homebrew(doc: &Value) -> bool {
    doc.get("_meta").and_then(|m| m.get("sources")).and_then(Value::as_array).is_some_and(|s| !s.is_empty())
}

fn read(src: &impl DataSource, path: &str) -> Result<Value, String> {
    let bytes = src.read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| format!("not valid JSON: {e}"))
}

fn has_entities(doc: &Value) -> bool {
    doc.as_object().is_some_and(|o| o.iter().any(|(k, v)| Kind::from_json_key(k).is_some() && v.is_array()))
}

/// What a zip brings.
#[derive(Default)]
struct Incoming {
    docs: Vec<Value>,
    books: Vec<(&'static str, Value)>,
    spell_sources: Vec<Value>,
}

/// The imported library as stored.
#[derive(Default)]
struct Imported {
    books: Value,
    spell_sources: Value,
    /// Per source (lowercase): its document.
    sources: BTreeMap<String, Value>,
}

impl Imported {
    fn open(store: &Store) -> Result<Imported, String> {
        let dir = store.imported_dir();
        let read = |name: &str| -> Result<Option<Value>, String> {
            let path = dir.join(name);
            if !store.fs().is_file(&path) {
                return Ok(None);
            }
            let text = store.read_text(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&text).map(Some).map_err(|e| format!("{}: {e}", path.display()))
        };
        let mut lib = Imported {
            books: read("books.json")?.unwrap_or_else(|| json!({})),
            spell_sources: read("spell-sources.json")?.unwrap_or_else(|| json!({})),
            sources: BTreeMap::new(),
        };
        for (name, _) in store.fs().list(&dir.join("sources")).unwrap_or_default() {
            if let Some(doc) = read(&format!("sources/{name}"))? {
                if let Some(id) = doc_source(&doc) {
                    lib.sources.insert(id.to_lowercase(), doc);
                }
            }
        }
        Ok(lib)
    }

    fn merge(&mut self, incoming: Incoming, report: &mut Report) {
        for (key, doc) in incoming.books {
            for book in doc.get(key).and_then(Value::as_array).into_iter().flatten() {
                let id = |b: &Value| b.get("source").or_else(|| b.get("id")).and_then(Value::as_str).map(str::to_lowercase);
                let list = self.books.as_object_mut().unwrap().entry(key).or_insert_with(|| json!([])).as_array_mut().unwrap();
                match list.iter_mut().find(|b| id(b).is_some() && id(b) == id(book)) {
                    Some(old) => *old = book.clone(),
                    None => list.push(book.clone()),
                }
            }
        }
        for lookup in incoming.spell_sources {
            merge_objects(&mut self.spell_sources, lookup);
        }
        // Where each existing entity is: (source, kind) → key → index.
        let mut at: HashMap<(String, Kind), HashMap<String, usize>> = HashMap::new();
        for doc in incoming.docs {
            let Some(top) = doc.as_object() else { continue };
            let metas: Vec<Value> = top.get("_meta").and_then(|m| m.get("sources")).and_then(Value::as_array).cloned().unwrap_or_default();
            let only_source = (metas.len() == 1).then(|| metas[0].get("json").and_then(Value::as_str)).flatten();
            for (k, list) in top {
                let (Some(kind), Some(list)) = (Kind::from_json_key(k), list.as_array()) else { continue };
                for entry in list {
                    let Some(source) = entity_source(entry).or(only_source) else { continue };
                    let key = identity(kind, entry);
                    let id = source.to_lowercase();
                    let doc = self.sources.entry(id.clone()).or_insert_with(|| json!({"_meta": {"sources": [{"json": source}]}}));
                    if let Some(meta) =
                        metas.iter().find(|m| m.get("json").and_then(Value::as_str).is_some_and(|j| j.eq_ignore_ascii_case(source)))
                    {
                        doc["_meta"]["sources"] = json!([meta]);
                    }
                    let list = doc.as_object_mut().unwrap().entry(k).or_insert_with(|| json!([])).as_array_mut().unwrap();
                    let index = at
                        .entry((id.clone(), kind))
                        .or_insert_with(|| list.iter().enumerate().map(|(i, e)| (identity(kind, e), i)).collect());
                    merge_entry(list, index, key, entry, report);
                    if !report.sources.iter().any(|s| s.eq_ignore_ascii_case(source)) {
                        report.sources.push(source.to_string());
                    }
                }
            }
        }
    }

    fn save(&self, store: &Store) -> Result<(), String> {
        let dir = store.imported_dir();
        let write = |path: PathBuf, v: &Value| store.write_text(&path, &v.to_string()).map_err(|e| e.to_string());
        write(dir.join("books.json"), &self.books)?;
        write(dir.join("spell-sources.json"), &self.spell_sources)?;
        for (id, doc) in &self.sources {
            write(dir.join("sources").join(format!("{}.json", file_name(id))), doc)?;
        }
        Ok(())
    }
}

/// Put an entity in a list: in place of the one it is, or at the end.
fn merge_entry(list: &mut Vec<Value>, index: &mut HashMap<String, usize>, key: String, entry: &Value, report: &mut Report) {
    match index.get(&key) {
        Some(&i) if list[i] == *entry => report.unchanged += 1,
        Some(&i) => {
            list[i] = entry.clone();
            report.replaced += 1;
        }
        None => {
            index.insert(key, list.len());
            list.push(entry.clone());
            report.added += 1;
        }
    }
}

/// An imported homebrew source, as the index lists it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BrewSource {
    /// The source id entities use, e.g. `Hunter`.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub authors: Vec<String>,
    /// `one` for homebrew written for the 2024 rules.
    #[serde(default)]
    pub edition: String,
    /// Entities by kind (JSON key), e.g. `[("class", 1), ("spell", 4)]`.
    #[serde(default)]
    pub kinds: Vec<(String, usize)>,
}

impl BrewSource {
    pub fn entries(&self) -> usize {
        self.kinds.iter().map(|(_, n)| n).sum()
    }
}

/// An entity of a homebrew file: its JSON key, kind, and content.
type Taken<'a> = (&'a str, Kind, &'a Value);

/// The imported homebrew while importing: the index; documents are read
/// and written one source at a time.
struct Brew {
    index: BTreeMap<String, BrewSource>,
    changed: bool,
}

impl Brew {
    fn open(store: &Store) -> Brew {
        let index = homebrew(store).into_iter().map(|b| (b.id.to_lowercase(), b)).collect();
        Brew { index, changed: false }
    }

    fn path(store: &Store, id: &str) -> PathBuf {
        store.imported_homebrew_dir().join(format!("{}.json", file_name(id)))
    }

    /// Merge one homebrew file into the stored sources it has entities of.
    fn merge(&mut self, store: &Store, doc: Value, report: &mut Report) -> Result<(), String> {
        let Some(top) = doc.as_object() else { return Ok(()) };
        let meta = top.get("_meta");
        let metas: Vec<Value> = meta.and_then(|m| m.get("sources")).and_then(Value::as_array).cloned().unwrap_or_default();
        let edition = meta.and_then(|m| m.get("edition")).and_then(Value::as_str).unwrap_or_default();
        let only_source = (metas.len() == 1).then(|| metas[0].get("json").and_then(Value::as_str)).flatten();
        // By source: the source as written, and its entities.
        let mut by_source: BTreeMap<String, (&str, Vec<Taken>)> = BTreeMap::new();
        for (k, list) in top {
            let (Some(kind), Some(list)) = (Kind::from_json_key(k), list.as_array()) else { continue };
            for entry in list {
                let Some(source) = entity_source(entry).or(only_source) else { continue };
                by_source.entry(source.to_lowercase()).or_insert((source, Vec::new())).1.push((k.as_str(), kind, entry));
            }
        }
        for (id, (source, entries)) in by_source {
            let path = Brew::path(store, &id);
            let mut stored: Value = if store.fs().is_file(&path) {
                let text = store.read_text(&path).map_err(|e| e.to_string())?;
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?
            } else {
                json!({})
            };
            let source_meta = metas
                .iter()
                .find(|m| m.get("json").and_then(Value::as_str).is_some_and(|j| j.eq_ignore_ascii_case(source)))
                .cloned()
                .unwrap_or_else(|| json!({"json": source}));
            stored["_meta"] = json!({"sources": [source_meta.clone()], "edition": edition});
            let mut at: HashMap<&str, HashMap<String, usize>> = HashMap::new();
            for (k, kind, entry) in entries {
                let list = stored.as_object_mut().unwrap().entry(k).or_insert_with(|| json!([])).as_array_mut().unwrap();
                let index = at.entry(k).or_insert_with(|| list.iter().enumerate().map(|(i, e)| (identity(kind, e), i)).collect());
                merge_entry(list, index, identity(kind, entry), entry, report);
            }
            store.write_text(&path, &stored.to_string()).map_err(|e| e.to_string())?;
            let s = |k: &str| source_meta.get(k).and_then(Value::as_str).map(str::to_string);
            let kinds = stored
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(k, _)| Kind::from_json_key(k).is_some())
                .filter_map(|(k, v)| Some((k.clone(), v.as_array()?.len())))
                .collect();
            let authors = source_meta.get("authors").and_then(Value::as_array).into_iter().flatten();
            self.index.insert(
                id,
                BrewSource {
                    id: source.to_string(),
                    name: s("full").unwrap_or_else(|| source.to_string()),
                    authors: authors.filter_map(|a| a.as_str().map(str::to_string)).collect(),
                    edition: edition.to_string(),
                    kinds,
                },
            );
            report.sources.push(source.to_string());
            self.changed = true;
        }
        Ok(())
    }

    fn save(&self, store: &Store) -> Result<(), String> {
        if !self.changed {
            return Ok(());
        }
        let list: Vec<&BrewSource> = self.index.values().collect();
        let text = serde_json::to_string(&list).map_err(|e| e.to_string())?;
        store.write_text(&store.imported_homebrew_dir().join("index.json"), &text).map_err(|e| e.to_string())
    }
}

/// The imported homebrew sources, by name.
pub fn homebrew(store: &Store) -> Vec<BrewSource> {
    let path = store.imported_homebrew_dir().join("index.json");
    let mut list: Vec<BrewSource> = store.read_text(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    list.sort_by_key(|b| b.name.to_lowercase());
    list
}

/// The documents of some imported homebrew sources, with problems for
/// those that could not be read.
pub fn load_homebrew(store: &Store, ids: &[String]) -> (Vec<Value>, Vec<String>) {
    let (mut docs, mut problems) = (Vec::new(), Vec::new());
    for id in ids {
        let path = Brew::path(store, &id.to_lowercase());
        if !store.fs().is_file(&path) {
            continue;
        }
        match store.read_text(&path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
            Ok(doc) => docs.push(doc),
            Err(e) => problems.push(format!("Homebrew {id}: {e}")),
        }
    }
    (docs, problems)
}

/// Remove an imported homebrew source.
pub fn remove_homebrew(store: &Store, id: &str) -> Result<(), String> {
    let mut brew = Brew::open(store);
    brew.index.remove(&id.to_lowercase());
    brew.changed = true;
    let path = Brew::path(store, &id.to_lowercase());
    if store.fs().is_file(&path) {
        store.fs().remove_file(&path).map_err(|e| format!("{id}: {e}"))?;
    }
    brew.save(store)
}

/// Who an entity is: its library key, or for the few without a name
/// (some subraces), its whole content.
fn identity(kind: Kind, entry: &Value) -> String {
    match Key::of_entity(kind, entry) {
        Some(k) => format!("{:?}|{}", k.kind, k.path),
        None => format!("{kind:?}|{entry}"),
    }
}

/// A file name for a source that stays distinct (`XPHB` → `xphb`).
fn file_name(source: &str) -> String {
    let slug = slugify(source);
    if slug == source.to_lowercase() {
        slug
    } else {
        // Names that slugify alike (`A.B`, `A B`) keep apart.
        let hash = source.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
        format!("{slug}-{hash:08x}")
    }
}

/// The source a stored document holds.
fn doc_source(doc: &Value) -> Option<&str> {
    doc.get("_meta")?.get("sources")?.as_array()?.first()?.get("json")?.as_str()
}

fn merge_objects(into: &mut Value, from: Value) {
    match (into, from) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                match a.get_mut(&k) {
                    Some(old) => merge_objects(old, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (slot, v) => *slot = v,
    }
}

/// Whether anything was imported.
pub fn has_imported(store: &Store) -> bool {
    store.fs().is_file(&store.imported_dir().join("books.json"))
}

/// The imported library, loaded.
pub fn load(store: &Store) -> Result<Library, String> {
    let lib = Imported::open(store)?;
    let docs: Vec<Value> = lib.sources.into_values().collect();
    let lookup = lib.spell_sources.as_object().is_some_and(|o| !o.is_empty()).then_some(lib.spell_sources);
    Ok(Library::load_parsed(&lib.books, &docs, lookup))
}

/// Sources in the imported library, with how many entities each has.
pub fn sources(store: &Store) -> Result<Vec<(String, usize)>, String> {
    let lib = Imported::open(store)?;
    let count = |doc: &Value| {
        doc.as_object()
            .map_or(0, |o| o.iter().filter(|(k, _)| Kind::from_json_key(k).is_some()).filter_map(|(_, v)| v.as_array()).map(Vec::len).sum())
    };
    Ok(lib.sources.values().filter_map(|d| Some((doc_source(d)?.to_string(), count(d)))).collect())
}

/// Remove a source and everything in it from the imported library.
pub fn remove_source(store: &Store, source: &str) -> Result<(), String> {
    let path = store.imported_dir().join("sources").join(format!("{}.json", file_name(&source.to_lowercase())));
    store.fs().remove_file(&path).map_err(|e| format!("{source}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_store::fs::MemFs;
    use std::sync::Arc;

    fn store() -> Store {
        Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml")
    }

    fn files(list: &[(&str, Value)]) -> MemorySource {
        let mut src = MemorySource::default();
        for (path, v) in list {
            src.insert(*path, v.to_string());
        }
        src
    }

    fn data_folder() -> MemorySource {
        files(&[
            ("5e/data/books.json", json!({"book": [{"name": "Test Book", "id": "TB", "source": "TB"}]})),
            ("5e/data/class/index.json", json!({"fighter": "class-fighter.json"})),
            ("5e/data/spells/index.json", json!({"TB": "spells-tb.json"})),
            ("5e/data/class/class-fighter.json", json!({"class": [{"name": "Fighter", "source": "TB"}]})),
            // Same kinds, another format: never imported.
            ("5e/data/class/foundry.json", json!({"class": [{"name": "Fighter", "source": "TB", "advancement": []}]})),
            ("5e/data/spells/spells-tb.json", json!({"spell": [{"name": "Bless", "source": "TB", "level": 1}]})),
            ("5e/data/spells/sources.json", json!({"TB": {"Bless": {"class": [{"name": "Fighter", "source": "TB"}]}}})),
            ("5e/data/items.json", json!({"item": [{"name": "Rope", "source": "TB"}]})),
            ("5e/README.md", json!("not data")),
        ])
    }

    fn count(lib: &Library, kind: Kind) -> usize {
        lib.entities().iter().filter(|e| e.kind == kind).count()
    }

    #[test]
    fn a_data_folder_imports_once() {
        let store = store();
        assert!(!has_imported(&store));
        let report = import_files(&store, &data_folder()).unwrap();
        assert_eq!((report.added, report.replaced, report.unchanged), (3, 0, 0), "{report:?}");
        assert_eq!(report.sources, ["TB"]);
        assert!(report.skipped.iter().any(|(f, _)| f == "5e/README.md"));
        assert!(has_imported(&store));

        let lib = load(&store).unwrap();
        assert_eq!((count(&lib, Kind::Class), count(&lib, Kind::Spell), count(&lib, Kind::Item)), (1, 1, 1));
        assert!(lib.sources().any(|s| s.id == "TB" && s.name == "Test Book"), "books are kept");
        let bless = dnt_rules::overlay::resolve_id(&lib, "spell:Bless|TB").unwrap();
        assert_eq!(lib.spell_classes(bless).len(), 1, "the spell lookup is kept");

        // Again: nothing doubles.
        let again = import_files(&store, &data_folder()).unwrap();
        assert_eq!((again.added, again.replaced, again.unchanged), (0, 0, 3));
        assert_eq!(count(&load(&store).unwrap(), Kind::Class), 1);
    }

    #[test]
    fn partial_homebrew_is_added_and_updated() {
        let store = store();
        import_files(&store, &data_folder()).unwrap();
        let brew = |hd: i64| {
            files(&[(
                "my-brew/Warden.json",
                json!({
                    "_meta": {"sources": [{"json": "WRD", "full": "The Warden", "abbreviation": "WRD"}]},
                    "class": [{"name": "Warden", "source": "WRD", "hd": {"number": 1, "faces": hd}}],
                    "subclass": [{"name": "Oak", "shortName": "Oak", "source": "WRD", "className": "Warden", "classSource": "WRD"}]
                }),
            )])
        };
        let first = import_files(&store, &brew(8)).unwrap();
        assert_eq!((first.added, first.sources.clone()), (2, vec!["WRD".to_string()]));
        let second = import_files(&store, &brew(10)).unwrap();
        assert_eq!((second.added, second.replaced, second.unchanged), (0, 1, 1));

        // Homebrew is kept apart from the data, and listed.
        assert_eq!(count(&load(&store).unwrap(), Kind::Class), 1, "only Fighter");
        let listed = homebrew(&store);
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].id.as_str(), listed[0].name.as_str(), listed[0].entries()), ("WRD", "The Warden", 2));

        let (docs, problems) = load_homebrew(&store, &["wrd".to_string()]);
        assert!(problems.is_empty());
        let mut lib = load(&store).unwrap();
        lib.load_docs(&docs);
        let warden = dnt_rules::overlay::resolve_id(&lib, "class:Warden|WRD").unwrap();
        assert_eq!(lib.entity(warden).data["hd"]["faces"], 10, "the new version");
        assert!(lib.sources().any(|s| s.id == "WRD" && s.name == "The Warden"));

        remove_homebrew(&store, "WRD").unwrap();
        assert!(homebrew(&store).is_empty());
        assert!(load_homebrew(&store, &["WRD".to_string()]).0.is_empty());
    }

    #[test]
    fn data_without_metadata_joins_the_data() {
        let store = store();
        import_files(&store, &data_folder()).unwrap();
        let more = files(&[
            ("spells.json", json!({"spell": [{"name": "Shield", "source": "TB", "level": 1}]})),
            // Tooling of a homebrew collection: not even reported.
            ("_generated/index-meta.json", json!({"spell": []})),
            ("_img/x.png", json!("")),
        ]);
        let report = import_files(&store, &more).unwrap();
        assert_eq!(report.added, 1);
        assert!(report.skipped.is_empty(), "{:?}", report.skipped);
        assert_eq!(count(&load(&store).unwrap(), Kind::Spell), 2);
        let mut list = sources(&store).unwrap();
        list.sort();
        assert_eq!(list, [("TB".to_string(), 4)]);
        remove_source(&store, "TB").unwrap();
    }

    #[test]
    fn zips_are_read() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, bytes) in data_folder().files {
            zip.start_file(name, opts).unwrap();
            std::io::Write::write_all(&mut zip, &bytes).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let store = store();
        assert_eq!(import_zip(&store, bytes).unwrap().added, 3);
        assert!(import_zip(&store, b"not a zip".to_vec()).unwrap_err().contains("Not a zip"));
        let empty = files(&[("notes.json", json!({"hello": 1}))]);
        assert!(import_files(&store, &empty).unwrap_err().contains("no data"));
    }
}
