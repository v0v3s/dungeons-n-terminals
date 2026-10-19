//! Everything a frontend works within: the configuration, the campaigns
//! and the active one, and the library as the active campaign sees it.
//!
//! The library is built in layers: the data (imported, or a data folder
//! from the configuration), then the global homebrew, then the active
//! campaign's homebrew; the campaign's sources decide what shows.

use dnt_data::{Library, SourceFilter};
use dnt_rules::builtin;
use dnt_rules::overlay::{Overlay, OverlaySet};
use dnt_store::notes::Notes;
use dnt_store::{Campaign, CampaignSettings, CharacterFile, Config, Store};

use crate::import;

pub struct Workspace {
    pub store: Store,
    pub config: Config,
    /// The data and the global homebrew (no campaign).
    base: Option<Library>,
    /// The base with the active campaign's homebrew; what everything reads.
    pub lib: Option<Library>,
    /// Built-in rules and the user's own.
    pub overlays: OverlaySet,
    /// Campaigns, sorted by name.
    pub campaigns: Vec<Campaign>,
    pub active: Option<usize>,
    /// Problems found while loading, for a status line.
    pub problems: Vec<String>,
}

impl Workspace {
    /// Open the store: configuration, campaigns, the last active one, and
    /// the library if there is data.
    pub fn open(store: Store) -> Workspace {
        let mut problems = Vec::new();
        let config = store.load_config().unwrap_or_else(|e| {
            problems.push(format!("Configuration: {e}"));
            Config::default()
        });
        let (campaigns, errors) = store.campaigns();
        problems.extend(errors.iter().map(|e| format!("Campaign: {e}")));
        let active = config.last_campaign.as_ref().and_then(|s| campaigns.iter().position(|c| &c.slug == s));
        let mut ws = Workspace { store, config, base: None, lib: None, overlays: OverlaySet::default(), campaigns, active, problems };
        ws.reload();
        ws
    }

    /// Whether there is data to read.
    pub fn has_data(&self) -> bool {
        self.lib.is_some()
    }

    /// Rebuild the library from scratch (after an import, or a change to the
    /// global homebrew or the rules).
    pub fn reload(&mut self) {
        self.base = match self.load_base() {
            Ok(b) => b,
            Err(e) => {
                self.problems.push(e);
                None
            }
        };
        self.overlays = self.load_overlays();
        self.prepare();
    }

    fn load_base(&self) -> Result<Option<Library>, String> {
        let mut base = match &self.config.library.path {
            Some(dir) => Library::load_dir(dir).map_err(|e| e.to_string())?,
            None if import::has_imported(&self.store) => import::load(&self.store)?,
            None => return Ok(None),
        };
        for path in self.store.global_homebrew_files() {
            let text = self.store.read_text(&path).map_err(|e| e.to_string())?;
            let doc = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            base.load_json(&doc);
        }
        Ok(Some(base))
    }

    fn load_overlays(&mut self) -> OverlaySet {
        let (mut set, diags) = builtin::overlays();
        let mut problems = diags.len();
        for path in self.store.overlay_files() {
            match self.store.read_text(&path).map_err(|e| e.to_string()).and_then(|t| Overlay::parse(&t)) {
                Ok((overlay, diags)) => {
                    problems += diags.len() + self.base.as_ref().map_or(0, |b| overlay.unresolved(b).len());
                    // A user's overlay replaces the built-in one for its feature.
                    set.replace(overlay);
                }
                Err(_) => problems += 1,
            }
        }
        if problems > 0 {
            self.problems.push(format!("{problems} problem(s) in the rules"));
        }
        set
    }

    /// Layer the active campaign's homebrew on the base.
    pub fn prepare(&mut self) {
        let Some(base) = &self.base else {
            self.lib = None;
            return;
        };
        let mut lib = base.clone();
        if let Some(i) = self.active {
            let taken = |s: &str| base.source_info(s).is_some();
            if let Err(e) = self.store.ensure_source(&mut self.campaigns[i], &taken) {
                self.problems.push(format!("Could not save the campaign: {e}"));
            }
            match self.store.homebrew(&self.campaigns[i]) {
                Ok(doc) => self.problems.extend(lib.apply_campaign(&doc).into_iter().map(|p| format!("Campaign homebrew: {p}"))),
                Err(e) => self.problems.push(format!("Campaign homebrew: {e}")),
            }
        }
        self.lib = Some(lib);
    }

    pub fn active_campaign(&self) -> Option<&Campaign> {
        self.active.and_then(|i| self.campaigns.get(i))
    }

    /// What the active campaign shows (everything without one).
    pub fn filter(&self) -> SourceFilter {
        self.active_campaign().map(Campaign::source_filter).unwrap_or_default()
    }

    fn find(&self, slug: &str) -> Result<usize, String> {
        self.campaigns.iter().position(|c| c.slug == slug).ok_or_else(|| format!("No campaign {slug}"))
    }

    /// Make a campaign the active one (remembered for next time).
    pub fn activate(&mut self, slug: &str) -> Result<String, String> {
        let i = self.find(slug)?;
        self.active = Some(i);
        self.config.last_campaign = Some(slug.to_string());
        self.store.save_config(&self.config).map_err(|e| e.to_string())?;
        self.prepare();
        Ok(format!("Active campaign: {}", self.campaigns[i].settings.name))
    }

    /// Create a campaign; the first one becomes active.
    pub fn create_campaign(&mut self, name: &str, sources: Vec<String>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("A campaign needs a name".into());
        }
        let settings = CampaignSettings { name: name.to_string(), sources, ..Default::default() };
        let c = self.store.create_campaign(settings).map_err(|e| e.to_string())?;
        let slug = c.slug.clone();
        self.campaigns.push(c);
        self.campaigns.sort_by_key(|c| c.settings.name.to_lowercase());
        let active = self.active_campaign().map(|c| c.slug.clone());
        self.active = active.and_then(|s| self.campaigns.iter().position(|c| c.slug == s));
        if self.active.is_none() {
            self.activate(&slug)?;
        }
        Ok(slug)
    }

    /// Change a campaign's sources (empty: all).
    pub fn set_sources(&mut self, slug: &str, sources: Vec<String>, show_reprinted: bool) -> Result<(), String> {
        let i = self.find(slug)?;
        let c = &mut self.campaigns[i];
        c.settings.sources = sources;
        c.settings.show_reprinted = show_reprinted;
        self.store.save_campaign(c).map_err(|e| e.to_string())
    }

    /// Rename a campaign (its folder keeps its slug).
    pub fn rename_campaign(&mut self, slug: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("A campaign needs a name".into());
        }
        let i = self.find(slug)?;
        self.campaigns[i].settings.name = name.to_string();
        self.store.save_campaign(&self.campaigns[i]).map_err(|e| e.to_string())?;
        let active = self.active_campaign().map(|c| c.slug.clone());
        self.campaigns.sort_by_key(|c| c.settings.name.to_lowercase());
        self.active = active.and_then(|s| self.campaigns.iter().position(|c| c.slug == s));
        Ok(())
    }

    /// The active campaign's characters, sorted by name.
    pub fn characters(&self) -> Vec<CharacterFile> {
        self.active_campaign().map(|c| self.store.characters(c).0).unwrap_or_default()
    }

    /// The active campaign's notes.
    pub fn notes(&self) -> Option<Notes> {
        self.active_campaign().and_then(|c| self.store.notes(c).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_data::source::MemorySource;
    use dnt_store::fs::MemFs;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn campaigns_layer_their_homebrew() {
        let store = Store::with_fs(Arc::new(MemFs::new()), "/data", "/config.toml");
        let mut ws = Workspace::open(store);
        assert!(!ws.has_data(), "nothing imported yet");

        let mut src = MemorySource::default();
        src.insert("x.json", json!({"item": [{"name": "Rope", "source": "B"}, {"name": "Lute", "source": "C"}]}).to_string());
        import::import_files(&ws.store, &src).unwrap();
        ws.reload();
        assert_eq!(ws.lib.as_ref().unwrap().entities().len(), 2);

        let slug = ws.create_campaign("Curse of Strahd", vec!["B".into()]).unwrap();
        assert_eq!(ws.active_campaign().map(|c| c.slug.as_str()), Some(slug.as_str()), "the first campaign becomes active");
        let lib = ws.lib.as_ref().unwrap();
        let shown: Vec<&str> = lib.entities().iter().filter(|e| ws.filter().allows(e)).map(|e| e.name.as_str()).collect();
        assert!(shown.contains(&"Rope") && !shown.contains(&"Lute"), "{shown:?}");

        // Campaign homebrew joins the library.
        let mut doc = ws.store.homebrew(ws.active_campaign().unwrap()).unwrap();
        doc["item"] = json!([{"name": "Holy Symbol of Ravenkind", "source": ws.active_campaign().unwrap().settings.source}]);
        ws.store.save_homebrew(ws.active_campaign().unwrap(), &doc).unwrap();
        ws.prepare();
        assert!(ws.lib.as_ref().unwrap().entities().iter().any(|e| e.name == "Holy Symbol of Ravenkind"));

        // Reopening remembers the active campaign.
        let again = Workspace::open(ws.store);
        assert_eq!(again.active_campaign().map(|c| c.settings.name.as_str()), Some("Curse of Strahd"));
    }
}
