//! On-disk storage for configuration and campaigns.
//!
//! ```text
//! <data>/campaigns/<slug>/campaign.toml
//!                        /characters/
//!                        /notes/
//! <config>/config.toml
//! ```

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod fs;
pub mod notes;

use fs::{Fs, OsFs};

use dnt_rules::character::Character;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
    #[error("{path}: {source}")]
    Parse { path: PathBuf, source: toml::de::Error },
    #[error("serialising {what}: {source}")]
    Serialize { what: String, source: toml::ser::Error },
    #[error("{path}: {message}")]
    Character { path: PathBuf, message: String },
    #[error("{path}: {message}")]
    Format { path: PathBuf, message: String },
    #[error("{path}: {message}")]
    Json { path: PathBuf, message: String },
    #[error("no campaign named {0:?}")]
    NoSuchCampaign(String),
    #[error("could not determine the user's data/config directories")]
    NoHome,
}

impl StoreError {
    /// The error for a person: the file's name instead of its whole path.
    pub fn brief(&self) -> String {
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match self {
            StoreError::Io { path, source } => format!("{}: {source}", name(path)),
            StoreError::Parse { path, source } => format!("{}: {}", name(path), source.message()),
            StoreError::Character { path, message } | StoreError::Format { path, message } | StoreError::Json { path, message } => {
                format!("{}: {message}", name(path))
            }
            other => other.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Named source presets offered when choosing a campaign's books.
pub const PRESETS: &[(&str, &[&str])] = &[
    ("2024 Core", &["XPHB", "XDMG", "XMM"]),
    ("2014 Core", &["PHB", "DMG", "MM"]),
    ("2014 Core + Supplements", &["PHB", "DMG", "MM", "XGE", "TCE", "MPMM", "FTD", "SCAG", "BMT"]),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub library: LibraryConfig,
    /// Slug of the campaign opened at startup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_campaign: Option<String>,
    /// Default notes folder order for new campaigns.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes_order: Vec<String>,
    /// Automatic backups.
    #[serde(default)]
    pub backup: BackupConfig,
}

/// How often dnt backs everything up by itself (when it starts).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    Off,
    Day,
    #[default]
    Week,
    Month,
}

impl Every {
    /// Days between backups (none when off).
    pub fn days(self) -> Option<i64> {
        match self {
            Every::Off => None,
            Every::Day => Some(1),
            Every::Week => Some(7),
            Every::Month => Some(30),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackupConfig {
    #[serde(default)]
    pub every: Every,
    /// How many automatic backups to keep (older ones are deleted).
    #[serde(default = "default_keep")]
    pub keep: u32,
    /// When the last one was made (`2026-10-29 21:30`), or when automatic
    /// backups started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
    /// Where they go, when not the default folder: a path, or on Android a
    /// folder picked in the system's dialog (a `content://` URI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

fn default_keep() -> u32 {
    5
}

impl Default for BackupConfig {
    fn default() -> BackupConfig {
        BackupConfig { every: Every::default(), keep: default_keep(), last: None, folder: None }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LibraryConfig {
    /// Path to the data directory (see schemas/README.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Extra homebrew JSON files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub homebrew: Vec<PathBuf>,
}

/// Contents of `campaign.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CampaignSettings {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Enabled source ids, e.g. `["XPHB", "XDMG"]`. Empty means all sources.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Enabled imported homebrew sources. Homebrew is off unless listed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub homebrew: Vec<String>,
    /// Show originals even when their reprint is enabled.
    #[serde(default)]
    pub show_reprinted: bool,
    #[serde(default, skip_serializing_if = "NotesSettings::is_empty")]
    pub notes: NotesSettings,
    /// Source abbreviation of this campaign's homebrew (e.g. `CoS`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NotesSettings {
    /// Top-level folder order in the notes tree.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
}

impl NotesSettings {
    fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Campaign {
    pub slug: String,
    pub dir: PathBuf,
    pub settings: CampaignSettings,
}

impl Campaign {
    pub fn characters_dir(&self) -> PathBuf {
        self.dir.join("characters")
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.dir.join("notes")
    }

    pub fn homebrew_path(&self) -> PathBuf {
        self.dir.join("homebrew.json")
    }

    /// The source filter this campaign implies. The campaign's own
    /// homebrew source and the global homebrew are always enabled.
    pub fn source_filter(&self) -> dnt_data::SourceFilter {
        let mut filter = if self.settings.sources.is_empty() {
            dnt_data::SourceFilter::default()
        } else {
            let mut sources = self.settings.sources.clone();
            sources.extend(self.settings.homebrew.iter().cloned());
            if !self.settings.source.is_empty() {
                sources.push(self.settings.source.clone());
            }
            sources.push(GLOBAL_SOURCE.to_string());
            dnt_data::SourceFilter::only(&sources)
        };
        filter.hide_reprinted = !self.settings.show_reprinted;
        filter
    }
}

/// A character file within a campaign.
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterFile {
    pub slug: String,
    pub path: PathBuf,
    pub character: Character,
}

pub struct Store {
    fs: Arc<dyn Fs>,
    data_dir: PathBuf,
    config_path: PathBuf,
}

impl Store {
    /// Use the platform's standard locations (`~/.local/share/dnt`,
    /// `~/.config/dnt/config.toml` on Linux), or `$DNT_HOME/data` and
    /// `$DNT_HOME/config.toml` when `DNT_HOME` is set (a portable install,
    /// or the end-to-end tests).
    pub fn open_default() -> Result<Store> {
        if let Some(home) = std::env::var_os("DNT_HOME").filter(|h| !h.is_empty()) {
            let home = PathBuf::from(home);
            return Ok(Store::open(home.join("data"), home.join("config.toml")));
        }
        let data = dirs::data_dir().ok_or(StoreError::NoHome)?.join("dnt");
        let config = dirs::config_dir().ok_or(StoreError::NoHome)?.join("dnt/config.toml");
        Ok(Store::open(data, config))
    }

    pub fn open(data_dir: impl Into<PathBuf>, config_path: impl Into<PathBuf>) -> Store {
        Store::with_fs(Arc::new(OsFs), data_dir, config_path)
    }

    /// A store on another file system (in memory, say).
    pub fn with_fs(fs: Arc<dyn Fs>, data_dir: impl Into<PathBuf>, config_path: impl Into<PathBuf>) -> Store {
        Store { fs, data_dir: data_dir.into(), config_path: config_path.into() }
    }

    pub fn fs(&self) -> &dyn Fs {
        &*self.fs
    }

    /// The file system, to share.
    pub fn shared_fs(&self) -> Arc<dyn Fs> {
        self.fs.clone()
    }

    /// The folder with everything dnt keeps (campaigns, the library).
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn load_config(&self) -> Result<Config> {
        if !self.fs.exists(&self.config_path) {
            return Ok(Config::default());
        }
        read_toml(self.fs(), &self.config_path)
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        write_toml(self.fs(), &self.config_path, config, "config")
    }

    fn campaigns_dir(&self) -> PathBuf {
        self.data_dir.join("campaigns")
    }

    /// All campaigns, sorted by name. Unreadable campaigns are returned as errors
    /// alongside the readable ones.
    pub fn campaigns(&self) -> (Vec<Campaign>, Vec<StoreError>) {
        let mut campaigns = Vec::new();
        let mut errors = Vec::new();
        let dir = self.campaigns_dir();
        let entries = match self.fs.list(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (campaigns, errors),
            Err(source) => return (campaigns, vec![StoreError::Io { path: dir, source }]),
        };
        for (slug, _) in entries {
            if !self.fs.is_file(&dir.join(&slug).join("campaign.toml")) {
                continue;
            }
            match self.load_campaign(&slug) {
                Ok(c) => campaigns.push(c),
                Err(e) => errors.push(e),
            }
        }
        campaigns.sort_by_key(|c| c.settings.name.to_lowercase());
        (campaigns, errors)
    }

    pub fn load_campaign(&self, slug: &str) -> Result<Campaign> {
        let dir = self.campaigns_dir().join(slug);
        let file = dir.join("campaign.toml");
        if !self.fs.is_file(&file) {
            return Err(StoreError::NoSuchCampaign(slug.to_string()));
        }
        Ok(Campaign { slug: slug.to_string(), settings: read_toml(self.fs(), &file)?, dir })
    }

    /// Create a campaign with a unique slug derived from its name.
    pub fn create_campaign(&self, settings: CampaignSettings) -> Result<Campaign> {
        let base = slugify(&settings.name);
        let mut slug = base.clone();
        let mut n = 2;
        while self.fs.exists(&self.campaigns_dir().join(&slug)) {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let campaign = Campaign { dir: self.campaigns_dir().join(&slug), slug, settings };
        for dir in [campaign.characters_dir(), campaign.notes_dir()] {
            self.fs.create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
        }
        self.save_campaign(&campaign)?;
        self.notes(&campaign)?;
        Ok(campaign)
    }

    /// The campaign's notes, created with the default structure if empty.
    pub fn notes(&self, campaign: &Campaign) -> Result<notes::Notes> {
        let config = self.load_config().unwrap_or_default();
        let order = if !campaign.settings.notes.order.is_empty() {
            campaign.settings.notes.order.clone()
        } else if !config.notes_order.is_empty() {
            config.notes_order.clone()
        } else {
            notes::DEFAULT_FOLDERS.iter().map(|s| s.to_string()).collect()
        };
        let n = notes::Notes::new(self.fs.clone(), campaign.notes_dir(), self.data_dir.join("templates"), order);
        n.scaffold(&notes::Vars { campaign: campaign.settings.name.clone(), ..Default::default() })?;
        Ok(n)
    }

    pub fn save_campaign(&self, campaign: &Campaign) -> Result<()> {
        write_toml(self.fs(), &campaign.dir.join("campaign.toml"), &campaign.settings, "campaign")
    }

    /// Delete everything: the data, campaigns, notes and the configuration.
    pub fn reset(&self) -> Result<()> {
        if self.fs.is_dir(&self.data_dir) {
            self.fs.remove_dir_all(&self.data_dir).map_err(|source| StoreError::Io { path: self.data_dir.clone(), source })?;
        }
        if self.fs.is_file(&self.config_path) {
            self.fs.remove_file(&self.config_path).map_err(|source| StoreError::Io { path: self.config_path.clone(), source })?;
        }
        Ok(())
    }

    /// Delete a campaign and everything in it (characters, notes, homebrew).
    pub fn delete_campaign(&self, campaign: &Campaign) -> Result<()> {
        self.fs.remove_dir_all(&campaign.dir).map_err(|source| StoreError::Io { path: campaign.dir.clone(), source })
    }
}

impl Store {
    /// Characters of a campaign, sorted by name; unreadable files are
    /// returned as errors.
    pub fn characters(&self, campaign: &Campaign) -> (Vec<CharacterFile>, Vec<StoreError>) {
        let dir = campaign.characters_dir();
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let entries = match self.fs.list(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (out, errors),
            Err(source) => return (out, vec![StoreError::Io { path: dir, source }]),
        };
        for (name, _) in entries {
            let path = dir.join(name);
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match self.load_character(&path) {
                Ok(c) => out.push(c),
                Err(e) => errors.push(e),
            }
        }
        out.sort_by_key(|c| c.character.name.to_lowercase());
        (out, errors)
    }

    pub fn load_character(&self, path: &Path) -> Result<CharacterFile> {
        let text = self.fs.read_to_string(path).map_err(|source| StoreError::Io { path: path.to_path_buf(), source })?;
        let character = Character::from_toml(&text).map_err(|message| StoreError::Character { path: path.to_path_buf(), message })?;
        Ok(CharacterFile { slug: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(), path: path.to_path_buf(), character })
    }

    /// Save a new character under a unique slug derived from its name.
    pub fn create_character(&self, campaign: &Campaign, character: Character) -> Result<CharacterFile> {
        let dir = campaign.characters_dir();
        let base = slugify(&character.name);
        let mut slug = base.clone();
        let mut n = 2;
        while self.fs.exists(&dir.join(format!("{slug}.toml"))) {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let file = CharacterFile { path: dir.join(format!("{slug}.toml")), slug, character };
        self.save_character(&file)?;
        Ok(file)
    }

    /// Delete a character's file (its notes stay).
    pub fn delete_character(&self, file: &CharacterFile) -> Result<()> {
        self.fs.remove_file(&file.path).map_err(|source| StoreError::Io { path: file.path.clone(), source })
    }

    pub fn save_character(&self, file: &CharacterFile) -> Result<()> {
        write_text(self.fs(), &file.path, &file.character.to_toml())
    }

    /// The file holding a user's overlay for one feature (`id` as in the
    /// overlay, e.g. `feat:Tough|XPHB`).
    pub fn feature_overlay_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("library/overlays").join(format!("{}.toml", slugify(id)))
    }

    /// Write a feature's overlay; empty text removes the file.
    pub fn save_feature_overlay(&self, id: &str, text: &str) -> Result<()> {
        let path = self.feature_overlay_path(id);
        if text.trim().is_empty() {
            return match self.fs.remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(StoreError::Io { path, source: e }),
                _ => Ok(()),
            };
        }
        write_text(self.fs(), &path, text)
    }

    /// A user's overlay for one feature, if there is one.
    pub fn feature_overlay(&self, id: &str) -> Option<String> {
        self.fs.read_to_string(&self.feature_overlay_path(id)).ok()
    }

    /// Read a text file of the store (an overlay, say).
    pub fn read_text(&self, path: &Path) -> Result<String> {
        self.fs.read_to_string(path).map_err(|source| StoreError::Io { path: path.to_path_buf(), source })
    }

    /// Write a text file of the store (an export, say).
    pub fn write_text(&self, path: &Path, text: &str) -> Result<()> {
        write_text(self.fs(), path, text)
    }

    /// User overlay files (`<data>/library/overlays/*.toml`), sorted.
    /// Where the user's rules are.
    pub fn overlays_dir(&self) -> PathBuf {
        self.data_dir.join("library/overlays")
    }

    pub fn overlay_files(&self) -> Vec<PathBuf> {
        let dir = self.overlays_dir();
        let mut files: Vec<PathBuf> = self
            .fs
            .list(&dir)
            .into_iter()
            .flatten()
            .map(|(name, _)| dir.join(name))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        files.sort();
        files
    }
}

fn read_json(fs: &dyn Fs, path: &Path) -> Result<serde_json::Value> {
    let text = fs.read_to_string(path).map_err(|source| StoreError::Io { path: path.to_path_buf(), source })?;
    serde_json::from_str(&text).map_err(|e| StoreError::Json { path: path.to_path_buf(), message: e.to_string() })
}

/// A short source abbreviation from a campaign name: `Curse of Strahd` →
/// `CoS`, `Waterdeep: Dragon Heist` → `WDH`. Small words stay lowercase,
/// as in the source abbreviations of the data (`CoS`, `ToA`).
pub fn source_abbreviation(name: &str) -> String {
    const SMALL: &[&str] = &["of", "the", "and", "a", "an", "in", "on", "to", "for"];
    let abbr: String = name
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            let c = w.chars().next().unwrap();
            if i > 0 && SMALL.contains(&w.to_lowercase().as_str()) {
                c.to_ascii_lowercase()
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    if abbr.is_empty() {
        "HB".into()
    } else {
        abbr
    }
}

/// Source of the global homebrew library (`<data>/library/homebrew/`).
pub const GLOBAL_SOURCE: &str = "Homebrew";

impl Store {
    pub fn global_homebrew_dir(&self) -> PathBuf {
        self.data_dir.join("library/homebrew")
    }

    /// Where imported data lives (see `dnt_core::import`).
    pub fn imported_dir(&self) -> PathBuf {
        self.data_dir.join("library/imported")
    }

    /// Imported homebrew, one file per source, loaded when enabled.
    pub fn imported_homebrew_dir(&self) -> PathBuf {
        self.imported_dir().join("homebrew")
    }

    /// The file dnt writes promoted entries to.
    pub fn global_homebrew_path(&self) -> PathBuf {
        self.global_homebrew_dir().join("homebrew.json")
    }

    /// All global homebrew files (`<data>/library/homebrew/*.json`), sorted.
    pub fn global_homebrew_files(&self) -> Vec<PathBuf> {
        let dir = self.global_homebrew_dir();
        let mut files: Vec<PathBuf> = self
            .fs
            .list(&dir)
            .into_iter()
            .flatten()
            .map(|(name, _)| dir.join(name))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        files.sort();
        files
    }

    /// The global homebrew document dnt writes to.
    pub fn global_homebrew(&self) -> Result<serde_json::Value> {
        let path = self.global_homebrew_path();
        if !self.fs.exists(&path) {
            return Ok(serde_json::json!({
                "_meta": {"sources": [{"json": GLOBAL_SOURCE, "abbreviation": "HB", "full": "Homebrew", "version": "1"}]}
            }));
        }
        read_json(self.fs(), &path)
    }

    pub fn save_global_homebrew(&self, doc: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string_pretty(doc).expect("JSON serialises");
        write_text(self.fs(), &self.global_homebrew_path(), &(text + "\n"))
    }

    /// Point a campaign's characters at a moved entity: every quoted
    /// reference `"from"` (ignoring ASCII case) becomes `"to"`. Returns the
    /// number of files changed.
    pub fn rewrite_refs(&self, campaign: &Campaign, from: &str, to: &str) -> Result<usize> {
        let needle = format!("\"{}\"", from.to_ascii_lowercase());
        let mut changed = 0;
        let dir = campaign.characters_dir();
        let paths = self.fs.list(&dir).into_iter().flatten().map(|(name, _)| dir.join(name));
        for path in paths.filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml")) {
            let text = self.fs.read_to_string(&path).map_err(|source| StoreError::Io { path: path.clone(), source })?;
            let lower = text.to_ascii_lowercase();
            if !lower.contains(&needle) {
                continue;
            }
            let mut out = String::with_capacity(text.len());
            let mut last = 0;
            for (i, _) in lower.match_indices(&needle) {
                out.push_str(&text[last..i]);
                out.push_str(&format!("\"{to}\""));
                last = i + needle.len();
            }
            out.push_str(&text[last..]);
            write_text(self.fs(), &path, &out)?;
            changed += 1;
        }
        Ok(changed)
    }

    /// The campaign's homebrew document (an empty one with the campaign's
    /// source if the file does not exist yet).
    pub fn homebrew(&self, campaign: &Campaign) -> Result<serde_json::Value> {
        let path = campaign.homebrew_path();
        if !self.fs.exists(&path) {
            return Ok(serde_json::json!({
                "_meta": {"sources": [{
                    "json": campaign.settings.source,
                    "abbreviation": campaign.settings.source,
                    "full": format!("{} (campaign)", campaign.settings.name),
                    "version": "1"
                }]}
            }));
        }
        read_json(self.fs(), &path)
    }

    pub fn save_homebrew(&self, campaign: &Campaign, doc: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string_pretty(doc).expect("JSON serialises");
        write_text(self.fs(), &campaign.homebrew_path(), &(text + "\n"))
    }

    /// Give a campaign a homebrew source abbreviation if it has none, avoiding
    /// `taken` ones (lowercase). Returns whether it changed.
    pub fn ensure_source(&self, campaign: &mut Campaign, taken: &dyn Fn(&str) -> bool) -> Result<bool> {
        if !campaign.settings.source.is_empty() {
            return Ok(false);
        }
        // Campaign names often match their adventure's source (CoS):
        // then use `CoS-HB`, then `CoS-HB2`, …
        let base = source_abbreviation(&campaign.settings.name);
        let mut abbr = base.clone();
        let mut n = 1;
        while taken(&abbr.to_lowercase()) {
            abbr = if n == 1 { format!("{base}-HB") } else { format!("{base}-HB{n}") };
            n += 1;
        }
        campaign.settings.source = abbr;
        self.save_campaign(campaign)?;
        Ok(true)
    }
}

/// Lowercase ASCII slug: `Curse of Strahd!` → `curse-of-strahd`.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "campaign".to_string()
    } else {
        out
    }
}

fn read_toml<T: serde::de::DeserializeOwned>(fs: &dyn Fs, path: &Path) -> Result<T> {
    let text = fs.read_to_string(path).map_err(|source| StoreError::Io { path: path.to_path_buf(), source })?;
    dnt_rules::format::check(&text).map_err(|message| StoreError::Format { path: path.to_path_buf(), message })?;
    toml::from_str(&text).map_err(|source| StoreError::Parse { path: path.to_path_buf(), source })
}

fn write_toml<T: Serialize>(fs: &dyn Fs, path: &Path, value: &T, what: &str) -> Result<()> {
    let text = toml::to_string_pretty(value).map_err(|source| StoreError::Serialize { what: what.to_string(), source })?;
    write_text(fs, path, &dnt_rules::format::stamp(&text))
}

fn write_text(fs: &dyn Fs, path: &Path, text: &str) -> Result<()> {
    fs.write(path, text.as_bytes()).map_err(|source| StoreError::Io { path: path.to_path_buf(), source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("data"), dir.path().join("config/config.toml"));
        (dir, store)
    }

    #[test]
    fn abbreviations() {
        assert_eq!(source_abbreviation("Curse of Strahd"), "CoS");
        assert_eq!(source_abbreviation("Waterdeep: Dragon Heist"), "WDH");
        assert_eq!(source_abbreviation("tomb of annihilation"), "ToA");
        assert_eq!(source_abbreviation("!!!"), "HB");
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Curse of Strahd!"), "curse-of-strahd");
        assert_eq!(slugify("  Waterdeep: Dragon Heist "), "waterdeep-dragon-heist");
        assert_eq!(slugify("???"), "campaign");
    }

    #[test]
    fn campaign_roundtrip() {
        let (_tmp, store) = store();
        let settings =
            CampaignSettings { name: "Curse of Strahd".into(), sources: vec!["XPHB".into(), "XDMG".into()], ..Default::default() };
        let c = store.create_campaign(settings.clone()).unwrap();
        assert_eq!(c.slug, "curse-of-strahd");
        assert!(c.characters_dir().is_dir() && c.notes_dir().is_dir());

        let again = store.create_campaign(settings).unwrap();
        assert_eq!(again.slug, "curse-of-strahd-2");

        let (all, errors) = store.campaigns();
        assert!(errors.is_empty());
        assert_eq!(all.len(), 2);
        assert_eq!(store.load_campaign("curse-of-strahd").unwrap(), c);
    }

    #[test]
    fn character_roundtrip() {
        let (_tmp, store) = store();
        let camp = store.create_campaign(CampaignSettings { name: "C".into(), ..Default::default() }).unwrap();
        let ch = Character { name: "Elara Moonwhisper".into(), ..Default::default() };
        let a = store.create_character(&camp, ch.clone()).unwrap();
        let b = store.create_character(&camp, ch).unwrap();
        assert_eq!(a.slug, "elara-moonwhisper");
        assert_eq!(b.slug, "elara-moonwhisper-2");
        let (all, errors) = store.characters(&camp);
        assert!(errors.is_empty());
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].character.name, "Elara Moonwhisper");

        std::fs::write(camp.characters_dir().join("broken.toml"), "name = ").unwrap();
        let (all, errors) = store.characters(&camp);
        assert_eq!((all.len(), errors.len()), (2, 1));
    }

    #[test]
    fn config_roundtrip() {
        let (_tmp, store) = store();
        assert_eq!(store.load_config().unwrap(), Config::default());
        let config = Config {
            library: LibraryConfig { path: Some("/data/dnd".into()), homebrew: vec![] },
            last_campaign: Some("cos".into()),
            notes_order: vec![],
            backup: BackupConfig { every: Every::Day, keep: 3, last: Some("2026-10-29 10:00".into()), folder: Some("/mnt/backups".into()) },
        };
        store.save_config(&config).unwrap();
        assert_eq!(store.load_config().unwrap(), config);
    }

    #[test]
    fn filter_from_campaign() {
        let c = Campaign {
            slug: "x".into(),
            dir: "/x".into(),
            settings: CampaignSettings { name: "X".into(), sources: vec!["XPHB".into()], ..Default::default() },
        };
        let f = c.source_filter();
        assert!(f.source_enabled("xphb") && !f.source_enabled("PHB") && f.hide_reprinted);
        assert!(f.source_enabled("homebrew"), "the global homebrew is always on");
    }

    #[test]
    fn global_homebrew_and_refs() {
        let (_tmp, store) = store();
        let doc = store.global_homebrew().unwrap();
        assert_eq!(doc["_meta"]["sources"][0]["json"], GLOBAL_SOURCE);
        store.save_global_homebrew(&doc).unwrap();
        assert_eq!(store.global_homebrew_files(), vec![store.global_homebrew_path()]);

        let camp = store.create_campaign(CampaignSettings { name: "C".into(), ..Default::default() }).unwrap();
        let ch = Character { name: "A".into(), feats: vec!["feat:Lucky|XPHB".into(), "feat:Grim|C".into()], ..Default::default() };
        let file = store.create_character(&camp, ch).unwrap();
        assert_eq!(store.rewrite_refs(&camp, "feat:grim|c", "feat:Grim|Homebrew").unwrap(), 1);
        let again = store.load_character(&file.path).unwrap();
        assert_eq!(again.character.feats, vec!["feat:Lucky|XPHB", "feat:Grim|Homebrew"]);
        assert_eq!(store.rewrite_refs(&camp, "feat:grim|c", "x").unwrap(), 0);
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;
    use crate::fs::MemFs;

    #[test]
    fn a_store_in_memory() {
        let fs = Arc::new(MemFs::new());
        let store = Store::with_fs(fs.clone(), "/data", "/config/config.toml");
        let camp = store.create_campaign(CampaignSettings { name: "Curse of Strahd".into(), ..Default::default() }).unwrap();
        let file = store.create_character(&camp, Character { name: "Ireena".into(), ..Default::default() }).unwrap();
        store.save_config(&Config { last_campaign: Some(camp.slug.clone()), ..Default::default() }).unwrap();

        // Everything is in memory, and comes back from what was kept.
        let (files, dirs) = fs.contents();
        assert!(files.keys().all(|p| p.starts_with("/data") || p.starts_with("/config")));
        let again = Store::with_fs(Arc::new(MemFs::from_contents(files, dirs)), "/data", "/config/config.toml");
        let (campaigns, errors) = again.campaigns();
        assert!(errors.is_empty());
        assert_eq!(campaigns, std::slice::from_ref(&camp));
        assert_eq!(again.characters(&camp).0, [file]);
        assert_eq!(again.load_config().unwrap().last_campaign.as_deref(), Some("curse-of-strahd"));

        let notes = again.notes(&camp).unwrap();
        assert!(notes.folders().contains(&"NPCs".to_string()), "scaffolded");
        let path = notes.create("NPCs", &notes::Vars { title: "Strahd".into(), ..Default::default() }).unwrap();
        assert!(notes.read(&path).unwrap().contains("Strahd"));
        notes.rename("NPCs", "People").unwrap();
        assert!(notes.exists("People/Strahd.md") && !notes.exists("NPCs"));
    }
}
