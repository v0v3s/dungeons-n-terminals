//! Updating dnt itself: the newest release on GitHub, whether it is newer
//! than this one, and its file for this platform, downloaded and checked
//! against the release's `SHA256SUMS`. Installing it is each frontend's
//! (the terminal app replaces its binary; the app hands an APK or an
//! installer to the system).

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The newest release, as GitHub's API gives it.
pub const DEFAULT_URL: &str = "https://api.github.com/repos/v0v3s/dungeons-n-terminals/releases/latest";

/// This dnt's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A release: its version and notes, and its files.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    /// `1.4.0`
    pub version: String,
    /// Its notes (Markdown).
    pub notes: String,
    /// (file name, where to download it)
    pub assets: Vec<(String, String)>,
}

/// What a frontend installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Android,
    WindowsApp,
    LinuxTerminal,
    WindowsTerminal,
}

impl Target {
    /// The release file for it.
    pub fn asset(self) -> &'static str {
        match self {
            Target::Android => "dnt-app-android.apk",
            Target::WindowsApp => "dnt-app-windows-x86_64-setup.exe",
            Target::LinuxTerminal => "dnt-linux-x86_64",
            Target::WindowsTerminal => "dnt-windows-x86_64.exe",
        }
    }
}

/// Where to look: `DNT_UPDATE_URL` (a web address of the release, or a
/// release's JSON file or the folder holding it as `release.json`, as the
/// tests do), else the newest release on GitHub.
pub fn location() -> String {
    std::env::var("DNT_UPDATE_URL").ok().filter(|l| !l.is_empty()).unwrap_or_else(|| DEFAULT_URL.to_string())
}

/// Read a release from GitHub's JSON for it (`tag_name`, `body`, `assets`
/// with `name` and `browser_download_url`; a download that is not a web
/// address is a path relative to `base`).
pub fn parse(json: &str, base: Option<&Path>) -> Result<Release, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("Not a release: {e}"))?;
    let tag = v.get("tag_name").and_then(|t| t.as_str()).ok_or("Not a release: no tag_name")?;
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .filter_map(|a| {
            let name = a.get("name")?.as_str()?.to_string();
            let url = a.get("browser_download_url")?.as_str()?;
            let url = match base {
                Some(b) if !url.contains("://") => b.join(url).display().to_string(),
                _ => url.to_string(),
            };
            Some((name, url))
        })
        .collect();
    Ok(Release {
        version: tag.trim_start_matches('v').to_string(),
        notes: v.get("body").and_then(|b| b.as_str()).unwrap_or_default().to_string(),
        assets,
    })
}

/// Whether `latest` is a newer version than `current` (`1.10.0` > `1.9.2`).
pub fn newer(latest: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> { v.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect() };
    parts(latest) > parts(current)
}

/// Whether today's automatic check is due: on, and not done today.
pub fn due(config: &dnt_store::UpdateConfig, today: &str) -> bool {
    config.auto && config.checked.as_deref() != Some(today)
}

/// The SHA-256 a release's `SHA256SUMS` gives a file (`<hex>  <name>`).
pub fn sum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_lowercase())
    })
}

/// A file's SHA-256, as hex.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(not(target_arch = "wasm32"))]
mod net {
    use super::*;

    /// Read a web address or a file.
    pub fn read(at: &str, limit: u64) -> Result<Vec<u8>, String> {
        if at.starts_with("https://") || at.starts_with("http://") {
            let mut response = ureq::get(at)
                .header("User-Agent", concat!("dnt/", env!("CARGO_PKG_VERSION")))
                .call()
                .map_err(|e| format!("Could not reach {}: {e}", host(at)))?;
            response.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| format!("Could not download it: {e}"))
        } else {
            std::fs::read(at).map_err(|e| format!("{at}: {e}"))
        }
    }

    fn host(at: &str) -> &str {
        at.split('/').nth(2).unwrap_or(at)
    }

    /// The newest release.
    pub fn latest(location: &str) -> Result<Release, String> {
        let path = Path::new(location);
        if !location.contains("://") && path.is_dir() {
            let json = String::from_utf8(read(&path.join("release.json").display().to_string(), 1 << 20)?).map_err(|e| e.to_string())?;
            return parse(&json, Some(path));
        }
        let base = (!location.contains("://")).then(|| path.parent().unwrap_or(path));
        let json = String::from_utf8(read(location, 1 << 20)?).map_err(|e| e.to_string())?;
        parse(&json, base)
    }

    /// Download the release's file for `target` into `dir`, checked against
    /// its `SHA256SUMS`: where it is.
    pub fn download(release: &Release, target: Target, dir: &Path) -> Result<PathBuf, String> {
        let name = target.asset();
        let url = |n: &str| release.assets.iter().find(|(a, _)| a == n).map(|(_, u)| u.clone());
        let file = url(name).ok_or_else(|| format!("dnt {} has no {name}", release.version))?;
        let sums = url("SHA256SUMS").ok_or_else(|| format!("dnt {} has no SHA256SUMS to check it against", release.version))?;
        let sums = String::from_utf8(read(&sums, 1 << 20)?).map_err(|e| e.to_string())?;
        let want = sum_for(&sums, name).ok_or_else(|| format!("SHA256SUMS does not list {name}"))?;
        let bytes = read(&file, 512 << 20)?;
        if sha256(&bytes) != want {
            return Err(format!("{name} does not match its SHA256SUMS: not installed"));
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(name);
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use net::{download, latest, read};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("1.10.0", "1.9.2"));
        assert!(newer("2.0.0", "1.99.99"));
        assert!(!newer("1.3.0", "1.3.0"));
        assert!(!newer("1.2.9", "1.3.0"));
    }

    #[test]
    fn a_release_and_its_sums_are_read() {
        let json = r#"{"tag_name": "v1.4.0", "body": "- New things", "assets": [
            {"name": "dnt-linux-x86_64", "browser_download_url": "https://example.org/dnt-linux-x86_64"},
            {"name": "SHA256SUMS", "browser_download_url": "SHA256SUMS"}]}"#;
        let r = parse(json, Some(Path::new("/r"))).unwrap();
        assert_eq!((r.version.as_str(), r.notes.as_str()), ("1.4.0", "- New things"));
        assert_eq!(r.assets[1].1, Path::new("/r").join("SHA256SUMS").display().to_string());
        let sums = "abc123  dnt-linux-x86_64\nDEF456  dnt-app-android.apk\n";
        assert_eq!(sum_for(sums, "dnt-app-android.apk").as_deref(), Some("def456"));
        assert_eq!(sum_for(sums, "nothing"), None);
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn the_check_is_daily_and_can_be_off() {
        let mut c = dnt_store::UpdateConfig::default();
        assert!(c.auto && due(&c, "2026-10-30"));
        c.checked = Some("2026-10-30".into());
        assert!(!due(&c, "2026-10-30") && due(&c, "2026-10-31"));
        c.auto = false;
        assert!(!due(&c, "2026-10-31"));
    }
}
