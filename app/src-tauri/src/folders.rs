//! A folder the user picked for backups. On Android it is not a path but a
//! folder the system's picker granted (a `content://` tree URI: on the
//! phone, an SD card or a cloud drive), reached through FoldersPlugin.kt;
//! here it is a file system ([`TreeFs`]) for dnt_core::backup.

#[cfg(target_os = "android")]
pub use android::TreeFs;

/// Register the Android side.
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("folders")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("cz.v0v3s.dnt", "FoldersPlugin")?;
                _app.manage(android::Plugin(handle));
            }
            Ok(())
        })
        .build()
}

/// Ask for a folder: a path, or on Android a tree URI. `None` when the
/// picker was closed.
pub fn pick(handle: &tauri::AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        android::pick(handle)
    }
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;
        Ok(handle.dialog().file().blocking_pick_folder().map(|f| f.to_string()))
    }
}

/// A tree URI as the user knows the folder: `Documents/dnt` (on the
/// phone), `1234-ABCD: dnt` (an SD card).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn place_of(tree: &str) -> String {
    let id = tree.rsplit_once("/tree/").map_or(tree, |(_, id)| id);
    let id = decode(id);
    match id.split_once(':') {
        Some(("primary", "")) => "the phone's storage".into(),
        Some(("primary", path)) => path.to_string(),
        Some((volume, path)) if !volume.contains('.') => format!("{volume}: {path}"),
        _ => id,
    }
}

/// Undo a URI's `%XX` escapes.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "android")]
mod android {
    use std::io;
    use std::path::{Path, PathBuf};

    use dnt_store::fs::Fs;
    use serde::{Deserialize, Serialize};
    use tauri::plugin::PluginHandle;
    use tauri::{Manager, Wry};

    pub struct Plugin(pub PluginHandle<Wry>);

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Tree<'a> {
        tree: &'a str,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct File<'a> {
        tree: &'a str,
        name: &'a str,
        /// A file of the app's own to copy from or to.
        path: String,
    }

    #[derive(Deserialize)]
    struct Picked {
        tree: Option<String>,
    }

    #[derive(Deserialize)]
    struct Listed {
        files: Vec<Entry>,
    }

    #[derive(Deserialize)]
    struct Entry {
        name: String,
        size: u64,
    }

    #[derive(Deserialize)]
    struct Done {}

    pub fn pick(handle: &tauri::AppHandle) -> Result<Option<String>, String> {
        let plugin = handle.state::<Plugin>();
        let picked: Picked = plugin.0.run_mobile_plugin("pickFolder", ()).map_err(|e| e.to_string())?;
        Ok(picked.tree)
    }

    /// The files of a picked folder (its own, not its folders' files).
    /// Paths are the files' names.
    pub struct TreeFs {
        plugin: PluginHandle<Wry>,
        tree: String,
        /// Where files are copied through.
        cache: PathBuf,
    }

    impl std::fmt::Debug for TreeFs {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TreeFs({})", self.tree)
        }
    }

    impl TreeFs {
        pub fn new(handle: &tauri::AppHandle, tree: String) -> Result<TreeFs, String> {
            let cache = handle.path().app_cache_dir().map_err(|e| e.to_string())?.join("folders");
            std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
            Ok(TreeFs { plugin: handle.state::<Plugin>().0.clone(), tree, cache })
        }

        fn files(&self) -> io::Result<Vec<Entry>> {
            let listed: Listed = self.plugin.run_mobile_plugin("list", Tree { tree: &self.tree }).map_err(other)?;
            Ok(listed.files)
        }

        fn run(&self, what: &str, path: &Path, through: &Path) -> io::Result<()> {
            let file = File { tree: &self.tree, name: &name(path), path: through.display().to_string() };
            self.plugin.run_mobile_plugin::<Done>(what, file).map(|_| ()).map_err(other)
        }
    }

    fn other(e: impl std::fmt::Display) -> io::Error {
        io::Error::other(e.to_string())
    }

    fn name(path: &Path) -> String {
        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn unsupported() -> io::Error {
        io::Error::new(io::ErrorKind::Unsupported, "not in a picked folder")
    }

    impl Fs for TreeFs {
        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            let through = self.cache.join("in.tmp");
            self.run("copyOut", path, &through)?;
            let bytes = std::fs::read(&through);
            let _ = std::fs::remove_file(&through);
            bytes
        }

        fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
            let through = self.cache.join("out.tmp");
            std::fs::write(&through, bytes)?;
            let done = self.run("copyIn", path, &through);
            let _ = std::fs::remove_file(&through);
            done
        }

        fn create_dir_all(&self, _: &Path) -> io::Result<()> {
            Ok(())
        }

        fn list(&self, _: &Path) -> io::Result<Vec<(String, bool)>> {
            Ok(self.files()?.into_iter().map(|e| (e.name, false)).collect())
        }

        fn rename(&self, _: &Path, _: &Path) -> io::Result<()> {
            Err(unsupported())
        }

        fn remove_file(&self, path: &Path) -> io::Result<()> {
            self.run("remove", path, Path::new(""))
        }

        fn remove_dir_all(&self, _: &Path) -> io::Result<()> {
            Err(unsupported())
        }

        fn is_file(&self, path: &Path) -> bool {
            let name = name(path);
            self.files().is_ok_and(|f| f.iter().any(|e| e.name == name))
        }

        fn is_dir(&self, path: &Path) -> bool {
            path.as_os_str().is_empty()
        }

        fn len(&self, path: &Path) -> io::Result<u64> {
            let name = name(path);
            self.files()?.into_iter().find(|e| e.name == name).map(|e| e.size).ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_folder_is_named_as_the_user_knows_it() {
        let tree = "content://com.android.externalstorage.documents/tree/primary%3ADocuments%2Fdnt";
        assert_eq!(place_of(tree), "Documents/dnt");
        assert_eq!(place_of("content://com.android.externalstorage.documents/tree/1234-ABCD%3Adnt"), "1234-ABCD: dnt");
        assert_eq!(place_of("content://com.android.externalstorage.documents/tree/primary%3A"), "the phone's storage");
    }
}
