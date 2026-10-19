//! Where data files come from: a folder on disk, or bytes already in
//! memory (a browser fetches the files first, see [`index_files`] and
//! [`crate::Library::data_files`]). Paths are relative to the data folder,
//! with `/` separators: `books.json`, `spells/index.json`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::library::LoadError;

pub trait DataSource: Sync {
    /// The bytes of a file.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;

    /// Where a file is, for messages.
    fn locate(&self, path: &str) -> PathBuf {
        PathBuf::from(path)
    }
}

/// Files to read before the rest: they list the other files (see
/// [`crate::Library::data_files`]). `adventures.json` and
/// `spells/sources.json` may be missing.
pub fn index_files() -> &'static [&'static str] {
    &["books.json", "adventures.json", "class/index.json", "spells/index.json", "spells/sources.json"]
}

/// A data folder (it, or its `data/` subfolder).
pub struct DirSource {
    root: PathBuf,
}

impl DirSource {
    pub fn open(root: &Path) -> Result<DirSource, LoadError> {
        if root.join("data/books.json").is_file() {
            Ok(DirSource { root: root.join("data") })
        } else if root.join("books.json").is_file() {
            Ok(DirSource { root: root.to_path_buf() })
        } else {
            Err(LoadError::NotADataDir(root.to_path_buf()))
        }
    }
}

impl DataSource for DirSource {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.locate(path))
    }

    fn locate(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

/// Files held in memory, by path.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl MemorySource {
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.into(), bytes.into());
    }
}

impl DataSource for MemorySource {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.files.get(path).cloned().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{path} is not loaded")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, Library};
    use serde_json::json;

    #[test]
    fn a_library_loads_from_memory() {
        let mut src = MemorySource::default();
        let mut put = |path: &str, v: serde_json::Value| src.insert(path, v.to_string());
        put("books.json", json!({"book": [{"name": "Test Book", "id": "T", "source": "T"}]}));
        put("class/index.json", json!({"fighter": "class-fighter.json"}));
        put("spells/index.json", json!({"T": "spells-t.json"}));
        put("class/class-fighter.json", json!({"class": [{"name": "Fighter", "source": "T"}]}));
        put("spells/spells-t.json", json!({"spell": [{"name": "Bless", "source": "T", "level": 1}]}));
        put("items.json", json!({"item": [{"name": "Rope", "source": "T"}]}));

        let files = Library::data_files(&src).unwrap();
        assert!(files.contains(&"class/class-fighter.json".to_string()) && files.contains(&"spells/spells-t.json".to_string()));
        let lib = Library::load(&src).unwrap();
        let kinds: Vec<Kind> = lib.entities().iter().map(|e| e.kind).collect();
        assert!([Kind::Class, Kind::Spell, Kind::Item].iter().all(|k| kinds.contains(k)), "{kinds:?}");
        assert!(lib.warnings.iter().any(|w| w.contains("races.json")), "missing files are reported");
        assert!(Library::load(&MemorySource::default()).is_err(), "no indexes");
    }
}
