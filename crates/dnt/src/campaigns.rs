//! Campaign management screen: campaign list and per-campaign source picker.

use std::collections::BTreeMap;

use dnt_data::Library;
use dnt_store::{Campaign, CampaignSettings, PRESETS};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Campaigns,
    Sources,
}

/// A selectable source row.
pub struct SourceRow {
    pub id: String,
    pub name: String,
    pub group: String,
    pub count: usize,
}

pub enum Action {
    None,
    Quit,
    /// Create a campaign with this name.
    Create(String),
    /// Make the campaign at this index active.
    Activate(usize),
    /// The given campaign's settings changed and should be saved/applied.
    Changed(usize),
}

pub struct Campaigns {
    pub list: Vec<Campaign>,
    pub selected: usize,
    pub pane: Pane,
    /// `Some` while typing a new campaign's name.
    pub name_input: Option<String>,
    pub sources: Vec<SourceRow>,
    pub source_selected: usize,
    pub source_offset: usize,
    pub source_filter: String,
    pub filtering: bool,
    pub preset: usize,
    pub source_height: usize,
}

const GROUP_ORDER: &[&str] = &["core", "supplement", "setting", "adventure", "homebrew"];

fn group_label(group: &str) -> String {
    match group {
        "core" => "Core".into(),
        "supplement" | "supplement-alt" => "Supplements".into(),
        "setting" | "setting-alt" => "Settings".into(),
        "adventure" => "Adventures".into(),
        "homebrew" => "Homebrew".into(),
        _ => "Other".into(),
    }
}

impl Campaigns {
    pub fn new(lib: &Library, list: Vec<Campaign>) -> Campaigns {
        let counts = lib.entity_counts();
        let mut sources: Vec<SourceRow> = counts
            .into_iter()
            .map(|(id, count)| {
                let info = lib.source_info(&id);
                SourceRow {
                    name: info.map_or_else(|| id.clone(), |i| i.name.clone()),
                    group: group_label(info.and_then(|i| i.group.as_deref()).unwrap_or("")),
                    id,
                    count,
                }
            })
            .collect();
        let rank = |g: &str| {
            GROUP_ORDER
                .iter()
                .position(|o| group_label(o) == g)
                .unwrap_or(GROUP_ORDER.len())
        };
        sources.sort_by(|a, b| rank(&a.group).cmp(&rank(&b.group)).then(a.name.cmp(&b.name)));
        Campaigns {
            list,
            selected: 0,
            pane: Pane::Campaigns,
            name_input: None,
            sources,
            source_selected: 0,
            source_offset: 0,
            source_filter: String::new(),
            filtering: false,
            preset: 0,
            source_height: 0,
        }
    }

    pub fn wants_text(&self) -> bool {
        self.name_input.is_some() || self.filtering
    }

    /// Indices into `sources` matching the filter.
    pub fn visible_sources(&self) -> Vec<usize> {
        let f = self.source_filter.to_lowercase();
        (0..self.sources.len())
            .filter(|&i| {
                let s = &self.sources[i];
                f.is_empty() || s.name.to_lowercase().contains(&f) || s.id.to_lowercase().contains(&f)
            })
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if let Some(name) = self.name_input.as_mut() {
            match key.code {
                KeyCode::Esc => self.name_input = None,
                KeyCode::Enter => {
                    let name = name.trim().to_string();
                    self.name_input = None;
                    if !name.is_empty() {
                        return Action::Create(name);
                    }
                }
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => name.push(c),
                _ => {}
            }
            return Action::None;
        }
        if self.filtering {
            match key.code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.source_filter.clear();
                }
                KeyCode::Enter => self.filtering = false,
                KeyCode::Backspace => {
                    self.source_filter.pop();
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => self.source_filter.push(c),
                _ => {}
            }
            self.source_selected = 0;
            self.source_offset = 0;
            return Action::None;
        }

        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('n') => {
                self.name_input = Some(String::new());
                return Action::None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.pane = match self.pane {
                    Pane::Campaigns if !self.list.is_empty() => Pane::Sources,
                    _ => Pane::Campaigns,
                };
                return Action::None;
            }
            _ => {}
        }
        match self.pane {
            Pane::Campaigns => self.key_campaigns(key),
            Pane::Sources => self.key_sources(key),
        }
    }

    fn key_campaigns(&mut self, key: KeyEvent) -> Action {
        let n = self.list.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if n > 0 => self.selected = (self.selected + 1).min(n - 1),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Enter if n > 0 => return Action::Activate(self.selected),
            KeyCode::Char('l') | KeyCode::Right if n > 0 => self.pane = Pane::Sources,
            _ => {}
        }
        Action::None
    }

    fn key_sources(&mut self, key: KeyEvent) -> Action {
        let visible = self.visible_sources();
        let n = visible.len();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let half = (self.source_height / 2).max(1);
        let Some(campaign) = self.list.get_mut(self.selected) else {
            return Action::None;
        };
        let settings: &mut CampaignSettings = &mut campaign.settings;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if n > 0 => self.source_selected = (self.source_selected + 1).min(n - 1),
            KeyCode::Char('k') | KeyCode::Up => self.source_selected = self.source_selected.saturating_sub(1),
            KeyCode::Char('d') if ctrl && n > 0 => self.source_selected = (self.source_selected + half).min(n - 1),
            KeyCode::Char('u') if ctrl => self.source_selected = self.source_selected.saturating_sub(half),
            KeyCode::Char('g') => self.source_selected = 0,
            KeyCode::Char('G') => self.source_selected = n.saturating_sub(1),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => self.pane = Pane::Campaigns,
            KeyCode::Char('/') => {
                self.filtering = true;
                self.source_filter.clear();
            }
            KeyCode::Char(' ') | KeyCode::Enter if n > 0 => {
                let id = self.sources[visible[self.source_selected]].id.clone();
                if settings.sources.is_empty() {
                    // "All sources" → everything except the toggled one.
                    settings.sources = self.sources.iter().map(|s| s.id.clone()).filter(|s| *s != id).collect();
                } else if let Some(pos) = settings.sources.iter().position(|s| s.eq_ignore_ascii_case(&id)) {
                    settings.sources.remove(pos);
                } else {
                    settings.sources.push(id);
                }
                settings.sources.sort();
                return Action::Changed(self.selected);
            }
            KeyCode::Char('p') => {
                let (_, preset) = PRESETS[self.preset % PRESETS.len()];
                settings.sources = preset.iter().map(|s| s.to_string()).collect();
                self.preset = (self.preset + 1) % PRESETS.len();
                return Action::Changed(self.selected);
            }
            KeyCode::Char('a') => {
                settings.sources.clear();
                return Action::Changed(self.selected);
            }
            KeyCode::Char('r') => {
                settings.show_reprinted = !settings.show_reprinted;
                return Action::Changed(self.selected);
            }
            _ => {}
        }
        Action::None
    }

    /// Enabled-source summary for a campaign, grouped by kind of book.
    pub fn summary(&self, campaign: &Campaign) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for s in &self.sources {
            if enabled(campaign, &s.id) {
                *out.entry(s.group.clone()).or_default() += 1;
            }
        }
        out
    }
}

pub fn enabled(campaign: &Campaign, source: &str) -> bool {
    campaign.settings.sources.is_empty() || campaign.settings.sources.iter().any(|s| s.eq_ignore_ascii_case(source))
}
