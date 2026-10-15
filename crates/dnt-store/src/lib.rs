//! On-disk storage for configuration and campaigns.
//!
//! ```text
//! <data>/campaigns/<slug>/campaign.toml
//!                        /characters/
//!                        /notes/
//! <config>/config.toml
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub mod notes;

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
    Json { path: PathBuf, message: String },
    #[error("no campaign named {0:?}")]
    NoSuchCampaign(String),
    #[error("could not determine the user's data/config directories")]
    NoHome,
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Named source presets offered when choosing a campaign's books.
pub const PRESETS: &[(&str, &[&str])] = &[
    ("2024 Core", &["XPHB", "XDMG", "XMM"]),
    ("2014 Core", &["PHB", "DMG", "MM"]),
    (
        "2014 Core + Supplements",
        &["PHB", "DMG", "MM", "XGE", "TCE", "MPMM", "FTD", "SCAG", "BMT"],
    ),
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LibraryConfig {
    /// Path to a data data checkout.
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
    /// homebrew source is always enabled.
    pub fn source_filter(&self) -> dnt_data::SourceFilter {
        let mut filter = if self.settings.sources.is_empty() {
            dnt_data::SourceFilter::default()
        } else {
            let mut sources = self.settings.sources.clone();
            if !self.settings.source.is_empty() {
                sources.push(self.settings.source.clone());
            }
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
    data_dir: PathBuf,
    config_path: PathBuf,
}

impl Store {
    /// Use the platform's standard locations (`~/.local/share/dnt`,
    /// `~/.config/dnt/config.toml` on Linux).
    pub fn open_default() -> Result<Store> {
        let data = dirs::data_dir().ok_or(StoreError::NoHome)?.join("dnt");
        let config = dirs::config_dir().ok_or(StoreError::NoHome)?.join("dnt/config.toml");
        Ok(Store::open(data, config))
    }

    pub fn open(data_dir: impl Into<PathBuf>, config_path: impl Into<PathBuf>) -> Store {
        Store {
            data_dir: data_dir.into(),
            config_path: config_path.into(),
        }
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn load_config(&self) -> Result<Config> {
        if !self.config_path.exists() {
            return Ok(Config::default());
        }
        read_toml(&self.config_path)
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        write_toml(&self.config_path, config, "config")
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
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (campaigns, errors),
            Err(source) => return (campaigns, vec![StoreError::Io { path: dir, source }]),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.join("campaign.toml").is_file() {
                continue;
            }
            let slug = entry.file_name().to_string_lossy().into_owned();
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
        if !file.is_file() {
            return Err(StoreError::NoSuchCampaign(slug.to_string()));
        }
        Ok(Campaign {
            slug: slug.to_string(),
            settings: read_toml(&file)?,
            dir,
        })
    }

    /// Create a campaign with a unique slug derived from its name.
    pub fn create_campaign(&self, settings: CampaignSettings) -> Result<Campaign> {
        let base = slugify(&settings.name);
        let mut slug = base.clone();
        let mut n = 2;
        while self.campaigns_dir().join(&slug).exists() {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let campaign = Campaign {
            dir: self.campaigns_dir().join(&slug),
            slug,
            settings,
        };
        for dir in [campaign.characters_dir(), campaign.notes_dir()] {
            fs::create_dir_all(&dir).map_err(|source| StoreError::Io { path: dir, source })?;
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
        let n = notes::Notes::new(campaign.notes_dir(), self.data_dir.join("templates"), order);
        n.scaffold(&notes::Vars {
            campaign: campaign.settings.name.clone(),
            ..Default::default()
        })?;
        Ok(n)
    }

    pub fn save_campaign(&self, campaign: &Campaign) -> Result<()> {
        write_toml(&campaign.dir.join("campaign.toml"), &campaign.settings, "campaign")
    }
}

impl Store {
    /// Characters of a campaign, sorted by name; unreadable files are
    /// returned as errors.
    pub fn characters(&self, campaign: &Campaign) -> (Vec<CharacterFile>, Vec<StoreError>) {
        let dir = campaign.characters_dir();
        let mut out = Vec::new();
        let mut errors = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return (out, errors),
            Err(source) => return (out, vec![StoreError::Io { path: dir, source }]),
        };
        for entry in entries.flatten() {
            let path = entry.path();
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
        let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let character = Character::from_toml(&text).map_err(|message| StoreError::Character {
            path: path.to_path_buf(),
            message,
        })?;
        Ok(CharacterFile {
            slug: path.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
            path: path.to_path_buf(),
            character,
        })
    }

    /// Save a new character under a unique slug derived from its name.
    pub fn create_character(&self, campaign: &Campaign, character: Character) -> Result<CharacterFile> {
        let dir = campaign.characters_dir();
        let base = slugify(&character.name);
        let mut slug = base.clone();
        let mut n = 2;
        while dir.join(format!("{slug}.toml")).exists() {
            slug = format!("{base}-{n}");
            n += 1;
        }
        let file = CharacterFile {
            path: dir.join(format!("{slug}.toml")),
            slug,
            character,
        };
        self.save_character(&file)?;
        Ok(file)
    }

    pub fn save_character(&self, file: &CharacterFile) -> Result<()> {
        write_text(&file.path, &file.character.to_toml())
    }

    /// User overlay files (`<data>/library/overlays/*.toml`), sorted.
    pub fn overlay_files(&self) -> Vec<PathBuf> {
        let dir = self.data_dir.join("library/overlays");
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        files.sort();
        files
    }
}

/// A short source abbreviation from a campaign name: `Curse of Strahd` →
/// `CoS`, `Waterdeep: Dragon Heist` → `WDH`. Small words stay lowercase,
/// as in data (`CoS`, `ToA`).
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

impl Store {
    /// The campaign's homebrew document (an empty one with the campaign's
    /// source if the file does not exist yet).
    pub fn homebrew(&self, campaign: &Campaign) -> Result<serde_json::Value> {
        let path = campaign.homebrew_path();
        if !path.exists() {
            return Ok(serde_json::json!({
                "_meta": {"sources": [{
                    "json": campaign.settings.source,
                    "abbreviation": campaign.settings.source,
                    "full": format!("{} (campaign)", campaign.settings.name),
                    "version": "1"
                }]}
            }));
        }
        let text = fs::read_to_string(&path).map_err(|source| StoreError::Io { path: path.clone(), source })?;
        serde_json::from_str(&text).map_err(|e| StoreError::Json {
            path,
            message: e.to_string(),
        })
    }

    pub fn save_homebrew(&self, campaign: &Campaign, doc: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string_pretty(doc).expect("JSON serialises");
        write_text(&campaign.homebrew_path(), &(text + "\n"))
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

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| StoreError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn write_toml<T: Serialize>(path: &Path, value: &T, what: &str) -> Result<()> {
    let text = toml::to_string_pretty(value).map_err(|source| StoreError::Serialize {
        what: what.to_string(),
        source,
    })?;
    write_text(path, &text)
}

/// Write via a temporary file and rename, so a crash never leaves a
/// half-written file.
fn write_text(path: &Path, text: &str) -> Result<()> {
    let io_err = |source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)
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
        let settings = CampaignSettings {
            name: "Curse of Strahd".into(),
            sources: vec!["XPHB".into(), "XDMG".into()],
            ..Default::default()
        };
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
        let camp = store
            .create_campaign(CampaignSettings {
                name: "C".into(),
                ..Default::default()
            })
            .unwrap();
        let ch = Character {
            name: "Elara Moonwhisper".into(),
            ..Default::default()
        };
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
            library: LibraryConfig {
                path: Some("/data/dnd".into()),
                homebrew: vec![],
            },
            last_campaign: Some("cos".into()),
            notes_order: vec![],
        };
        store.save_config(&config).unwrap();
        assert_eq!(store.load_config().unwrap(), config);
    }

    #[test]
    fn filter_from_campaign() {
        let c = Campaign {
            slug: "x".into(),
            dir: "/x".into(),
            settings: CampaignSettings {
                name: "X".into(),
                sources: vec!["XPHB".into()],
                ..Default::default()
            },
        };
        let f = c.source_filter();
        assert!(f.source_enabled("xphb") && !f.source_enabled("PHB") && f.hide_reprinted);
    }
}
