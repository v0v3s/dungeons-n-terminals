//! Top-level application state: sections, the active campaign and global keys.

use dnt_data::{Library, SourceFilter};
use dnt_store::{CampaignSettings, Config, Store, PRESETS};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::campaigns::{Action, Campaigns};
use crate::compendium::{Compendium, Outcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Compendium,
    Characters,
    Notes,
    Campaigns,
}

impl Section {
    pub const ALL: [Section; 4] = [Section::Compendium, Section::Characters, Section::Notes, Section::Campaigns];

    pub fn label(self) -> &'static str {
        match self {
            Section::Compendium => "Compendium",
            Section::Characters => "Characters",
            Section::Notes => "Notes",
            Section::Campaigns => "Campaigns",
        }
    }
}

pub struct App {
    pub section: Section,
    pub compendium: Compendium,
    pub campaigns: Campaigns,
    /// Index into `campaigns.list` of the active campaign.
    pub active: Option<usize>,
    /// Sources forced from the command line, overriding the campaign.
    pub override_filter: Option<SourceFilter>,
    pub status: String,
    pub quit: bool,
    store: Store,
    config: Config,
}

impl App {
    pub fn new(lib: &Library, store: Store, config: Config, override_filter: Option<SourceFilter>, status: String) -> App {
        let (list, errors) = store.campaigns();
        let mut status = status;
        if let Some(e) = errors.first() {
            status = format!("Campaign error: {e}");
        }
        let active = config
            .last_campaign
            .as_ref()
            .and_then(|slug| list.iter().position(|c| &c.slug == slug));
        let filter = override_filter
            .clone()
            .or_else(|| active.map(|i| list[i].source_filter()))
            .unwrap_or_default();
        let section = if list.is_empty() { Section::Campaigns } else { Section::Compendium };
        let mut campaigns = Campaigns::new(lib, list);
        campaigns.selected = active.unwrap_or(0);
        App {
            section,
            compendium: Compendium::new(lib, filter),
            campaigns,
            active,
            override_filter,
            status,
            quit: false,
            store,
            config,
        }
    }

    pub fn active_campaign(&self) -> Option<&dnt_store::Campaign> {
        self.active.and_then(|i| self.campaigns.list.get(i))
    }

    fn wants_text(&self) -> bool {
        match self.section {
            Section::Compendium => self.compendium.wants_text(),
            Section::Campaigns => self.campaigns.wants_text(),
            _ => false,
        }
    }

    pub fn handle_key(&mut self, lib: &Library, key: KeyEvent) {
        self.status.clear();
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if !self.wants_text() {
            if let KeyCode::Char(c @ '1'..='9') = key.code {
                if let Some(&s) = Section::ALL.get(c as usize - '1' as usize) {
                    self.section = s;
                }
                return;
            }
        }
        match self.section {
            Section::Compendium => match self.compendium.handle_key(lib, key) {
                Outcome::Quit => self.quit = true,
                Outcome::Status(s) => self.status = s,
                Outcome::None => {}
            },
            Section::Campaigns => {
                let action = self.campaigns.handle_key(key);
                self.campaign_action(lib, action);
            }
            Section::Characters | Section::Notes => {
                if key.code == KeyCode::Char('q') {
                    self.quit = true;
                }
            }
        }
    }

    fn campaign_action(&mut self, lib: &Library, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.quit = true,
            Action::Create(name) => {
                let (_, preset) = PRESETS[0];
                let settings = CampaignSettings {
                    name,
                    sources: preset.iter().map(|s| s.to_string()).collect(),
                    ..Default::default()
                };
                match self.store.create_campaign(settings) {
                    Ok(c) => {
                        let slug = c.slug.clone();
                        self.campaigns.list.push(c);
                        self.campaigns.list.sort_by_key(|c| c.settings.name.to_lowercase());
                        let index = self.campaigns.list.iter().position(|c| c.slug == slug).unwrap();
                        self.campaigns.selected = index;
                        // Indices may have shifted; re-resolve the active campaign.
                        let active_slug = self.active_campaign().map(|c| c.slug.clone());
                        self.active = active_slug.and_then(|s| self.campaigns.list.iter().position(|c| c.slug == s));
                        self.activate(lib, index);
                        self.status = format!("Created campaign {slug} with {}", PRESETS[0].0);
                    }
                    Err(e) => self.status = format!("Could not create campaign: {e}"),
                }
            }
            Action::Activate(i) => self.activate(lib, i),
            Action::Changed(i) => {
                let campaign = &self.campaigns.list[i];
                if let Err(e) = self.store.save_campaign(campaign) {
                    self.status = format!("Could not save campaign: {e}");
                }
                if self.active == Some(i) {
                    self.apply_filter(lib);
                }
            }
        }
    }

    fn activate(&mut self, lib: &Library, index: usize) {
        self.active = Some(index);
        let campaign = &self.campaigns.list[index];
        self.config.last_campaign = Some(campaign.slug.clone());
        let name = campaign.settings.name.clone();
        if let Err(e) = self.store.save_config(&self.config) {
            self.status = format!("Could not save config: {e}");
        } else {
            self.status = format!("Active campaign: {name}");
        }
        self.apply_filter(lib);
    }

    fn apply_filter(&mut self, lib: &Library) {
        if self.override_filter.is_some() {
            self.status = "Sources are fixed by --sources for this session".into();
            return;
        }
        let filter = self.active_campaign().map(|c| c.source_filter()).unwrap_or_default();
        self.compendium.set_filter(lib, filter);
    }
}
