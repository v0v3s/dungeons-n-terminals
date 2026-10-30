//! The rules library: rules for features shared in a repository
//! (github.com/v0v3s/dnt-rules), browsed as a tree (a folder, its files,
//! their rules) and imported as the user's own rules.

use std::collections::BTreeSet;
use std::io::{Cursor, Read};
use std::path::Path;

use dnt_data::Key;
use dnt_rules::builtin;
use dnt_rules::overlay::{blocks, Overlay};
use dnt_store::Store;
use serde::Serialize;

/// Where the library is: the repository's main branch, as a zip.
pub const DEFAULT_URL: &str = "https://codeload.github.com/v0v3s/dnt-rules/zip/refs/heads/main";

/// The library, as a tree.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RulesLibrary {
    pub folders: Vec<Folder>,
    /// Files or blocks that could not be read, as `path: what`.
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Folder {
    /// `srd`
    pub name: String,
    /// `SRD`
    pub label: String,
    pub files: Vec<File>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct File {
    /// `srd/classes.toml`
    pub path: String,
    /// `Classes`
    pub label: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rule {
    /// `classFeature:Roving|Ranger|SRD52|6`
    pub id: String,
    /// `Roving (Ranger 6)`
    pub label: String,
    /// Its `[[feature]]` block.
    pub text: String,
    pub status: Status,
    /// dnt has rules of its own for the feature (these replace them).
    pub builtin: bool,
}

/// A library rule against the user's own rules for the same feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The user has none.
    New,
    /// The user has these.
    Same,
    /// The user has other rules for it: importing asks.
    Differs,
}

/// The library's files (`srd/classes.toml` → its text), from a zip of the
/// repository (its top folder left out) or a folder.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("Not a zip: {e}"))?;
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map_err(|e| e.to_string())?.to_string();
        // `dnt-rules-main/srd/classes.toml` → `srd/classes.toml`.
        let Some((_, path)) = name.split_once('/') else { continue };
        if f.is_dir() || !is_rules_file(path) {
            continue;
        }
        let mut text = String::new();
        f.read_to_string(&mut text).map_err(|e| format!("{path}: {e}"))?;
        out.push((path.to_string(), text));
    }
    Ok(out)
}

/// As [`read_zip`], from a folder (a checkout of the repository).
pub fn read_dir(root: &Path) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let mut folders: Vec<_> = std::fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?.flatten().collect();
    folders.sort_by_key(|e| e.file_name());
    for folder in folders.into_iter().filter(|e| e.path().is_dir()) {
        let mut files: Vec<_> = std::fs::read_dir(folder.path()).into_iter().flatten().flatten().collect();
        files.sort_by_key(|e| e.file_name());
        for f in files {
            let path = format!("{}/{}", folder.file_name().to_string_lossy(), f.file_name().to_string_lossy());
            if is_rules_file(&path) {
                let text = std::fs::read_to_string(f.path()).map_err(|e| format!("{path}: {e}"))?;
                out.push((path, text));
            }
        }
    }
    Ok(out)
}

/// A rules file: `folder/name.toml`, not the repository's own tooling.
fn is_rules_file(path: &str) -> bool {
    let mut parts = path.split('/');
    let (Some(folder), Some(name), None) = (parts.next(), parts.next(), parts.next()) else { return false };
    !folder.starts_with('.') && name.ends_with(".toml")
}

/// The library's files from where it is: a web address (a zip), a zip
/// file or a folder.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch(location: &str) -> Result<Vec<(String, String)>, String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        let mut response = ureq::get(location).call().map_err(|e| format!("Could not reach the rules library: {e}"))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(20 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| format!("Could not read the rules library: {e}"))?;
        return read_zip(&bytes);
    }
    let path = Path::new(location);
    if path.is_dir() {
        read_dir(path)
    } else {
        read_zip(&std::fs::read(path).map_err(|e| format!("{location}: {e}"))?)
    }
}

/// Where to read the library: `DNT_RULES_URL` (a web address, a zip or a
/// folder), else the repository.
pub fn location() -> String {
    std::env::var("DNT_RULES_URL").ok().filter(|l| !l.is_empty()).unwrap_or_else(|| DEFAULT_URL.to_string())
}

/// What a rule's status says, for a list.
pub fn status_text(rule: &Rule) -> String {
    let mut s = match rule.status {
        Status::New => "new",
        Status::Same => "yours already",
        Status::Differs => "differs from yours",
    }
    .to_string();
    if rule.builtin {
        s.push_str(" · replaces dnt's own");
    }
    s
}

/// The library as a tree, each rule against the user's own.
pub fn tree(files: &[(String, String)], store: &Store) -> RulesLibrary {
    let yours = user_blocks(store);
    let (builtins, _) = builtin::overlays();
    let mut lib = RulesLibrary::default();
    for (path, text) in files {
        let (folder, name) = path.split_once('/').unwrap_or(("", path));
        let mut file = File { path: path.clone(), label: label_of(name.trim_end_matches(".toml")), rules: Vec::new() };
        for block in blocks(text) {
            let (id, keys) = match Overlay::parse(&block) {
                Ok((o, _)) if o.features.len() == 1 => (o.features[0].id.clone(), o.features[0].keys.clone()),
                Ok(_) => continue,
                Err(e) => {
                    lib.problems.push(format!("{path}: {}", e.lines().next().unwrap_or_default()));
                    continue;
                }
            };
            let status = match yours.iter().find(|b| b.keys.iter().any(|k| keys.contains(k))) {
                None => Status::New,
                Some(b) if same(&b.text, &block) => Status::Same,
                Some(_) => Status::Differs,
            };
            let builtin = keys.iter().any(|k| !builtins.for_entity(k).is_empty());
            file.rules.push(Rule { label: rule_label(&id), id, text: block, status, builtin });
        }
        if file.rules.is_empty() {
            continue;
        }
        match lib.folders.iter_mut().find(|f| f.name == folder) {
            Some(f) => f.files.push(file),
            None => lib.folders.push(Folder { name: folder.to_string(), label: folder_label(folder), files: vec![file] }),
        }
    }
    // The SRD first, then the rest by name.
    lib.folders.sort_by_key(|f| (f.name != "srd", f.label.to_lowercase()));
    lib
}

/// What an import did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Imported {
    /// Rules the user had none of.
    pub added: usize,
    /// The user's rules replaced by the library's.
    pub overwritten: usize,
    /// The user's rules kept (and the library's left out).
    pub kept: usize,
    /// Already the user's.
    pub same: usize,
}

impl Imported {
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("Imported {} rule(s)", self.added + self.overwritten)];
        if self.overwritten > 0 {
            parts.push(format!("{} of yours overwritten", self.overwritten));
        }
        if self.kept > 0 {
            parts.push(format!("{} of yours kept", self.kept));
        }
        if self.same > 0 {
            parts.push(format!("{} you had already", self.same));
        }
        parts.join(", ")
    }
}

/// Import these rules (by id) as the user's: a rule that differs from the
/// user's own replaces it only when its id is in `overwrite`.
pub fn import(store: &Store, lib: &RulesLibrary, ids: &BTreeSet<String>, overwrite: &BTreeSet<String>) -> Result<Imported, String> {
    let mut out = Imported::default();
    let rules = lib.folders.iter().flat_map(|f| &f.files).flat_map(|f| &f.rules).filter(|r| ids.contains(&r.id));
    for rule in rules {
        match rule.status {
            Status::Same => out.same += 1,
            Status::Differs if !overwrite.contains(&rule.id) => out.kept += 1,
            status => {
                if status == Status::Differs {
                    remove_users(store, &rule.text)?;
                    out.overwritten += 1;
                } else {
                    out.added += 1;
                }
                store.save_feature_overlay(&rule.id, &rule.text).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(out)
}

/// One of the user's `[[feature]]` blocks, where it is.
struct UserBlock {
    path: std::path::PathBuf,
    text: String,
    keys: Vec<Key>,
}

fn user_blocks(store: &Store) -> Vec<UserBlock> {
    let mut out = Vec::new();
    for path in store.overlay_files() {
        let Ok(text) = store.read_text(&path) else { continue };
        for block in blocks(&text) {
            if let Ok((o, _)) = Overlay::parse(&block) {
                let keys = o.features.iter().flat_map(|f| f.keys.clone()).collect();
                out.push(UserBlock { path: path.clone(), text: block, keys });
            }
        }
    }
    out
}

/// Take the user's rules for a feature out of the files they are in (a
/// file of several features keeps the others).
fn remove_users(store: &Store, library_block: &str) -> Result<(), String> {
    let Ok((o, _)) = Overlay::parse(library_block) else { return Ok(()) };
    let keys: Vec<Key> = o.features.iter().flat_map(|f| f.keys.clone()).collect();
    for b in user_blocks(store).into_iter().filter(|b| b.keys.iter().any(|k| keys.contains(k))) {
        let text = store.read_text(&b.path).map_err(|e| e.to_string())?;
        let rest: Vec<String> = blocks(&text).into_iter().filter(|x| !same(x, &b.text)).collect();
        if rest.is_empty() {
            store.fs().remove_file(&b.path).map_err(|e| format!("{}: {e}", b.path.display()))?;
        } else {
            store.write_text(&b.path, &rest.join("\n")).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The same rules, whatever the spacing.
fn same(a: &str, b: &str) -> bool {
    fn norm(s: &str) -> Vec<&str> {
        s.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect()
    }
    norm(a) == norm(b)
}

/// `classFeature:Roving|Ranger|SRD52|6` → `Roving (Ranger 6)`.
pub fn rule_label(id: &str) -> String {
    let (kind, rest) = id.split_once(':').unwrap_or(("", id));
    let p: Vec<&str> = rest.split('|').collect();
    let name = p.first().copied().unwrap_or(rest);
    let what = match (kind, p.as_slice()) {
        ("classFeature", [_, class, _, level, ..]) => format!("{class} {level}"),
        ("subclassFeature", [_, class, _, sub, _, level, ..]) => format!("{class} {level}, {sub}"),
        ("feat", _) => "feat".into(),
        ("optionalfeature", _) => "option".into(),
        ("race", _) => "species".into(),
        ("background", _) => "background".into(),
        ("item", _) => "item".into(),
        _ => String::new(),
    };
    if what.is_empty() {
        name.to_string()
    } else {
        format!("{name} ({what})")
    }
}

fn folder_label(name: &str) -> String {
    match name {
        "srd" => "SRD".into(),
        _ => label_of(name),
    }
}

/// `kibbles-dragon-patron` → `Kibbles Dragon Patron`.
fn label_of(name: &str) -> String {
    name.split(['-', '_']).filter(|w| !w.is_empty()).map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ")
}

/// Every rule's id under a path of the tree: `srd`, `srd/classes.toml`,
/// or a rule's id.
pub fn ids_under(lib: &RulesLibrary, path: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for folder in &lib.folders {
        for file in &folder.files {
            for rule in &file.rules {
                let file_stem = file.path.trim_end_matches(".toml");
                if path == folder.name || path == file.path || path == file_stem || path == rule.id {
                    out.insert(rule.id.clone());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_store::fs::MemFs;
    use std::sync::Arc;

    fn files() -> Vec<(String, String)> {
        vec![
            (
                "srd/classes.toml".into(),
                "# Classes\n[[feature]]\nid = \"classFeature:Roving|Ranger|SRD52|6\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"10\"\n\n[[feature]]\nid = \"feat:Alert|SRD52\"\n[[feature.effect]]\ntarget = \"initiative\"\nadd = \"prof\"\n".into(),
            ),
            ("homebrew/my-brew.toml".into(), "[[feature]]\nid = \"feat:Brewed Luck|TSTB\"\n".into()),
        ]
    }

    #[test]
    fn the_library_is_a_tree_and_imports_with_a_choice_for_each_rule_of_yours() {
        let store = Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml");
        // Yours: other rules for Roving, in a file with another feature.
        let mine = "[[feature]]\nid = \"classFeature:Roving|Ranger|SRD52|6\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"5\"\n\n[[feature]]\nid = \"feat:Other|T\"\n";
        store.write_text(&store.overlays_dir().join("mine.toml"), mine).unwrap();
        let lib = tree(&files(), &store);
        assert_eq!(lib.folders.iter().map(|f| f.label.as_str()).collect::<Vec<_>>(), ["SRD", "Homebrew"]);
        let rules = &lib.folders[0].files[0].rules;
        assert_eq!(lib.folders[0].files[0].label, "Classes");
        assert_eq!((rules[0].label.as_str(), rules[0].status), ("Roving (Ranger 6)", Status::Differs));
        assert_eq!((rules[1].label.as_str(), rules[1].status, rules[1].builtin), ("Alert (feat)", Status::New, true));
        assert_eq!(ids_under(&lib, "srd").len(), 2);
        assert_eq!(ids_under(&lib, "homebrew/my-brew").len(), 1);

        // Keep yours: Alert comes in, Roving stays yours.
        let all = ids_under(&lib, "srd");
        let done = import(&store, &lib, &all, &BTreeSet::new()).unwrap();
        assert_eq!((done.added, done.kept, done.overwritten), (1, 1, 0));
        assert!(store.read_text(&store.overlays_dir().join("mine.toml")).unwrap().contains("add = \"5\""));
        // Overwrite yours: the library's Roving, your other feature kept.
        let lib = tree(&files(), &store);
        let overwrite: BTreeSet<String> = ["classFeature:Roving|Ranger|SRD52|6".to_string()].into();
        let done = import(&store, &lib, &all, &overwrite).unwrap();
        assert_eq!((done.overwritten, done.same), (1, 1));
        let left = store.read_text(&store.overlays_dir().join("mine.toml")).unwrap();
        assert!(!left.contains("Roving") && left.contains("feat:Other|T"), "{left}");
        let lib = tree(&files(), &store);
        assert!(lib.folders[0].files[0].rules.iter().all(|r| r.status == Status::Same));
    }

    #[test]
    fn a_zip_of_the_repository_is_read() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        for (path, text) in [
            ("dnt-rules-main/README.md", "x"),
            ("dnt-rules-main/.github/workflows/check.yml", "x"),
            ("dnt-rules-main/srd/feats.toml", "[[feature]]\nid = \"feat:Alert|SRD52\"\n"),
        ] {
            zip.start_file(path, opts).unwrap();
            std::io::Write::write_all(&mut zip, text.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let files = read_zip(&bytes).unwrap();
        assert_eq!(files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["srd/feats.toml"]);
    }
}
