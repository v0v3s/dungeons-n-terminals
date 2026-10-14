//! The in-memory library of all imported entities.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::copy;
use crate::id::{display_name, entity_source, Key};
use crate::kind::Kind;
use crate::tags::Link;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: {source}")]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("{0} does not look like a data data directory (no books.json)")]
    NotADataDir(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub kind: Kind,
    pub key: Key,
    pub name: String,
    pub source: String,
    pub data: Value,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    /// Source abbreviation as written in the data, e.g. `XPHB`.
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub published: Option<String>,
    pub homebrew: bool,
}

/// Index into [`Library::entities`].
pub type EntityId = usize;

#[derive(Debug, Clone)]
pub struct ClassRef {
    pub name: String,
    pub source: String,
}

#[derive(Default)]
pub struct Library {
    entities: Vec<Entity>,
    index: HashMap<Key, EntityId>,
    sources: BTreeMap<String, SourceInfo>,
    spell_classes: HashMap<EntityId, Vec<ClassRef>>,
    /// Non-fatal problems encountered while loading.
    pub warnings: Vec<String>,
    /// Entities ingested but not yet indexed (awaiting `_copy` resolution).
    pending: Vec<(Kind, Value)>,
    pending_spell_lookup: Option<Value>,
}

/// Top-level files in a data `data/` directory that hold player-facing content.
const DATA_FILES: &[&str] = &[
    "races.json",
    "backgrounds.json",
    "feats.json",
    "items.json",
    "items-base.json",
    "magicvariants.json",
    "optionalfeatures.json",
    "conditionsdiseases.json",
    "languages.json",
    "skills.json",
    "senses.json",
    "actions.json",
    "variantrules.json",
];

impl Library {
    pub fn new() -> Library {
        Library::default()
    }

    /// Load a data data directory (either the repository root or its `data/`).
    pub fn load_data_dir(root: &Path) -> Result<Library, LoadError> {
        let data = if root.join("data/books.json").is_file() {
            root.join("data")
        } else if root.join("books.json").is_file() {
            root.to_path_buf()
        } else {
            return Err(LoadError::NotADataDir(root.to_path_buf()));
        };

        let mut files: Vec<PathBuf> = DATA_FILES.iter().map(|f| data.join(f)).collect();
        for dir in ["class", "spells"] {
            let index = read_json(&data.join(dir).join("index.json"))?;
            if let Some(map) = index.as_object() {
                files.extend(map.values().filter_map(Value::as_str).map(|f| data.join(dir).join(f)));
            }
        }

        let mut lib = Library::new();
        lib.load_books(&read_json(&data.join("books.json"))?);
        for (path, parsed) in parse_parallel(&files) {
            match parsed {
                Ok(json) => lib.ingest(&json, false),
                Err(e) => lib.warnings.push(format!("{}: {e}", path.display())),
            }
        }
        if let Ok(lookup) = read_json(&data.join("spells/sources.json")) {
            lib.pending_spell_lookup = Some(lookup);
        }
        lib.finish();
        Ok(lib)
    }

    /// Load an additional file (e.g. homebrew) into an already-built library.
    pub fn load_file(&mut self, path: &Path) -> Result<(), LoadError> {
        let json = read_json(path)?;
        self.ingest(&json, true);
        self.finish();
        Ok(())
    }

    fn load_books(&mut self, books: &Value) {
        for book in books.get("book").and_then(Value::as_array).into_iter().flatten() {
            let s = |k: &str| book.get(k).and_then(Value::as_str).map(str::to_string);
            if let (Some(id), Some(name)) = (s("source").or_else(|| s("id")), s("name")) {
                self.sources.insert(
                    id.to_lowercase(),
                    SourceInfo {
                        id,
                        name,
                        group: s("group"),
                        published: s("published"),
                        homebrew: false,
                    },
                );
            }
        }
    }

    /// Add all recognised entities from one JSON document.
    fn ingest(&mut self, json: &Value, homebrew: bool) {
        let Some(top) = json.as_object() else { return };
        if let Some(srcs) = top.get("_meta").and_then(|m| m.get("sources")).and_then(Value::as_array) {
            for src in srcs {
                let s = |k: &str| src.get(k).and_then(Value::as_str).map(str::to_string);
                if let Some(id) = s("json") {
                    self.sources.entry(id.to_lowercase()).or_insert(SourceInfo {
                        name: s("full").unwrap_or_else(|| id.clone()),
                        id,
                        group: Some("homebrew".into()),
                        published: s("dateReleased"),
                        homebrew,
                    });
                }
            }
        }
        for (key, list) in top {
            let (Some(kind), Some(list)) = (Kind::from_json_key(key), list.as_array()) else {
                continue;
            };
            for obj in list {
                self.pending.push((kind, obj.clone()));
            }
        }
    }

    /// Resolve `_copy`s for pending entities and index everything.
    fn finish(&mut self) {
        let mut pending = std::mem::take(&mut self.pending);
        // Copies may chain; resolve until no further progress.
        loop {
            let mut progressed = false;
            let mut still = Vec::new();
            for (kind, obj) in pending {
                if let Some(meta) = obj.get("_copy") {
                    let base = Key::of_entity(kind, meta)
                        .and_then(|k| self.index.get(&k).copied());
                    match base {
                        Some(base) => {
                            let merged = copy::merge(&self.entities[base].data, &obj);
                            self.insert(kind, merged);
                            progressed = true;
                        }
                        None => still.push((kind, obj)),
                    }
                } else {
                    self.insert(kind, obj);
                    progressed = true;
                }
            }
            pending = still;
            if pending.is_empty() || !progressed {
                break;
            }
        }
        for (kind, obj) in pending {
            let name = obj.get("name").and_then(Value::as_str).unwrap_or("?");
            self.warnings.push(format!("{kind}: unresolved _copy for {name}"));
        }

        if let Some(lookup) = self.pending_spell_lookup.take() {
            self.attach_spell_classes(&lookup);
        }
    }

    fn insert(&mut self, kind: Kind, data: Value) {
        let (Some(key), Some(name)) = (Key::of_entity(kind, &data), display_name(kind, &data)) else {
            return;
        };
        let source = entity_source(&data).unwrap_or_default().to_string();
        let entity = Entity {
            kind,
            key: key.clone(),
            name,
            source,
            data,
        };
        // Later definitions (e.g. homebrew re-imports) replace earlier ones.
        match self.index.get(&key) {
            Some(&id) => self.entities[id] = entity,
            None => {
                self.index.insert(key, self.entities.len());
                self.entities.push(entity);
            }
        }
    }

    fn attach_spell_classes(&mut self, lookup: &Value) {
        let Some(by_source) = lookup.as_object() else { return };
        for (source, spells) in by_source {
            for (name, info) in spells.as_object().into_iter().flatten() {
                let Some(&id) = self.index.get(&Key::new(Kind::Spell, &[name, source])) else {
                    continue;
                };
                let classes = info
                    .get("class")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|c| {
                        Some(ClassRef {
                            name: c.get("name")?.as_str()?.to_string(),
                            source: c.get("source")?.as_str()?.to_string(),
                        })
                    })
                    .collect();
                self.spell_classes.insert(id, classes);
            }
        }
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        &self.entities[id]
    }

    pub fn get(&self, key: &Key) -> Option<EntityId> {
        self.index.get(key).copied()
    }

    pub fn sources(&self) -> impl Iterator<Item = &SourceInfo> {
        self.sources.values()
    }

    pub fn source_info(&self, id: &str) -> Option<&SourceInfo> {
        self.sources.get(&id.to_lowercase())
    }

    pub fn spell_classes(&self, id: EntityId) -> &[ClassRef] {
        self.spell_classes.get(&id).map(Vec::as_slice).unwrap_or_default()
    }

    /// Resolve an inline-tag link to an entity.
    pub fn resolve(&self, link: &Link) -> Option<EntityId> {
        let parts: Vec<&str> = link.args.split('|').collect();
        Kind::for_tag(&link.tag)
            .iter()
            .filter_map(|&kind| Key::from_ref(kind, &link.tag, &parts))
            .find_map(|(key, _)| self.get(&key))
    }

    /// Resolve a `classFeatures`/`subclassFeatures` reference string.
    pub fn resolve_feature_ref(&self, kind: Kind, reference: &str) -> Option<EntityId> {
        let tag = match kind {
            Kind::SubclassFeature => "subclassFeature",
            _ => "classFeature",
        };
        let parts: Vec<&str> = reference.split('|').collect();
        Key::from_ref(kind, tag, &parts).and_then(|(key, _)| self.get(&key))
    }

    /// Subclasses belonging to a class.
    pub fn subclasses_of(&self, class: EntityId) -> Vec<EntityId> {
        let c = &self.entities[class];
        let mut subs: Vec<EntityId> = (0..self.entities.len())
            .filter(|&i| {
                let e = &self.entities[i];
                e.kind == Kind::Subclass
                    && e.data.get("className").and_then(Value::as_str) == Some(&c.name)
                    && e.data.get("classSource").and_then(Value::as_str) == Some(&c.source)
            })
            .collect();
        subs.sort_by(|&a, &b| self.entities[a].name.cmp(&self.entities[b].name));
        subs
    }
}

/// Which sources are visible, e.g. per campaign.
#[derive(Debug, Clone, Default)]
pub struct SourceFilter {
    /// Lowercase source ids; `None` means everything is enabled.
    pub enabled: Option<HashSet<String>>,
    /// Hide entities whose `reprintedAs` points to an enabled source.
    pub hide_reprinted: bool,
}

impl SourceFilter {
    pub fn only<I: IntoIterator<Item = S>, S: AsRef<str>>(sources: I) -> SourceFilter {
        SourceFilter {
            enabled: Some(sources.into_iter().map(|s| s.as_ref().to_lowercase()).collect()),
            hide_reprinted: true,
        }
    }

    pub fn source_enabled(&self, source: &str) -> bool {
        self.enabled.as_ref().is_none_or(|e| e.contains(&source.to_lowercase()))
    }

    pub fn allows(&self, entity: &Entity) -> bool {
        if !self.source_enabled(&entity.source) {
            return false;
        }
        if self.hide_reprinted {
            let reprinted = entity
                .data
                .get("reprintedAs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|r| r.as_str().or_else(|| r.get("uid").and_then(Value::as_str)))
                .any(|r| r.split('|').nth(1).is_some_and(|src| self.source_enabled(src)));
            if reprinted {
                return false;
            }
        }
        true
    }
}

fn read_json(path: &Path) -> Result<Value, LoadError> {
    let bytes = std::fs::read(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| LoadError::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and parse files on all cores, preserving input order.
fn parse_parallel(files: &[PathBuf]) -> Vec<(PathBuf, Result<Value, LoadError>)> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(files.len().max(1));
    let chunk = files.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|chunk| scope.spawn(move || chunk.iter().map(|p| (p.clone(), read_json(p))).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("parser thread panicked")).collect()
    })
}
