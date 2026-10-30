//! The in-memory library of all imported entities.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::copy;
use crate::id::{display_name, entity_source, Key};
use crate::kind::Kind;
use crate::source::{DataSource, DirSource};
use crate::tags::Link;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{path}: {source}")]
    Json { path: PathBuf, source: serde_json::Error },
    #[error("{0} does not look like a dnt data directory (no books.json; see schemas/README.md)")]
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

#[derive(Default, Clone)]
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
    /// Base entity → its generated `_versions` (lineages, feat variants, …).
    versions: HashMap<EntityId, Vec<EntityId>>,
    version_base: HashMap<EntityId, EntityId>,
    /// Entities replaced by a campaign override: their book data.
    originals: HashMap<EntityId, Value>,
    /// (magic variant, base item) pairs already turned into items.
    generated_variants: std::collections::HashSet<(EntityId, EntityId)>,
}

/// Top-level files in a data directory that hold player-facing content.
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

    /// Load a data directory (it, or its `data/` subdirectory; see schemas/README.md).
    pub fn load_dir(root: &Path) -> Result<Library, LoadError> {
        Library::load(&DirSource::open(root)?)
    }

    /// The data files to load besides the index files, as the indexes in
    /// `src` list them.
    pub fn data_files(src: &impl DataSource) -> Result<Vec<String>, LoadError> {
        let mut files: Vec<String> = DATA_FILES.iter().map(|f| f.to_string()).collect();
        for dir in ["class", "spells"] {
            let index = read_json(src, &format!("{dir}/index.json"))?;
            if let Some(map) = index.as_object() {
                files.extend(map.values().filter_map(Value::as_str).map(|f| format!("{dir}/{f}")));
            }
        }
        Ok(files)
    }

    /// Load the data a source holds (see [`crate::source`]).
    pub fn load(src: &impl DataSource) -> Result<Library, LoadError> {
        let mut lib = Library::new();
        lib.read(src)?;
        lib.finish();
        Ok(lib)
    }

    /// The built-in SRD 5.2.1 alone.
    pub fn srd() -> Library {
        let mut lib = Library::new();
        lib.read_srd();
        lib.finish();
        lib
    }

    /// A data directory over the built-in SRD.
    pub fn load_dir_over_srd(root: &Path) -> Result<Library, LoadError> {
        let mut lib = Library::new();
        lib.read_srd();
        lib.read(&DirSource::open(root)?)?;
        lib.finish();
        Ok(lib)
    }

    fn read_srd(&mut self) {
        self.read(&crate::srd::files()).expect("the built-in SRD loads");
    }

    /// Read a source's books and entities into the library (finished by
    /// [`Library::finish`]).
    fn read(&mut self, src: &impl DataSource) -> Result<(), LoadError> {
        let files = Library::data_files(src)?;
        self.load_books(&read_json(src, "books.json")?, "book", None);
        if let Ok(adventures) = read_json(src, "adventures.json") {
            self.load_books(&adventures, "adventure", Some("adventure"));
        }
        for (path, parsed) in parse_all(src, &files) {
            match parsed {
                Ok(json) => self.ingest(&json, false),
                Err(e) => self.warnings.push(format!("{}: {e}", src.locate(&path).display())),
            }
        }
        if let Ok(lookup) = read_json(src, "spells/sources.json") {
            self.add_spell_lookup(lookup);
        }
        Ok(())
    }

    /// Spells' classes by source, added to those read before.
    fn add_spell_lookup(&mut self, lookup: Value) {
        match (&mut self.pending_spell_lookup, lookup) {
            (Some(Value::Object(have)), Value::Object(more)) => have.extend(more),
            (pending, lookup) => *pending = Some(lookup),
        }
    }

    /// Load documents already read: `books` lists books and adventures
    /// (`{"book": [...], "adventure": [...]}`), `docs` hold the entities
    /// (each may name its sources in `_meta`), and `spell_lookup` gives
    /// spells their classes (as `spells/sources.json` does).
    pub fn load_parsed(books: &Value, docs: &[Value], spell_lookup: Option<Value>) -> Library {
        let mut lib = Library::new();
        lib.read_parsed(books, docs, spell_lookup);
        lib.finish();
        lib
    }

    /// [`Library::load_parsed`] over the built-in SRD.
    pub fn load_parsed_over_srd(books: &Value, docs: &[Value], spell_lookup: Option<Value>) -> Library {
        let mut lib = Library::new();
        lib.read_srd();
        lib.read_parsed(books, docs, spell_lookup);
        lib.finish();
        lib
    }

    fn read_parsed(&mut self, books: &Value, docs: &[Value], spell_lookup: Option<Value>) {
        self.load_books(books, "book", None);
        self.load_books(books, "adventure", Some("adventure"));
        for doc in docs {
            self.ingest(doc, false);
        }
        if let Some(lookup) = spell_lookup {
            self.add_spell_lookup(lookup);
        }
    }

    /// Read and parse files of a source (in parallel where there are
    /// threads), keeping their order.
    pub fn parse_files(src: &impl DataSource, files: &[String]) -> Vec<(String, Result<Value, LoadError>)> {
        parse_all(src, files)
    }

    /// Load a campaign's homebrew. Entities marked `"_dnt": {"replaces":
    /// true}` with a `_copy` patch the entity they copy *in place* (same
    /// key), so every lookup gets the campaign version; the book data is
    /// kept for [`Library::original`]. Everything else loads as normal
    /// homebrew. Returns problems found.
    pub fn apply_campaign(&mut self, json: &Value) -> Vec<String> {
        let mut problems = Vec::new();
        let mut rest = json.clone();
        if let Some(top) = rest.as_object_mut() {
            for (key, list) in top.iter_mut() {
                let (Some(kind), Some(list)) = (Kind::from_json_key(key), list.as_array_mut()) else { continue };
                list.retain(|obj| {
                    let replaces = obj.get("_dnt").and_then(|d| d.get("replaces")).and_then(Value::as_bool) == Some(true);
                    if !replaces {
                        return true;
                    }
                    let base = obj.get("_copy").and_then(|m| Key::of_entity(kind, m)).and_then(|k| self.get(&k));
                    match base {
                        Some(id) => {
                            let original = self.originals.get(&id).cloned().unwrap_or_else(|| self.entities[id].data.clone());
                            // An override keeps everything it does not change
                            // (page, srd flags, …), unlike a plain `_copy`.
                            let mut patch = obj.clone();
                            patch["_copy"]["_preserve"]["*"] = Value::Bool(true);
                            let mut merged = copy::merge(&original, &patch);
                            if let Some(m) = merged.as_object_mut() {
                                // Keep the original identity so ids keep resolving.
                                for k in ["name", "source"].iter().chain(crate::homebrew::identity_fields(kind)) {
                                    match original.get(k) {
                                        Some(v) => m.insert(k.to_string(), v.clone()),
                                        None => m.remove(*k),
                                    };
                                }
                                if let Some(name) = obj.get("name").filter(|n| Some(*n) != original.get("name")) {
                                    m.insert("_dntName".into(), name.clone());
                                }
                            }
                            if kind == Kind::Spell && obj.get("classes").is_some() {
                                // Re-read from the data when the library finishes loading.
                                self.spell_classes.remove(&id);
                            }
                            self.originals.insert(id, original);
                            self.entities[id].data = merged;
                        }
                        None => problems.push(format!(
                            "override of a missing {}: {}",
                            kind.json_key(),
                            obj.get("_copy").map(|c| c.to_string()).unwrap_or_default()
                        )),
                    }
                    false
                });
            }
        }
        self.load_json(&rest);
        problems
    }

    /// The book version of an entity a campaign override replaced.
    pub fn original(&self, id: EntityId) -> Option<&Value> {
        self.originals.get(&id)
    }

    /// Top-level fields an override changed (empty if not overridden).
    pub fn overridden_fields(&self, id: EntityId) -> Vec<String> {
        let Some(orig) = self.originals.get(&id) else { return Vec::new() };
        let cur = &self.entities[id].data;
        let mut keys: Vec<String> = cur
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, v)| !k.starts_with('_') && orig.get(*k) != Some(*v))
            .map(|(k, _)| k.clone())
            .collect();
        keys.extend(
            orig.as_object().into_iter().flatten().filter(|(k, _)| !k.starts_with('_') && cur.get(*k).is_none()).map(|(k, _)| k.clone()),
        );
        keys.sort();
        keys
    }

    /// Load an in-memory data document (e.g. homebrew, or test data).
    pub fn load_json(&mut self, json: &Value) {
        self.ingest(json, true);
        self.finish();
    }

    /// Load several homebrew documents at once (indexing once).
    pub fn load_docs<'a>(&mut self, docs: impl IntoIterator<Item = &'a Value>) {
        for doc in docs {
            self.ingest(doc, true);
        }
        self.finish();
    }

    /// Load an additional file (e.g. homebrew) into an already-built library.
    pub fn load_file(&mut self, path: &Path) -> Result<(), LoadError> {
        let bytes = std::fs::read(path).map_err(|source| LoadError::Io { path: path.to_path_buf(), source })?;
        let json = serde_json::from_slice(&bytes).map_err(|source| LoadError::Json { path: path.to_path_buf(), source })?;
        self.ingest(&json, true);
        self.finish();
        Ok(())
    }

    fn load_books(&mut self, books: &Value, key: &str, group: Option<&str>) {
        for book in books.get(key).and_then(Value::as_array).into_iter().flatten() {
            let s = |k: &str| book.get(k).and_then(Value::as_str).map(str::to_string);
            if let (Some(id), Some(name)) = (s("source").or_else(|| s("id")), s("name")) {
                self.sources.insert(
                    id.to_lowercase(),
                    SourceInfo {
                        id,
                        name,
                        group: group.map(str::to_string).or_else(|| s("group")),
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
                        // Data sources missing from the books keep no group.
                        group: homebrew.then(|| "homebrew".into()),
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
                    let base = Key::of_entity(kind, meta).and_then(|k| self.index.get(&k).copied());
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

        self.expand_versions();
        self.expand_magic_variants();
        self.mark_srd_reprints();
        if let Some(lookup) = self.pending_spell_lookup.take() {
            self.attach_spell_classes(&lookup);
        }
        // Homebrew spells list their classes inline.
        for id in 0..self.entities.len() {
            if self.entities[id].kind != Kind::Spell || self.spell_classes.contains_key(&id) {
                continue;
            }
            let classes: Vec<ClassRef> = self.entities[id]
                .data
                .get("classes")
                .and_then(|c| c.get("fromClassList"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    Some(ClassRef {
                        name: c.get("name")?.as_str()?.to_string(),
                        source: c.get("source").and_then(Value::as_str).unwrap_or("PHB").to_string(),
                    })
                })
                .collect();
            if !classes.is_empty() {
                self.spell_classes.insert(id, classes);
            }
        }
    }

    /// Generate entities for `_versions`: either named versions with a
    /// `_mod`, or an `_abstract` template filled in by `_implementations`
    /// (`{{color}}` → `Black`, …). Each version is a copy of its base.
    fn expand_versions(&mut self) {
        let bases: Vec<EntityId> = (0..self.entities.len())
            .filter(|i| self.entities[*i].data.get("_versions").is_some() && !self.versions.contains_key(i))
            .collect();
        for base in bases {
            let kind = self.entities[base].kind;
            let base_data = self.entities[base].data.clone();
            let mut variants = Vec::new();
            for v in base_data.get("_versions").and_then(Value::as_array).into_iter().flatten() {
                match (v.get("_abstract"), v.get("_implementations").and_then(Value::as_array)) {
                    (Some(abs), Some(impls)) => {
                        for imp in impls {
                            let mut text = abs.to_string();
                            for (var, val) in imp.get("_variables").and_then(Value::as_object).into_iter().flatten() {
                                let val = match val {
                                    Value::String(s) => serde_json::to_string(s).unwrap_or_default().trim_matches('"').to_string(),
                                    other => other.to_string(),
                                };
                                text = text.replace(&format!("{{{{{var}}}}}"), &val);
                            }
                            let Ok(mut filled) = serde_json::from_str::<Value>(&text) else { continue };
                            if let (Some(f), Some(i)) = (filled.as_object_mut(), imp.as_object()) {
                                for (k, val) in i.iter().filter(|(k, _)| *k != "_variables") {
                                    f.insert(k.clone(), val.clone());
                                }
                            }
                            variants.push(filled);
                        }
                    }
                    _ => variants.push(v.clone()),
                }
            }
            let mut ids = Vec::new();
            for variant in variants {
                let Some(obj) = variant.as_object() else { continue };
                let mut copy: serde_json::Map<String, Value> =
                    obj.iter().filter(|(k, _)| *k != "_mod").map(|(k, v)| (k.clone(), v.clone())).collect();
                let mut meta = serde_json::Map::new();
                if let Some(m) = obj.get("_mod") {
                    meta.insert("_mod".into(), m.clone());
                }
                copy.insert("_copy".into(), Value::Object(meta));
                let mut merged = copy::merge(&base_data, &Value::Object(copy));
                if let Some(o) = merged.as_object_mut() {
                    o.remove("_versions");
                    o.insert("_versionOf".into(), Value::String(self.entities[base].name.clone()));
                }
                let before = self.entities.len();
                self.insert(kind, merged);
                if self.entities.len() > before {
                    ids.push(before);
                    self.version_base.insert(before, base);
                }
            }
            self.versions.insert(base, ids);
        }
    }

    /// Versions generated from an entity's `_versions` (empty if none).
    pub fn versions_of(&self, id: EntityId) -> &[EntityId] {
        self.versions.get(&id).map(Vec::as_slice).unwrap_or_default()
    }

    /// The entity a version was generated from.
    pub fn version_base(&self, id: EntityId) -> Option<EntityId> {
        self.version_base.get(&id).copied()
    }

    /// Turn generic magic variants ("+1 Weapon", "Flame Tongue") into
    /// specific items for every base item they apply to ("+1 Longsword"),
    /// as the format intends. Existing items with the same name win.
    fn expand_magic_variants(&mut self) {
        let variants: Vec<EntityId> = (0..self.entities.len()).filter(|&i| self.entities[i].kind == Kind::MagicVariant).collect();
        let bases: Vec<EntityId> = (0..self.entities.len()).filter(|&i| self.entities[i].kind == Kind::BaseItem).collect();
        for &v in &variants {
            for &b in &bases {
                if self.generated_variants.contains(&(v, b)) {
                    continue;
                }
                self.generated_variants.insert((v, b));
                let (vd, bd) = (&self.entities[v].data, &self.entities[b].data);
                if !variant_applies(vd, bd) {
                    continue;
                }
                let Some(item) = specific_variant(vd, bd, &self.entities[v].name) else { continue };
                if Key::of_entity(Kind::Item, &item).is_some_and(|k| self.index.contains_key(&k)) {
                    continue;
                }
                self.insert(Kind::Item, item);
            }
        }
    }

    /// A book's entry that is in the SRD says so (`"srd52": true`, or the
    /// SRD's name for it): the built-in SRD's entry is then reprinted as
    /// the book's, which shows instead where the book is enabled.
    fn mark_srd_reprints(&mut self) {
        let srd = crate::srd::SOURCE;
        let mut found: Vec<(EntityId, String)> = Vec::new();
        for e in &self.entities {
            if e.source.eq_ignore_ascii_case(srd) {
                continue;
            }
            let srd_name = match e.data.get("srd52") {
                Some(Value::Bool(true)) => e.name.as_str(),
                Some(Value::String(name)) => name.as_str(),
                _ => continue,
            };
            let key = Key::new(e.kind, &[srd_name, srd]);
            if let Some(&id) = self.index.get(&key) {
                found.push((id, format!("{}|{}", e.name, e.source)));
            }
        }
        for (id, reprint) in found {
            if let Some(obj) = self.entities[id].data.as_object_mut() {
                let list = obj.entry("reprintedAs").or_insert_with(|| Value::Array(Vec::new()));
                if let Some(list) = list.as_array_mut() {
                    list.push(Value::String(reprint));
                }
            }
        }
    }

    fn insert(&mut self, kind: Kind, data: Value) {
        let (Some(key), Some(name)) = (Key::of_entity(kind, &data), display_name(kind, &data)) else {
            return;
        };
        let source = entity_source(&data).unwrap_or_default().to_string();
        let entity = Entity { kind, key: key.clone(), name, source, data };
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
                        Some(ClassRef { name: c.get("name")?.as_str()?.to_string(), source: c.get("source")?.as_str()?.to_string() })
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

    /// Number of entities per source id (as written in the data).
    pub fn entity_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for e in &self.entities {
            *counts.entry(e.source.clone()).or_default() += 1;
        }
        counts
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
        Kind::for_tag(&link.tag).iter().filter_map(|&kind| Key::from_ref(kind, &link.tag, &parts)).find_map(|(key, _)| self.get(&key))
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
        SourceFilter { enabled: Some(sources.into_iter().map(|s| s.as_ref().to_lowercase()).collect()), hide_reprinted: true }
    }

    pub fn source_enabled(&self, source: &str) -> bool {
        self.enabled.as_ref().is_none_or(|e| e.contains(&source.to_lowercase()))
    }

    pub fn allows(&self, entity: &Entity) -> bool {
        if !self.source_enabled(&entity.source) {
            return false;
        }
        if self.hide_reprinted {
            // Magic variants say it in what their items inherit.
            let reprinted = entity
                .data
                .get("reprintedAs")
                .or_else(|| entity.data.get("inherits").and_then(|i| i.get("reprintedAs")))
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

fn read_json(src: &impl DataSource, path: &str) -> Result<Value, LoadError> {
    let bytes = src.read(path).map_err(|source| LoadError::Io { path: src.locate(path), source })?;
    serde_json::from_slice(&bytes).map_err(|source| LoadError::Json { path: src.locate(path), source })
}

/// Read and parse files, on all cores where there are threads, keeping
/// their order.
fn parse_all(src: &impl DataSource, files: &[String]) -> Vec<(String, Result<Value, LoadError>)> {
    par_map(files, |p| (p.clone(), read_json(src, p)))
}

/// `f` over every item, on as many threads as there are cores (one
/// thread on the web), keeping the order.
#[cfg(not(target_arch = "wasm32"))]
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(items.len().max(1));
    let chunk = items.len().div_ceil(threads).max(1);
    let f = &f;
    std::thread::scope(|scope| {
        let handles: Vec<_> = items.chunks(chunk).map(|chunk| scope.spawn(move || chunk.iter().map(f).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().expect("worker thread panicked")).collect()
    })
}

#[cfg(target_arch = "wasm32")]
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    items.iter().map(f).collect()
}

/// Whether a base item matches a variant's `requires` (any of them, all
/// of its fields) and none of its `excludes`.
fn variant_applies(variant: &Value, base: &Value) -> bool {
    let matches = |field: &str, want: &Value| match (base.get(field), want) {
        (Some(Value::Array(have)), Value::String(w)) => have.iter().any(|h| {
            let h = h.as_str().or_else(|| h.get("uid").and_then(Value::as_str)).unwrap_or("");
            h.eq_ignore_ascii_case(w) || (!w.contains('|') && h.split('|').next().is_some_and(|p| p.eq_ignore_ascii_case(w)))
        }),
        (Some(Value::String(h)), Value::String(w)) => h.eq_ignore_ascii_case(w),
        (Some(h), w) => h == w,
        (None, Value::Bool(false)) => true,
        (None, _) => false,
    };
    let requires = variant.get("requires").and_then(Value::as_array);
    let required = requires.is_some_and(|r| r.iter().any(|req| req.as_object().is_some_and(|o| o.iter().all(|(k, v)| matches(k, v)))));
    let excluded = variant.get("excludes").and_then(Value::as_object).is_some_and(|ex| {
        ex.iter().any(|(k, v)| match v {
            Value::Array(options) => options.iter().any(|o| matches(k, o)),
            v => matches(k, v),
        })
    });
    required && !excluded
}

/// A specific item from a variant and a base item: the base's fields with
/// the variant's `inherits` on top, named `<prefix><base><suffix>`.
fn specific_variant(variant: &Value, base: &Value, variant_name: &str) -> Option<Value> {
    let inherits = variant.get("inherits")?.as_object()?;
    let base_name = base.get("name")?.as_str()?;
    let base_source = base.get("source")?.as_str()?;
    let mut out = base.as_object()?.clone();
    for k in [
        "srd",
        "srd52",
        "basicRules",
        "basicRules2024",
        "page",
        "reprintedAs",
        "hasFluff",
        "hasFluffImages",
        "value",
        "entries",
        "otherSources",
        "referenceSources",
    ] {
        out.remove(k);
    }
    let mut name = base_name.to_string();
    if let Some(remove) = inherits.get("nameRemove").and_then(Value::as_str) {
        name = name.replace(remove, "");
    }
    let prefix = inherits.get("namePrefix").and_then(Value::as_str).unwrap_or("");
    let suffix = inherits.get("nameSuffix").and_then(Value::as_str).unwrap_or("");
    for (k, v) in inherits {
        if !matches!(k.as_str(), "namePrefix" | "nameSuffix" | "nameRemove") {
            out.insert(k.clone(), v.clone());
        }
    }
    out.insert("name".into(), Value::String(format!("{prefix}{}{suffix}", name.trim())));
    out.insert("baseItem".into(), Value::String(format!("{base_name}|{base_source}")));
    let variant_source =
        variant.get("source").and_then(Value::as_str).or_else(|| inherits.get("source").and_then(Value::as_str)).unwrap_or("");
    out.insert("_variant".into(), Value::String(format!("{variant_name}|{variant_source}")));
    // `{=bonusWeapon}`, `{=dmgType}` in the text become the item's values.
    let fields = out.clone();
    if let Some(entries) = out.get_mut("entries") {
        fill_templates(entries, &fields);
    }
    Some(Value::Object(out))
}

fn fill_templates(v: &mut Value, fields: &serde_json::Map<String, Value>) {
    match v {
        Value::String(s) if s.contains("{=") => {
            let mut out = String::new();
            let mut rest = s.as_str();
            while let Some(start) = rest.find("{=") {
                out.push_str(&rest[..start]);
                let Some(end) = rest[start..].find('}') else { break };
                let key = &rest[start + 2..start + end];
                let value = match (key, fields.get(key)) {
                    ("dmgType", Some(Value::String(t))) => crate::view::damage_type(t).to_string(),
                    (_, Some(Value::String(t))) => t.clone(),
                    (_, Some(other)) => other.to_string(),
                    (_, None) => String::new(),
                };
                out.push_str(&value);
                rest = &rest[start + end + 1..];
            }
            out.push_str(rest);
            *s = out;
        }
        Value::Array(a) => a.iter_mut().for_each(|x| fill_templates(x, fields)),
        Value::Object(o) => o.values_mut().for_each(|x| fill_templates(x, fields)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reprints_hide_their_originals() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "spell": [{"name": "Fireball", "source": "PHB", "reprintedAs": ["Fireball|XPHB"]}, {"name": "Fireball", "source": "XPHB"}],
            "magicvariant": [
                {"name": "+1 Armor", "inherits": {"source": "DMG", "reprintedAs": ["+1 Armor|XDMG"]}},
                {"name": "+1 Armor", "inherits": {"source": "XDMG"}}
            ]
        }));
        let filter = SourceFilter { enabled: None, hide_reprinted: true };
        let mut shown: Vec<String> =
            lib.entities().iter().filter(|e| filter.allows(e)).map(|e| format!("{} {}", e.name, e.source)).collect();
        shown.sort();
        assert_eq!(shown, ["+1 Armor XDMG", "Fireball XPHB"]);
    }

    #[test]
    fn campaign_overrides_patch_in_place() {
        let mut lib = Library::new();
        lib.load_json(&json!({"spell": [{"name": "Hunter's Mark", "source": "XPHB", "level": 1, "page": 289,
            "duration": [{"type": "timed", "duration": {"type": "hour", "amount": 1}, "concentration": true}],
            "entries": ["Mark a target."]}]}));
        let id = lib.get(&Key::new(Kind::Spell, &["Hunter's Mark", "XPHB"])).unwrap();
        let problems = lib.apply_campaign(&json!({
            "_meta": {"sources": [{"json": "CoS", "full": "Curse of Strahd"}]},
            "spell": [
                {"name": "Hunter's Mark", "source": "CoS", "_copy": {"name": "Hunter's Mark", "source": "XPHB"},
                 "_dnt": {"replaces": true}, "level": 2,
                 "classes": {"fromClassList": [{"name": "Druid", "source": "XPHB"}]},
                 "duration": [{"type": "timed", "duration": {"type": "minute", "amount": 10}, "concentration": true}]},
                {"name": "Blood Spike", "source": "CoS", "level": 2, "entries": ["Ouch."],
                 "classes": {"fromClassList": [{"name": "Wizard", "source": "XPHB"}]}},
                {"name": "Nope", "source": "CoS", "_copy": {"name": "Missing", "source": "XPHB"}, "_dnt": {"replaces": true}}
            ]
        }));
        assert_eq!(problems.len(), 1);
        // Same id and key; campaign data.
        let e = lib.entity(id);
        assert_eq!((e.name.as_str(), e.source.as_str()), ("Hunter's Mark", "XPHB"));
        assert_eq!(e.data["duration"][0]["duration"]["type"], "minute");
        assert_eq!(e.data["entries"][0], "Mark a target.", "unchanged fields come from the book");
        assert_eq!(lib.original(id).unwrap()["duration"][0]["duration"]["type"], "hour");
        assert_eq!(e.data["level"], 2, "a spell's level is not part of its identity");
        assert_eq!(lib.overridden_fields(id), vec!["classes", "duration", "level"]);
        assert_eq!(lib.spell_classes(id)[0].name, "Druid");
        assert_eq!(e.data["page"], 289, "book metadata is kept");
        let spike = lib.get(&Key::new(Kind::Spell, &["Blood Spike", "CoS"])).unwrap();
        assert_eq!(lib.spell_classes(spike)[0].name, "Wizard");
        assert_eq!(lib.source_info("cos").unwrap().name, "Curse of Strahd");
    }

    #[test]
    fn versions_named_and_templated() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "race": [
                {"name": "Elf", "source": "T", "speed": 30, "entries": [{"type": "entries", "name": "Elven Lineage", "entries": ["Choose."]}],
                 "_versions": [{"name": "Elf; Drow Lineage", "source": "T", "darkvision": 120,
                    "_mod": {"entries": {"mode": "replaceArr", "replace": "Elven Lineage",
                        "items": {"type": "entries", "name": "Elven Lineage (Drow)", "entries": ["Drow."]}}}}]},
                {"name": "Dragonborn", "source": "T", "entries": ["Breath: {{x}}"],
                 "_versions": [{"_abstract": {"name": "Dragonborn ({{color}})", "source": "T",
                    "_mod": {"entries": {"mode": "appendArr", "items": "Resist {{damageType}}."}}},
                  "_implementations": [{"_variables": {"color": "Red", "damageType": "Fire"}, "resist": ["fire"]}]}]}
            ],
            "feat": [{"name": "Magic Initiate", "source": "T", "entries": ["Pick a list."],
                "_versions": [{"name": "Magic Initiate; Cleric", "source": "T"}]}]
        }));
        let get = |kind, name: &str| lib.get(&Key::new(kind, &[name, "T"])).unwrap_or_else(|| panic!("{name} missing"));
        let elf = get(Kind::Race, "Elf");
        let drow = get(Kind::Race, "Elf; Drow Lineage");
        assert_eq!(lib.versions_of(elf), &[drow]);
        assert_eq!(lib.version_base(drow), Some(elf));
        assert_eq!(lib.entity(drow).data["darkvision"], 120);
        assert_eq!(lib.entity(drow).data["speed"], 30, "inherits from the base");
        assert_eq!(lib.entity(drow).data["entries"][0]["name"], "Elven Lineage (Drow)");
        assert!(lib.entity(drow).data.get("_versions").is_none());

        let red = get(Kind::Race, "Dragonborn (Red)");
        assert_eq!(lib.entity(red).data["entries"][1], "Resist Fire.");
        assert_eq!(lib.entity(red).data["resist"], json!(["fire"]));

        get(Kind::Feat, "Magic Initiate; Cleric");
    }

    #[test]
    fn a_books_srd_entry_hides_the_built_in_one() {
        let books = json!({"book": [{"name": "Book", "id": "BK", "source": "BK"}]});
        let docs = [json!({"spell": [{"name": "Fireball", "source": "BK", "srd52": true, "level": 3, "entries": ["x"]}],
                           "feat": [{"name": "Lucky Break", "source": "BK", "srd52": "Alert", "entries": ["x"]}]})];
        let lib = Library::load_parsed_over_srd(&books, &docs, None);
        let shown = |filter: &SourceFilter, kind: Kind, name: &str| {
            lib.entities()
                .iter()
                .filter(|e| e.kind == kind && e.name == name && filter.allows(e))
                .map(|e| e.source.clone())
                .collect::<Vec<_>>()
        };
        let both = SourceFilter { enabled: None, hide_reprinted: true };
        assert_eq!(shown(&both, Kind::Spell, "Fireball"), ["BK"]);
        assert!(shown(&both, Kind::Feat, "Alert").is_empty());
        // Without the book, the SRD's is there.
        let srd = SourceFilter::only(["SRD52"]);
        assert_eq!(shown(&srd, Kind::Spell, "Fireball"), ["SRD52"]);
    }

    #[test]
    fn magic_variants_become_items() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "baseitem": [
                {"name": "Longsword", "source": "XPHB", "type": "M|XPHB", "weapon": true, "sword": true, "weaponCategory": "martial",
                 "dmg1": "1d8", "dmgType": "S", "property": ["V|XPHB"], "value": 1500, "weight": 3},
                {"name": "Net", "source": "XPHB", "type": "R|XPHB", "weapon": true, "net": true, "weaponCategory": "martial"},
                {"name": "Chain Mail", "source": "XPHB", "type": "HA|XPHB", "armor": true, "ac": 16}
            ],
            "magicvariant": [
                {"name": "+1 Weapon", "requires": [{"weapon": true}], "excludes": {"net": true},
                 "inherits": {"namePrefix": "+1 ", "source": "XDMG", "rarity": "uncommon", "bonusWeapon": "+1",
                              "entries": ["A {=bonusWeapon} bonus with this {=dmgType} weapon."]}},
                {"name": "Flame Tongue", "requires": [{"sword": true}],
                 "inherits": {"namePrefix": "Flame Tongue ", "source": "XDMG", "rarity": "rare", "reqAttune": true}},
                {"name": "+2 Armor", "requires": [{"type": "LA|XPHB"}, {"type": "HA|XPHB"}],
                 "inherits": {"namePrefix": "+2 ", "source": "XDMG", "rarity": "very rare", "bonusAc": "+2"}}
            ],
            "item": [{"name": "Flame Tongue Longsword", "source": "XDMG", "rarity": "rare", "entries": ["The real one."]}]
        }));
        let get = |n: &str| lib.get(&Key::new(Kind::Item, &[n, "XDMG"])).map(|id| &lib.entity(id).data);
        let sword = get("+1 Longsword").expect("generated");
        assert_eq!(sword["bonusWeapon"], "+1");
        assert_eq!(sword["dmg1"], "1d8", "from the base");
        assert_eq!(sword["baseItem"], "Longsword|XPHB");
        assert_eq!(sword["entries"][0], "A +1 bonus with this slashing weapon.");
        assert!(sword.get("value").is_none());
        assert!(get("+1 Net").is_none(), "excluded");
        assert_eq!(get("+2 Chain Mail").unwrap()["bonusAc"], "+2");
        assert!(get("+2 Longsword").is_none());
        // A real item of the same name wins.
        assert_eq!(get("Flame Tongue Longsword").unwrap()["entries"][0], "The real one.");
        // Loading more data does not duplicate anything.
        let n = lib.entities().len();
        lib.load_json(&json!({"spell": []}));
        assert_eq!(lib.entities().len(), n);
    }
}
