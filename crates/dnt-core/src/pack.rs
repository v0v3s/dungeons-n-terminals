//! Packing data for the import: from a data folder (one with
//! `books.json`, or a repository with `data/books.json`) and a homebrew
//! collection (a folder of homebrew files in any layout), only what dnt
//! reads, in a zip [`crate::import::import_zip`] takes:
//!
//! ```text
//! data/…       the index files, and the data files they list
//! homebrew/…   each homebrew file with entities dnt knows, as laid out
//! ```
//!
//! Entity files keep only `_meta` and the kinds dnt knows, so creatures,
//! adventure text, fluff and images are left behind.

use std::io::{Seek, Write};
use std::path::Path;

use dnt_data::source::{index_files, DataSource, DirSource};
use dnt_data::{Kind, Library};
use serde_json::{Map, Value};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::import::{hidden, ignored};

/// What a pack holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Packed {
    pub data_files: usize,
    pub homebrew_files: usize,
    /// Homebrew files with nothing dnt reads.
    pub skipped: usize,
}

/// Pack a data folder and/or a homebrew collection into `out`.
pub fn pack<W: Write + Seek>(data: Option<&Path>, homebrew: Option<&Path>, out: W) -> Result<Packed, String> {
    let mut zip = ZipWriter::new(out);
    let mut packed = Packed::default();
    if let Some(dir) = data {
        let src = DirSource::open(dir).map_err(|e| e.to_string())?;
        for path in index_files() {
            if let Ok(bytes) = src.read(path) {
                add(&mut zip, &format!("data/{path}"), &bytes)?;
                packed.data_files += 1;
            }
        }
        for path in Library::data_files(&src).map_err(|e| e.to_string())? {
            // A data folder may lack some files.
            let Ok(bytes) = src.read(&path) else { continue };
            let doc = parse(&bytes).map_err(|e| format!("{}: {e}", src.locate(&path).display()))?;
            add(&mut zip, &format!("data/{path}"), &usable(doc).to_string().into_bytes())?;
            packed.data_files += 1;
        }
    }
    if let Some(dir) = homebrew {
        let mut files = Vec::new();
        walk(dir, dir, &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
        files.sort();
        for rel in files {
            if hidden(&rel) || !rel.ends_with(".json") || ignored(&rel) {
                continue;
            }
            let bytes = std::fs::read(dir.join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
            // Broken or foreign files are not ours to fix.
            let Ok(doc) = parse(&bytes) else {
                packed.skipped += 1;
                continue;
            };
            let doc = usable(doc);
            if doc.as_object().is_none_or(|o| !o.keys().any(|k| Kind::from_json_key(k).is_some())) {
                packed.skipped += 1;
                continue;
            }
            add(&mut zip, &format!("homebrew/{rel}"), &doc.to_string().into_bytes())?;
            packed.homebrew_files += 1;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(packed)
}

fn parse(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// A document with only `_meta` and the kinds dnt knows.
fn usable(doc: Value) -> Value {
    match doc {
        Value::Object(top) => {
            let kept: Map<String, Value> =
                top.into_iter().filter(|(k, v)| k == "_meta" || (Kind::from_json_key(k).is_some() && v.is_array())).collect();
            Value::Object(kept)
        }
        other => other,
    }
}

fn add<W: Write + Seek>(zip: &mut ZipWriter<W>, name: &str, bytes: &[u8]) -> Result<(), String> {
    zip.start_file(name, SimpleFileOptions::default()).map_err(|e| e.to_string())?;
    zip.write_all(bytes).map_err(|e| e.to_string())
}

/// Every file below `dir`, by its path from `root` (with `/`).
fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        if entry.file_type()?.is_dir() {
            // Skip tooling folders (and their images) without reading them.
            if !hidden(&format!("{rel}/x")) {
                walk(root, &path, files)?;
            }
        } else {
            files.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import;
    use dnt_store::fs::MemFs;
    use dnt_store::Store;
    use serde_json::json;
    use std::io::Cursor;
    use std::sync::Arc;

    fn write(root: &Path, path: &str, v: Value) {
        let p = root.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, v.to_string()).unwrap();
    }

    #[test]
    fn packs_what_the_import_reads() {
        let tmp = std::env::temp_dir().join(format!("dnt-pack-{}", std::process::id()));
        let (data, brew) = (tmp.join("repo"), tmp.join("brew"));
        write(&data, "data/books.json", json!({"book": [{"name": "Test Book", "id": "TB", "source": "TB"}]}));
        write(&data, "data/class/index.json", json!({"fighter": "class-fighter.json"}));
        write(&data, "data/class/class-fighter.json", json!({"class": [{"name": "Fighter", "source": "TB"}], "classFluff": []}));
        write(&data, "data/spells/index.json", json!({"TB": "spells-tb.json"}));
        write(&data, "data/spells/spells-tb.json", json!({"spell": [{"name": "Bless", "source": "TB"}]}));
        write(&data, "data/bestiary/bestiary-tb.json", json!({"monster": [{"name": "Goblin", "source": "TB"}]}));
        let meta = json!({"sources": [{"json": "WRD", "full": "The Warden"}]});
        write(&brew, "class/Someone; Warden.json", json!({"_meta": meta, "class": [{"name": "Warden", "source": "WRD"}], "monster": []}));
        write(&brew, "creature/Someone; Beasts.json", json!({"_meta": meta, "monster": [{"name": "Wolf", "source": "WRD"}]}));
        write(&brew, "_generated/index-meta.json", json!({"class": []}));
        write(&brew, "package.json", json!({"name": "brew"}));

        let mut out = Cursor::new(Vec::new());
        let packed = pack(Some(&data), Some(&brew), &mut out).unwrap();
        std::fs::remove_dir_all(&tmp).unwrap();
        assert_eq!(packed, Packed { data_files: 5, homebrew_files: 1, skipped: 2 });

        let bytes = out.into_inner();
        let zip = zip::ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
        let mut names: Vec<String> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "data/books.json",
                "data/class/class-fighter.json",
                "data/class/index.json",
                "data/spells/index.json",
                "data/spells/spells-tb.json",
                "homebrew/class/Someone; Warden.json"
            ]
        );

        let store = Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml");
        let report = import::import_zip(&store, bytes).unwrap();
        assert_eq!(report.added, 3, "{report:?}");
        // (The test folder lacks most data files; the import says so.)
        assert!(!report.skipped.iter().any(|(f, _)| f.starts_with("homebrew/")), "{:?}", report.skipped);
        assert_eq!(import::homebrew(&store).len(), 1);
    }
}
