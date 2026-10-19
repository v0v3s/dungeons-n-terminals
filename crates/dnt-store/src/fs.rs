//! The file system the store reads and writes: the real disk ([`OsFs`]),
//! or files kept in memory ([`MemFs`]) for tests and for frontends with no
//! disk of their own (a browser keeps them elsewhere and loads them back).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub trait Fs: Send + Sync + Debug {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Write a whole file, creating its folders; never leaves it half
    /// written.
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    /// A folder's entries: names, and whether each is a folder.
    fn list(&self, dir: &Path) -> io::Result<Vec<(String, bool)>>;
    /// Move a file or a folder with everything in it.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;

    fn exists(&self, path: &Path) -> bool {
        self.is_file(path) || self.is_dir(path)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

/// The real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsFs;

impl Fs for OsFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    /// Via a temporary file and a rename, so a crash never leaves a
    /// half-written file.
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<(String, bool)>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir)?.flatten() {
            out.push((e.file_name().to_string_lossy().into_owned(), e.path().is_dir()));
        }
        Ok(out)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }
}

/// Files and folders in memory.
#[derive(Debug, Default)]
pub struct MemFs {
    inner: Mutex<Mem>,
}

#[derive(Debug, Default, Clone)]
struct Mem {
    files: BTreeMap<PathBuf, Vec<u8>>,
    dirs: BTreeSet<PathBuf>,
}

impl Mem {
    fn add_dirs(&mut self, path: &Path) {
        for a in path.ancestors().filter(|a| !a.as_os_str().is_empty()) {
            self.dirs.insert(a.to_path_buf());
        }
    }
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{} not found", path.display()))
}

impl MemFs {
    pub fn new() -> MemFs {
        MemFs::default()
    }

    /// Every file with its contents, and every folder (empty ones too), to
    /// keep them somewhere.
    pub fn contents(&self) -> (BTreeMap<PathBuf, Vec<u8>>, BTreeSet<PathBuf>) {
        let m = self.inner.lock().unwrap();
        (m.files.clone(), m.dirs.clone())
    }

    /// Files and folders kept from an earlier [`MemFs::contents`].
    pub fn from_contents(files: BTreeMap<PathBuf, Vec<u8>>, dirs: BTreeSet<PathBuf>) -> MemFs {
        let mut mem = Mem::default();
        for path in files.keys().filter_map(|f| f.parent()).chain(dirs.iter().map(PathBuf::as_path)) {
            mem.add_dirs(path);
        }
        mem.files = files;
        MemFs { inner: Mutex::new(mem) }
    }
}

impl Fs for MemFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.inner.lock().unwrap().files.get(path).cloned().ok_or_else(|| not_found(path))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut m = self.inner.lock().unwrap();
        if m.dirs.contains(path) {
            return Err(io::Error::new(io::ErrorKind::IsADirectory, format!("{} is a folder", path.display())));
        }
        if let Some(parent) = path.parent() {
            m.add_dirs(parent);
        }
        m.files.insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.lock().unwrap().add_dirs(path);
        Ok(())
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<(String, bool)>> {
        let m = self.inner.lock().unwrap();
        if !m.dirs.contains(dir) {
            return Err(not_found(dir));
        }
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
        let dirs = m.dirs.iter().filter(|d| d.parent() == Some(dir)).filter_map(|d| Some((name(d)?, true)));
        let files = m.files.keys().filter(|f| f.parent() == Some(dir)).filter_map(|f| Some((name(f)?, false)));
        Ok(dirs.chain(files).collect())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut m = self.inner.lock().unwrap();
        if let Some(bytes) = m.files.remove(from) {
            if let Some(parent) = to.parent() {
                m.add_dirs(parent);
            }
            m.files.insert(to.to_path_buf(), bytes);
            return Ok(());
        }
        if !m.dirs.contains(from) {
            return Err(not_found(from));
        }
        let moved = |p: &Path| p.strip_prefix(from).ok().map(|rest| to.join(rest));
        m.dirs = m.dirs.iter().map(|d| moved(d).unwrap_or_else(|| d.clone())).collect();
        m.files = std::mem::take(&mut m.files).into_iter().map(|(f, b)| (moved(&f).unwrap_or(f), b)).collect();
        if let Some(parent) = to.parent() {
            m.add_dirs(parent);
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.inner.lock().unwrap().files.remove(path).map(|_| ()).ok_or_else(|| not_found(path))
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut m = self.inner.lock().unwrap();
        if !m.dirs.contains(path) {
            return Err(not_found(path));
        }
        m.dirs.retain(|d| !d.starts_with(path));
        m.files.retain(|f, _| !f.starts_with(path));
        Ok(())
    }

    fn is_file(&self, path: &Path) -> bool {
        self.inner.lock().unwrap().files.contains_key(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        self.inner.lock().unwrap().dirs.contains(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_files_and_folders() {
        let fs = MemFs::new();
        let p = Path::new;
        fs.write(p("/d/a/one.md"), b"1").unwrap();
        fs.create_dir_all(p("/d/b")).unwrap();
        assert!(fs.is_dir(p("/d/a")) && fs.is_file(p("/d/a/one.md")) && !fs.exists(p("/d/c")));
        let mut root = fs.list(p("/d")).unwrap();
        root.sort();
        assert_eq!(root, [("a".to_string(), true), ("b".to_string(), true)]);
        assert!(fs.list(p("/nope")).is_err());

        fs.rename(p("/d/a"), p("/d/c")).unwrap();
        assert_eq!(fs.read_to_string(p("/d/c/one.md")).unwrap(), "1");
        assert!(!fs.exists(p("/d/a")));
        fs.rename(p("/d/c/one.md"), p("/d/b/two.md")).unwrap();
        assert_eq!(fs.list(p("/d/b")).unwrap(), [("two.md".to_string(), false)]);

        fs.create_dir_all(p("/d/empty")).unwrap();
        let (files, dirs) = fs.contents();
        let kept = MemFs::from_contents(files, dirs);
        assert!(kept.is_dir(p("/d/b")) && kept.is_file(p("/d/b/two.md")) && kept.is_dir(p("/d/empty")));
        fs.remove_dir_all(p("/d/b")).unwrap();
        assert!(fs.read(p("/d/b/two.md")).is_err());
    }
}
