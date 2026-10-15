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
use crate::entry_editor::EntryEditor;
use dnt_data::homebrew;
use crate::notes_view::{self, NotesView};

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

pub use crate::app_homebrew::{CloneFlow, NewFlow};

pub struct App {
    pub section: Section,
    pub compendium: Compendium,
    pub campaigns: Campaigns,
    /// Index into `campaigns.list` of the active campaign.
    pub active: Option<usize>,
    /// Sources forced from the command line, overriding the campaign.
    pub override_filter: Option<SourceFilter>,
    pub characters: Characters,
    pub notes: NotesView,
    /// Quick capture text, while the capture box is open.
    pub capture: Option<String>,
    /// Editing a campaign homebrew entity.
    pub editor: Option<EntryEditor>,
    /// Choosing how to clone an entity.
    pub clone_flow: Option<CloneFlow>,
    /// Creating a new homebrew entity.
    pub new_flow: Option<NewFlow>,
    /// The active campaign's homebrew document.
    pub homebrew: Option<serde_json::Value>,
    /// The global homebrew document dnt writes to.
    pub global_homebrew: Option<serde_json::Value>,
    /// Section to return to when the editor closes.
    pub editor_return: Option<Section>,
    /// A homebrew entry being edited in $EDITOR (a temporary file).
    pub entry_edit_file: Option<PathBuf>,
    pub overlays: OverlaySet,
    /// A file to open in $EDITOR; handled by the main loop.
    pub pending_edit: Option<PathBuf>,
    /// The library must be rebuilt (campaign switched, homebrew changed).
    pub pending_reload: bool,
    /// The base library must be reloaded (global homebrew changed).
    pub pending_base_reload: bool,
    pub status: String,
    pub quit: bool,
    pub store: Store,
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
        let notes = notes_for(&store, active.map(|i| &list[i]));
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
            notes,
            capture: None,
            editor: None,
            clone_flow: None,
            new_flow: None,
            homebrew: None,
            global_homebrew: None,
            editor_return: None,
            entry_edit_file: None,
            overlays,
            pending_edit: None,
            pending_reload: false,
            pending_base_reload: false,
            status,
            quit: false,
            store,
            config,
        }
    }

    /// Build the library for the active campaign: the base library plus the
    /// campaign's homebrew (giving the campaign a source if it has none).
    pub fn prepare_library(&mut self, base: &Library) -> Library {
        let mut lib = base.clone();
        self.global_homebrew = self.store.global_homebrew().ok();
        let Some(i) = self.active else { return lib };
        let taken = |s: &str| base.source_info(s).is_some();
        if let Err(e) = self.store.ensure_source(&mut self.campaigns.list[i], &taken) {
            self.status = format!("Could not save campaign: {e}");
        }
        match self.store.homebrew(&self.campaigns.list[i]) {
            Ok(doc) => {
                let problems = lib.apply_campaign(&doc);
                if let Some(p) = problems.first() {
                    self.status = format!("Campaign homebrew: {p}");
                }
                self.homebrew = Some(doc);
            }
            Err(e) => self.status = format!("Campaign homebrew: {e}"),
        }
        lib
    }

    /// Refresh everything derived from the library after it was rebuilt.
    pub fn library_reloaded(&mut self, lib: &Library) {
        let filter = self
            .override_filter
            .clone()
            .or_else(|| self.active_campaign().map(|c| c.source_filter()))
            .unwrap_or_default();
        let (category, query) = (self.compendium.category, self.compendium.query.clone());
        self.compendium = Compendium::new(lib, filter.clone());
        self.compendium.restore(lib, category, query);
        let (selected, pane) = (self.campaigns.selected, self.campaigns.pane);
        let list = std::mem::take(&mut self.campaigns.list);
        self.campaigns = Campaigns::new(lib, list);
        self.campaigns.selected = selected;
        self.campaigns.pane = pane;
        self.characters.filter = filter;
        self.characters.refresh(lib, &self.overlays);
    }

    /// Re-read the character file (or homebrew entry) after it was edited externally.
    pub fn after_edit(&mut self, lib: &Library, path: &std::path::Path) {
        if self.entry_edit_file.as_deref() == Some(path) {
            return self.after_entry_edit(lib, path);
        }
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
        if self.capture.is_some()
            || self.clone_flow.is_some()
            || self.new_flow.as_ref().is_some_and(|f| f.name.is_some())
            || self.editor.as_ref().is_some_and(EntryEditor::wants_text)
        {
            return true;
        }
        match self.section {
            Section::Compendium => self.compendium.wants_text(),
            Section::Campaigns => self.campaigns.wants_text(),
            Section::Characters => self.characters.wants_text(),
            Section::Notes => self.notes.wants_text(),
        }
    }

    pub fn handle_key(&mut self, lib: &Library, key: KeyEvent) {
        self.status.clear();
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        if let Some(text) = self.capture.as_mut() {
            match key.code {
                KeyCode::Esc => self.capture = None,
                KeyCode::Enter => {
                    let text = std::mem::take(text);
                    self.capture = None;
                    if !text.trim().is_empty() {
                        self.status = self.notes.capture(&text);
                    }
                }
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => text.push(c),
                _ => {}
            }
            return;
        }
        if alt && key.code == KeyCode::Char('c') {
            if self.notes.notes.is_some() {
                self.capture = Some(String::new());
            } else {
                self.status = "Quick capture needs an active campaign".into();
            }
            return;
        }
        if !self.wants_text() || alt {
            if let KeyCode::Char(c @ '1'..='9') = key.code {
                if let Some(&s) = Section::ALL.get(c as usize - '1' as usize) {
                    self.section = s;
                }
                return;
            }
        }
        if self.new_flow.is_some() {
            return self.new_key(lib, key);
        }
        match self.section {
            Section::Compendium if self.clone_flow.is_some() => self.clone_key(lib, key),
            Section::Compendium if self.editor.is_some() => self.editor_key(lib, key),
            Section::Compendium => match self.compendium.handle_key(lib, key) {
                Outcome::Quit => self.quit = true,
                Outcome::Status(s) => self.status = s,
                Outcome::None => {}
                Outcome::Clone(id) => {
                    if self.active.is_none() {
                        self.status = "Cloning needs an active campaign (4)".into();
                    } else {
                        self.clone_flow = Some(CloneFlow { entity: id, name: None, cursor: 0 });
                    }
                }
                Outcome::Edit(id) => self.start_edit(lib, id),
                Outcome::New => self.start_new(homebrew::Template::ALL, false),
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
                if key.code == KeyCode::Char('q') && !self.notes.wants_text() {
                    let _ = self.notes.save();
                    self.quit = true;
                    return;
                }
                let filter = self.compendium.filter.clone();
                match self.notes.handle_key(key, lib, &filter) {
                    notes_view::Action::None => {}
                    notes_view::Action::Status(s) => self.status = s,
                    notes_view::Action::Compendium(e) => {
                        self.section = Section::Compendium;
                        self.compendium.show(lib, e);
                    }
                    notes_view::Action::Order(order) => {
                        if let Some(i) = self.active {
                            self.campaigns.list[i].settings.notes.order = order;
                            if let Err(e) = self.store.save_campaign(&self.campaigns.list[i]) {
                                self.status = format!("Could not save folder order: {e}");
                            }
                        }
                    }
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
            A::CreateItem => self.start_new(homebrew::Template::ITEMS, true),
            A::OpenNote(name) => {
                self.section = Section::Notes;
                self.status = match self.notes.open_character(&name) {
                    notes_view::Action::Status(s) => s,
                    _ => format!("Notes for {name}"),
                };
            }
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
        // The library carries the active campaign's homebrew.
        self.pending_reload = true;
        let filter = self.characters.filter.clone();
        self.characters = Characters::new(self.store.characters(&self.campaigns.list[index]).0);
        self.characters.filter = filter;
        let _ = self.notes.save();
        self.notes = notes_for(&self.store, Some(&self.campaigns.list[index]));
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

fn notes_for(store: &Store, campaign: Option<&dnt_store::Campaign>) -> NotesView {
    match campaign {
        Some(c) => NotesView::new(store.notes(c).ok(), c.settings.name.clone()),
        None => NotesView::new(None, String::new()),
    }
}
