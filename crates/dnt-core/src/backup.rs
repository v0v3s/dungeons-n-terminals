//! Backups: everything dnt keeps, in one zip that restores it on another
//! device. Character and campaign exports (see [`crate::bundle`]) are zips
//! of the same shape: a `dnt.toml` saying what the zip is, then its files.
//!
//! ```text
//! dnt.toml        kind = "backup", format, the dnt that made it, when
//! config.toml     the settings (not where this device keeps its data)
//! data/…          the data folder as it is: campaigns, the library
//! ```
//!
//! Backups are made by hand or, at start, every day, week or month
//! (keeping the last few), into a folder of the user's choosing (a
//! [`Shelf`]). Restoring one replaces everything, after backing up what
//! was there.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dnt_store::fs::Fs;
use dnt_store::{Config, Every, Store};
use serde::{Deserialize, Serialize};

/// Where backups go: a folder, on a file system of its own (a folder
/// picked on Android is not a path, so the app gives it one that reaches
/// it).
#[derive(Debug, Clone)]
pub struct Shelf {
    pub fs: Arc<dyn Fs>,
    pub dir: PathBuf,
    /// The folder as the user knows it.
    pub place: String,
}

impl Shelf {
    /// The folder at `dir`, on the store's file system.
    pub fn at(store: &Store, dir: impl Into<PathBuf>) -> Shelf {
        let dir = dir.into();
        Shelf { fs: store.shared_fs(), place: dir.display().to_string(), dir }
    }

    /// The folder the settings name, else `backups/` in the data folder.
    /// A folder that is not a path (a `content://` URI) is the app's to
    /// reach: the default one here.
    pub fn configured(store: &Store, config: &Config) -> Shelf {
        match config.backup.folder.as_deref().filter(|f| !f.contains("://")) {
            Some(folder) => Shelf::at(store, folder),
            None => Shelf::default_for(store),
        }
    }

    /// `backups/` in the data folder.
    pub fn default_for(store: &Store) -> Shelf {
        Shelf::at(store, store.data_dir().join("backups"))
    }
}

/// The format a zip is written in; a newer one is refused.
pub const FORMAT: u32 = 1;

/// What a zip of dnt's is (its `dnt.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    /// `backup`, `character` or `campaign`.
    pub kind: String,
    /// The character or campaign (or "Everything").
    pub name: String,
    /// The dnt that made it.
    pub dnt: String,
    /// When (`2026-10-29 21:30`).
    pub made: String,
    /// Sources it needs that it does not carry (books).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
}

impl Manifest {
    pub fn new(kind: &str, name: &str) -> Manifest {
        Manifest {
            format: FORMAT,
            kind: kind.into(),
            name: name.into(),
            dnt: env!("CARGO_PKG_VERSION").into(),
            made: now(),
            needs: Vec::new(),
        }
    }
}

/// The local time as backups name it: `2026-10-29 21:30`.
pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// The same, to the second (for a backup's file name).
fn now_exact() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// A zip being written, in memory.
pub(crate) struct ZipOut(zip::ZipWriter<Cursor<Vec<u8>>>);

impl ZipOut {
    pub(crate) fn new() -> ZipOut {
        ZipOut(zip::ZipWriter::new(Cursor::new(Vec::new())))
    }

    pub(crate) fn add(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        self.0.start_file(name, opts).map_err(|e| e.to_string())?;
        self.0.write_all(bytes).map_err(|e| e.to_string())
    }

    pub(crate) fn finish(self) -> Result<Vec<u8>, String> {
        Ok(self.0.finish().map_err(|e| e.to_string())?.into_inner())
    }
}

/// A zip being read: its files by name.
pub(crate) struct ZipIn(zip::ZipArchive<Cursor<Vec<u8>>>);

impl ZipIn {
    pub(crate) fn new(bytes: Vec<u8>) -> Result<ZipIn, String> {
        zip::ZipArchive::new(Cursor::new(bytes)).map(ZipIn).map_err(|e| format!("Not a zip file: {e}"))
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.0.file_names().filter_map(Result::ok).filter(|n| !n.ends_with('/')).map(|n| n.into_owned()).collect()
    }

    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>, String> {
        let mut f = self.0.by_name(name).map_err(|e| format!("{name}: {e}"))?;
        let mut out = Vec::new();
        f.read_to_end(&mut out).map_err(|e| format!("{name}: {e}"))?;
        Ok(out)
    }

    pub(crate) fn text(&mut self, name: &str) -> Result<String, String> {
        String::from_utf8(self.read(name)?).map_err(|e| format!("{name}: {e}"))
    }

    /// Its `dnt.toml`, if it has one; one from a newer dnt is refused.
    pub(crate) fn manifest(&mut self) -> Result<Option<Manifest>, String> {
        if !self.names().iter().any(|n| n == "dnt.toml") {
            return Ok(None);
        }
        let m: Manifest = toml::from_str(&self.text("dnt.toml")?).map_err(|e| format!("dnt.toml: {e}"))?;
        if m.format > FORMAT {
            return Err(format!("This was made by a newer dnt ({}): update dnt to open it", m.dnt));
        }
        Ok(Some(m))
    }
}

/// Every file under `dir` (relative paths with `/`), but not the folders
/// left out.
fn files(store: &Store, dir: &Path, rel: &str, skip: &dyn Fn(&str) -> bool, out: &mut Vec<String>) {
    let Ok(entries) = store.fs().list(dir) else { return };
    for (name, is_dir) in entries {
        let path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        if skip(&path) || name.ends_with(".tmp") {
            continue;
        }
        if is_dir {
            files(store, &dir.join(&name), &path, skip, out);
        } else {
            out.push(path);
        }
    }
}

/// What a backup leaves out: backups themselves, exported sheets.
fn left_out(path: &str) -> bool {
    path == "backups" || (path.starts_with("campaigns/") && path.ends_with("/exports"))
}

/// Everything, as a zip.
pub fn make(store: &Store) -> Result<Vec<u8>, String> {
    let mut zip = ZipOut::new();
    zip.add("dnt.toml", toml::to_string(&Manifest::new("backup", "Everything")).map_err(|e| e.to_string())?.as_bytes())?;
    // The settings, but not where this device keeps its data.
    let mut config = store.load_config().map_err(|e| e.to_string())?;
    config.library.path = None;
    config.backup.folder = None;
    zip.add("config.toml", toml::to_string(&config).map_err(|e| e.to_string())?.as_bytes())?;
    let mut all = Vec::new();
    files(store, store.data_dir(), "", &left_out, &mut all);
    all.sort();
    for rel in all {
        let bytes = store.fs().read(&store.data_dir().join(&rel)).map_err(|e| format!("{rel}: {e}"))?;
        zip.add(&format!("data/{rel}"), &bytes)?;
    }
    zip.finish()
}

/// The name of a backup made at `made`: `dnt-backup-2026-10-29-213015.zip`.
fn file_name(made: &str, what: &str) -> String {
    let stamp: String = made.chars().map(|c| if c == ' ' { '-' } else { c }).filter(|c| *c != ':').collect();
    format!("dnt-backup-{stamp}{what}.zip")
}

/// Back everything up onto `shelf`, now; the file's path.
pub fn save(store: &Store, shelf: &Shelf, what: &str) -> Result<PathBuf, String> {
    save_at(store, shelf, &now_exact(), what)
}

fn save_at(store: &Store, shelf: &Shelf, made: &str, what: &str) -> Result<PathBuf, String> {
    let bytes = make(store)?;
    let mut name = file_name(made, what);
    // Two at the same time: the second one numbered.
    let mut n = 2;
    while shelf.fs.exists(&shelf.dir.join(&name)) {
        name = file_name(made, &format!("{what}-{n}"));
        n += 1;
    }
    let path = shelf.dir.join(&name);
    shelf.fs.write(&path, &bytes).map_err(|e| format!("Could not write {name} in {}: {e}", shelf.place))?;
    Ok(path)
}

/// A backup in the backups folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Saved {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
}

/// The backups on `shelf`, newest first.
pub fn list(shelf: &Shelf) -> Vec<Saved> {
    let dir = &shelf.dir;
    let mut out: Vec<Saved> = shelf
        .fs
        .list(dir)
        .unwrap_or_default()
        .into_iter()
        .filter(|(n, d)| !d && n.starts_with("dnt-backup-") && n.ends_with(".zip"))
        .map(|(name, _)| {
            let path = dir.join(&name);
            let bytes = shelf.fs.len(&path).unwrap_or(0);
            Saved { name, path, bytes }
        })
        .collect();
    out.sort_by(|a, b| b.name.cmp(&a.name));
    out
}

/// Check a backup before anything changes: a zip of dnt's, a backup, not
/// from a newer dnt, its files readable.
pub fn check(bytes: Vec<u8>) -> Result<Manifest, String> {
    let mut zip = ZipIn::new(bytes)?;
    let m = zip.manifest()?.ok_or("Not a dnt backup (it has no dnt.toml)")?;
    if m.kind != "backup" {
        return Err(format!("Not a backup: it is a {} ({})", m.kind, m.name));
    }
    for name in zip.names() {
        zip.read(&name)?;
    }
    Ok(m)
}

/// Restore a backup: what is here now is backed up onto `shelf` first,
/// then replaced. Returns that safety backup's path.
pub fn restore(store: &Store, bytes: Vec<u8>, shelf: &Shelf) -> Result<PathBuf, String> {
    check(bytes.clone())?;
    let mut zip = ZipIn::new(bytes)?;
    let safety = save(store, shelf, "-before-restore")?;
    // Out with the old, except the backups.
    for (name, is_dir) in store.fs().list(store.data_dir()).unwrap_or_default() {
        if left_out(&name) {
            continue;
        }
        let path = store.data_dir().join(&name);
        let gone = if is_dir { store.fs().remove_dir_all(&path) } else { store.fs().remove_file(&path) };
        gone.map_err(|e| format!("Could not clear {}: {e}", path.display()))?;
    }
    for name in zip.names() {
        let Some(rel) = name.strip_prefix("data/") else { continue };
        let bytes = zip.read(&name)?;
        store.fs().write(&store.data_dir().join(rel), &bytes).map_err(|e| format!("{rel}: {e}"))?;
    }
    // Its settings, keeping where this device keeps its data and its
    // backup schedule.
    let here = store.load_config().unwrap_or_default();
    let mut config: dnt_store::Config = toml::from_str(&zip.text("config.toml")?).map_err(|e| format!("config.toml: {e}"))?;
    config.library.path = here.library.path;
    config.backup = here.backup;
    store.save_config(&config).map_err(|e| e.to_string())?;
    Ok(safety)
}

/// Days from `from` to `to` (both `2026-10-29 21:30`).
fn days_between(from: &str, to: &str) -> Option<i64> {
    let parse = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").ok();
    Some((parse(to)? - parse(from)?).num_days())
}

/// At start: back everything up when the schedule says so, and keep only
/// the last few automatic ones. The first time only notes when the
/// schedule began. `Some` with what was done (or why it failed).
pub fn auto(store: &Store, shelf: &Shelf, now: &str) -> Option<Result<String, String>> {
    let mut config = store.load_config().ok()?;
    let days = config.backup.every.days()?;
    let due = match &config.backup.last {
        None => false,
        Some(last) => days_between(last, now).is_none_or(|d| d >= days),
    };
    if !due {
        if config.backup.last.is_none() {
            config.backup.last = Some(now.to_string());
            store.save_config(&config).ok()?;
        }
        return None;
    }
    let made = save_at(store, shelf, now, "");
    config.backup.last = Some(now.to_string());
    if let Err(e) = store.save_config(&config) {
        return Some(Err(e.to_string()));
    }
    Some(made.map(|path| {
        let pruned = prune(shelf, config.backup.keep as usize);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if pruned > 0 {
            format!("Backed up to {name} ({pruned} older removed)")
        } else {
            format!("Backed up to {name}")
        }
    }))
}

/// Delete automatic backups beyond the newest `keep` (not the ones made
/// before a restore). How many went.
pub fn prune(shelf: &Shelf, keep: usize) -> usize {
    let auto: Vec<Saved> = list(shelf).into_iter().filter(|s| !s.name.contains("before-restore")).collect();
    auto.iter().skip(keep.max(1)).filter(|s| shelf.fs.remove_file(&s.path).is_ok()).count()
}

/// The schedule as words, for settings: `every week, keeping 5`.
pub fn describe(every: Every, keep: u32) -> String {
    match every {
        Every::Off => "off".into(),
        Every::Day => format!("every day, keeping {keep}"),
        Every::Week => format!("every week, keeping {keep}"),
        Every::Month => format!("every month, keeping {keep}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_store::fs::MemFs;
    use std::sync::Arc;

    fn store() -> Store {
        Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml")
    }

    #[test]
    fn a_backup_restores_everything_and_what_was_there_is_kept() {
        let s = store();
        s.fs().write(Path::new("/data/campaigns/a/campaign.toml"), b"format = 1\nname = \"A\"\n").unwrap();
        s.fs().write(Path::new("/data/campaigns/a/exports/x.md"), b"x").unwrap();
        s.fs().write(Path::new("/data/library/imported/books.json"), b"{}").unwrap();
        let bytes = make(&s).unwrap();
        let m = check(bytes.clone()).unwrap();
        assert_eq!((m.kind.as_str(), m.format), ("backup", FORMAT));
        let names = ZipIn::new(bytes.clone()).unwrap().names();
        assert!(names.contains(&"data/campaigns/a/campaign.toml".to_string()));
        assert!(!names.iter().any(|n| n.contains("exports")), "{names:?}");

        // Something else now; restoring brings the backup back.
        s.fs().remove_dir_all(Path::new("/data/campaigns/a")).unwrap();
        s.fs().write(Path::new("/data/campaigns/b/campaign.toml"), b"format = 1\nname = \"B\"\n").unwrap();
        let safety = restore(&s, bytes, &Shelf::default_for(&s)).unwrap();
        assert!(s.fs().is_file(Path::new("/data/campaigns/a/campaign.toml")));
        assert!(!s.fs().exists(Path::new("/data/campaigns/b")));
        // What was there is in the safety backup.
        let before = ZipIn::new(s.fs().read(&safety).unwrap()).unwrap().names();
        assert!(before.contains(&"data/campaigns/b/campaign.toml".to_string()), "{before:?}");
    }

    #[test]
    fn a_broken_or_newer_backup_changes_nothing() {
        let s = store();
        assert!(check(b"not a zip".to_vec()).unwrap_err().contains("Not a zip"));
        let mut zip = ZipOut::new();
        let mut m = Manifest::new("backup", "Everything");
        m.format = FORMAT + 1;
        zip.add("dnt.toml", toml::to_string(&m).unwrap().as_bytes()).unwrap();
        let newer = zip.finish().unwrap();
        s.fs().write(Path::new("/data/campaigns/a/campaign.toml"), b"x").unwrap();
        assert!(restore(&s, newer, &Shelf::default_for(&s)).unwrap_err().contains("newer dnt"));
        assert!(s.fs().is_file(Path::new("/data/campaigns/a/campaign.toml")));
        assert!(list(&Shelf::default_for(&s)).is_empty(), "no safety backup either");
    }

    #[test]
    fn backups_go_to_the_folder_chosen_which_another_device_does_not_get() {
        let s = store();
        let mut config = s.load_config().unwrap();
        config.backup.folder = Some("/elsewhere".into());
        s.save_config(&config).unwrap();
        let shelf = Shelf::configured(&s, &config);
        assert_eq!(shelf.place, "/elsewhere");
        save(&s, &shelf, "").unwrap();
        assert_eq!(list(&shelf).len(), 1);
        let mut zip = ZipIn::new(make(&s).unwrap()).unwrap();
        assert!(!zip.text("config.toml").unwrap().contains("elsewhere"));
        // A folder the app reaches (Android's): the default one here.
        config.backup.folder = Some("content://com.android.externalstorage.documents/tree/x".into());
        assert_eq!(Shelf::configured(&s, &config).dir, Path::new("/data/backups"));
    }

    #[test]
    fn automatic_backups_follow_the_schedule_and_keep_the_last_few() {
        let s = store();
        let dir = &Shelf::at(&s, "/backups");
        // The first start only notes the time.
        assert_eq!(auto(&s, dir, "2026-10-01 10:00"), None);
        assert_eq!(s.load_config().unwrap().backup.last.as_deref(), Some("2026-10-01 10:00"));
        assert_eq!(auto(&s, dir, "2026-10-05 10:00"), None, "not a week yet");
        let mut config = s.load_config().unwrap();
        config.backup.keep = 2;
        s.save_config(&config).unwrap();
        // Each a week later, made with distinct names.
        for (i, day) in ["2026-10-08", "2026-10-15", "2026-10-22"].iter().enumerate() {
            let done = auto(&s, dir, &format!("{day} 10:00")).unwrap().unwrap();
            assert!(done.starts_with("Backed up to dnt-backup-"), "{done}");
            assert_eq!(list(dir).len(), (i + 1).min(2));
        }
        config = s.load_config().unwrap();
        config.backup.every = Every::Off;
        s.save_config(&config).unwrap();
        assert_eq!(auto(&s, dir, "2027-01-01 10:00"), None);
    }
}
