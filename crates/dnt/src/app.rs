//! Top-level application state: sections, the active campaign and global keys.

use dnt_data::{Library, SourceFilter};
use dnt_store::{CampaignSettings, Config, Store, PRESETS};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use std::path::PathBuf;

use dnt_rules::builtin;
use dnt_rules::overlay::{Overlay, OverlaySet};

use crate::campaigns::{Action, Campaigns};
use crate::characters::{self, Characters};
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
    pub characters: Characters,
    pub overlays: OverlaySet,
    /// A file to open in $EDITOR; handled by the main loop.
    pub pending_edit: Option<PathBuf>,
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
        let (overlays, problems) = load_overlays(&store, lib);
        if problems > 0 && status.is_empty() {
            status = format!("{problems} overlay problem(s) — run the check_overlays example");
        }
        let files = active.map(|i| store.characters(&list[i]).0).unwrap_or_default();
        let mut campaigns = Campaigns::new(lib, list);
        campaigns.selected = active.unwrap_or(0);
        let mut characters = Characters::new(files);
        characters.filter = filter.clone();
        App {
            section,
            compendium: Compendium::new(lib, filter),
            campaigns,
            active,
            override_filter,
            characters,
            overlays,
            pending_edit: None,
            status,
            quit: false,
            store,
            config,
        }
    }

    /// Re-read the character file after it was edited externally.
    pub fn after_edit(&mut self, lib: &Library, path: &std::path::Path) {
        match self.store.load_character(path) {
            Ok(file) => {
                self.characters.reload(file, lib, &self.overlays);
                self.status = "Reloaded character".into();
            }
            Err(e) => self.status = format!("Could not reload: {e}"),
        }
    }

    pub fn active_campaign(&self) -> Option<&dnt_store::Campaign> {
        self.active.and_then(|i| self.campaigns.list.get(i))
    }

    fn wants_text(&self) -> bool {
        match self.section {
            Section::Compendium => self.compendium.wants_text(),
            Section::Campaigns => self.campaigns.wants_text(),
            Section::Characters => self.characters.wants_text(),
            _ => false,
        }
    }

    pub fn handle_key(&mut self, lib: &Library, key: KeyEvent) {
        self.status.clear();
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if !self.wants_text() || alt {
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
            Section::Characters => {
                let filter = self.compendium.filter.clone();
                let action = self.characters.handle_key(key, lib, &self.overlays, &filter);
                self.character_action(lib, action);
            }
            Section::Notes => {
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

    fn character_action(&mut self, lib: &Library, action: characters::Action) {
        use characters::Action as A;
        match action {
            A::None => {}
            A::Quit => self.quit = true,
            A::Status(s) => self.status = s,
            A::Edit(path) => self.pending_edit = Some(path),
            A::Save(status) => {
                if let Some(sheet) = &self.characters.sheet {
                    match self.store.save_character(&sheet.file) {
                        Ok(()) => self.status = status,
                        Err(e) => self.status = format!("Could not save: {e}"),
                    }
                }
            }
            A::Create(character) => {
                let Some(campaign) = self.active_campaign().cloned() else {
                    self.status = "Create or activate a campaign first (4)".into();
                    return;
                };
                match self.store.create_character(&campaign, *character) {
                    Ok(file) => {
                        self.characters.files.push(file.clone());
                        self.characters.files.sort_by_key(|f| f.character.name.to_lowercase());
                        let i = self.characters.files.iter().position(|f| f.path == file.path).unwrap();
                        self.characters.open(i, lib, &self.overlays);
                        self.status = "Character created".into();
                    }
                    Err(e) => self.status = format!("Could not create character: {e}"),
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
        let filter = self.characters.filter.clone();
        self.characters = Characters::new(self.store.characters(&self.campaigns.list[index]).0);
        self.characters.filter = filter;
    }

    fn apply_filter(&mut self, lib: &Library) {
        if self.override_filter.is_some() {
            self.status = "Sources are fixed by --sources for this session".into();
            return;
        }
        let filter = self.active_campaign().map(|c| c.source_filter()).unwrap_or_default();
        self.characters.filter = filter.clone();
        self.compendium.set_filter(lib, filter);
    }
}

/// Built-in overlays plus the user's own, and the number of problems found.
fn load_overlays(store: &Store, lib: &Library) -> (OverlaySet, usize) {
    let (mut set, diags) = builtin::overlays();
    let mut problems = diags.len();
    for path in store.overlay_files() {
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Overlay::parse(&t)) {
            Ok((overlay, diags)) => {
                problems += diags.len() + overlay.unresolved(lib).len();
                set.add(overlay);
            }
            Err(_) => problems += 1,
        }
    }
    (set, problems)
}
