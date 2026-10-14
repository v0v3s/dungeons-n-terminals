//! Top-level application state: sections and global keys.

use dnt_data::{Library, SourceFilter};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::compendium::{Compendium, Outcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Compendium,
    Characters,
    Notes,
}

impl Section {
    pub const ALL: [Section; 3] = [Section::Compendium, Section::Characters, Section::Notes];

    pub fn label(self) -> &'static str {
        match self {
            Section::Compendium => "Compendium",
            Section::Characters => "Characters",
            Section::Notes => "Notes",
        }
    }
}

pub struct App {
    pub section: Section,
    pub compendium: Compendium,
    pub status: String,
    pub quit: bool,
}

impl App {
    pub fn new(lib: &Library, filter: SourceFilter, status: String) -> App {
        App {
            section: Section::Compendium,
            compendium: Compendium::new(lib, filter),
            status,
            quit: false,
        }
    }

    fn wants_text(&self) -> bool {
        self.section == Section::Compendium && self.compendium.wants_text()
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
            Section::Characters | Section::Notes => {
                if key.code == KeyCode::Char('q') {
                    self.quit = true;
                }
            }
        }
    }
}
