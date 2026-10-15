//! Characters section: roster and character sheet state, and key handling.

use dnt_data::{Library, SourceFilter};
use dnt_rules::builder::{build, Build};
use dnt_rules::choices::{self, Pending};
use dnt_rules::character::Character;
use dnt_rules::overlay::OverlaySet;
use dnt_rules::play::{self, Edition};
use dnt_rules::standard::{ABILITIES, SKILLS};
use dnt_rules::Evaluation;
use dnt_store::CharacterFile;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::levelup::{self, LevelUp};
use crate::picker::{Picked, Picker};
use crate::wizard::{self, Wizard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Features,
}

impl Tab {
    pub const ALL: [Tab; 2] = [Tab::Overview, Tab::Features];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Features => "Features",
        }
    }
}

/// Something the overview cursor can rest on.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Stat(String),
    Resource(String),
    Slot(usize),
    Pact,
    HitDie(i64),
    Toggle(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    Damage,
    Heal,
    TempHp,
}

impl Prompt {
    pub fn label(self) -> &'static str {
        match self {
            Prompt::Damage => "Damage",
            Prompt::Heal => "Heal",
            Prompt::TempHp => "Temporary HP",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Popup {
    Breakdown(String),
    Input(Prompt, String),
    Confirm(Rest),
    LevelUp(Box<LevelUp>),
    /// Confirm removing the most recent level.
    Undo,
    /// Filling in missing class feature choices.
    Fill(Box<Fill>),
}

/// Walks through pending choices one picker at a time.
#[derive(Debug, Clone)]
pub struct Fill {
    pub queue: Vec<Pending>,
    pub picker: Picker,
    pub done: usize,
}

impl Fill {
    fn new(queue: Vec<Pending>) -> Option<Fill> {
        let first = queue.first()?;
        let picker = fill_picker(first);
        Some(Fill { queue, picker, done: 0 })
    }
}

fn fill_picker(p: &Pending) -> Picker {
    let mut picker = levelup::choice_picker(&p.choice, p.missing);
    picker.title = format!("{} ({} {})", picker.title, p.class_name, p.class_level);
    picker
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    Short,
    Long,
}

pub struct Sheet {
    pub file: CharacterFile,
    /// Class feature choices this character is missing.
    pub pending: Vec<Pending>,
    filter: SourceFilter,
    pub build: Build,
    pub ev: Evaluation,
    pub edition: Edition,
    pub tab: Tab,
    pub cursor: usize,
    pub feature_cursor: usize,
    pub feature_scroll: usize,
    pub popup: Option<Popup>,
}

impl Sheet {
    pub fn new(file: CharacterFile, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Sheet {
        let build = build(&file.character, lib, overlays);
        let ev = build.evaluate();
        let edition = Edition::of(&build, lib);
        Sheet {
            pending: choices::pending(lib, &file.character, filter),
            filter: filter.clone(),
            file,
            build,
            ev,
            edition,
            tab: Tab::Overview,
            cursor: 0,
            feature_cursor: 0,
            feature_scroll: 0,
            popup: None,
        }
    }

    fn rebuild(&mut self, lib: &Library, overlays: &OverlaySet) {
        self.build = build(&self.file.character, lib, overlays);
        self.ev = self.build.evaluate();
        self.pending = choices::pending(lib, &self.file.character, &self.filter);
    }

    pub fn character(&self) -> &Character {
        &self.file.character
    }

    /// Focusable overview items, in cursor order.
    pub fn items(&self) -> Vec<Item> {
        let mut out: Vec<Item> = ABILITIES.iter().map(|(a, _)| Item::Stat(a.to_string())).collect();
        for k in ["ac", "hp.max", "initiative", "speed", "prof", "passive.perception"] {
            out.push(Item::Stat(k.into()));
        }
        out.extend(SKILLS.iter().map(|(k, _, _)| Item::Stat(format!("skill.{k}"))));
        out.extend(self.build.resources.iter().map(|(r, _)| Item::Resource(r.key.clone())));
        for level in 1..=9 {
            if play::slot_max(&self.ev, level) > 0 {
                out.push(Item::Slot(level));
            }
        }
        if self.ev.int("pact.slots.max").unwrap_or(0) > 0 {
            out.push(Item::Pact);
        }
        for key in self.ev.stats.keys().filter(|k| k.starts_with("hit_dice.")) {
            if let Ok(faces) = key.trim_start_matches("hit_dice.d").trim_end_matches(".max").parse() {
                out.push(Item::HitDie(faces));
            }
        }
        out.extend(self.build.toggles.iter().map(|(t, _)| Item::Toggle(t.key.clone())));
        out
    }

    pub fn current_item(&self) -> Option<Item> {
        self.items().into_iter().nth(self.cursor)
    }
}

pub enum Action {
    None,
    Quit,
    Status(String),
    /// The open character changed and must be saved.
    Save(String),
    /// Save a newly created character.
    Create(Box<Character>),
    /// Open the character file in $EDITOR.
    Edit(std::path::PathBuf),
}

pub struct Characters {
    pub files: Vec<CharacterFile>,
    pub selected: usize,
    pub sheet: Option<Sheet>,
    pub name_input: Option<String>,
    pub wizard: Option<Wizard>,
    /// The active campaign's source filter.
    pub filter: SourceFilter,
}

impl Characters {
    pub fn new(files: Vec<CharacterFile>) -> Characters {
        Characters {
            files,
            selected: 0,
            sheet: None,
            name_input: None,
            wizard: None,
            filter: SourceFilter::default(),
        }
    }

    pub fn wants_text(&self) -> bool {
        self.name_input.is_some()
            || self.wizard.is_some()
            || matches!(
                self.sheet.as_ref().and_then(|s| s.popup.as_ref()),
                Some(Popup::Input(..) | Popup::LevelUp(_) | Popup::Fill(_))
            )
    }

    pub fn open(&mut self, index: usize, lib: &Library, overlays: &OverlaySet) {
        if let Some(file) = self.files.get(index).cloned() {
            self.selected = index;
            self.sheet = Some(Sheet::new(file, lib, overlays, &self.filter));
        }
    }

    /// Reload the open sheet after its file changed on disk (e.g. edited).
    pub fn reload(&mut self, file: CharacterFile, lib: &Library, overlays: &OverlaySet) {
        if let Some(i) = self.files.iter().position(|f| f.path == file.path) {
            self.files[i] = file.clone();
        }
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.file = file;
            sheet.rebuild(lib, overlays);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Action {
        if let Some(name) = self.name_input.as_mut() {
            match key.code {
                KeyCode::Esc => self.name_input = None,
                KeyCode::Enter => {
                    let name = name.trim().to_string();
                    self.name_input = None;
                    if !name.is_empty() {
                        self.wizard = Some(Wizard::new(name, lib, filter));
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
        if let Some(w) = self.wizard.as_mut() {
            match w.key(key, lib, filter) {
                wizard::Outcome::Continue => {}
                wizard::Outcome::Cancel => self.wizard = None,
                wizard::Outcome::Finish(ch) => {
                    self.wizard = None;
                    return Action::Create(ch);
                }
            }
            return Action::None;
        }
        match self.sheet.as_mut() {
            None => self.roster_key(key, lib, overlays),
            Some(sheet) => {
                let action = sheet_key(sheet, key, lib, overlays, filter);
                if matches!(action, Action::Quit) {
                    // `q`/Esc on a sheet goes back to the roster.
                    if let Some(s) = self.sheet.take() {
                        if let Some(f) = self.files.iter_mut().find(|f| f.path == s.file.path) {
                            *f = s.file;
                        }
                    }
                    return Action::None;
                }
                action
            }
        }
    }

    fn roster_key(&mut self, key: KeyEvent, lib: &Library, overlays: &OverlaySet) -> Action {
        let n = self.files.len();
        match key.code {
            KeyCode::Char('q') => return Action::Quit,
            KeyCode::Char('j') | KeyCode::Down if n > 0 => self.selected = (self.selected + 1).min(n - 1),
            KeyCode::Char('k') | KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('l') if n > 0 => self.open(self.selected, lib, overlays),
            KeyCode::Char('n') => self.name_input = Some(String::new()),
            KeyCode::Char('e') if n > 0 => return Action::Edit(self.files[self.selected].path.clone()),
            _ => {}
        }
        Action::None
    }
}

fn sheet_key(sheet: &mut Sheet, key: KeyEvent, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Action {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // Popups take all input.
    if let Some(popup) = sheet.popup.clone() {
        match popup {
            Popup::LevelUp(mut flow) => {
                match flow.key(key, lib, &sheet.file.character, &sheet.ev, filter) {
                    levelup::Outcome::Continue => sheet.popup = Some(Popup::LevelUp(flow)),
                    levelup::Outcome::Cancel => sheet.popup = None,
                    levelup::Outcome::Apply => {
                        sheet.popup = None;
                        let status = flow.apply(&mut sheet.file.character);
                        return changed(sheet, lib, overlays, status);
                    }
                }
                return Action::None;
            }
            Popup::Fill(mut fill) => {
                sheet.popup = None;
                let values = match fill.picker.key(key) {
                    Some(Picked::One(v)) => vec![v],
                    Some(Picked::Many(vs)) => vs,
                    None => {
                        if key.code != KeyCode::Esc {
                            sheet.popup = Some(Popup::Fill(fill));
                        }
                        return Action::None;
                    }
                };
                let p = fill.queue.remove(0);
                if let Some(level) = sheet.file.character.levels.get_mut(p.level_index) {
                    level.choices.picks.entry(p.choice.key.clone()).or_default().extend(values);
                }
                fill.done += 1;
                // Recompute: earlier picks change what remains available.
                sheet.rebuild(lib, overlays);
                match Fill::new(sheet.pending.clone()) {
                    Some(mut next) => {
                        next.done = fill.done;
                        sheet.popup = Some(Popup::Fill(Box::new(next)));
                        return Action::Save(format!("{} chosen", p.choice.key));
                    }
                    None => return changed(sheet, lib, overlays, "All choices made".into()),
                }
            }
            Popup::Undo => {
                sheet.popup = None;
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    if let Some(l) = sheet.file.character.remove_last_level() {
                        let name = l.class.split_once(':').map_or("", |(_, r)| r.split('|').next().unwrap_or(r)).to_string();
                        return changed(sheet, lib, overlays, format!("Removed a {name} level"));
                    }
                }
                return Action::None;
            }
            Popup::Breakdown(_) => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | '?' | ' ')) {
                    sheet.popup = None;
                }
                return Action::None;
            }
            Popup::Confirm(rest) => {
                sheet.popup = None;
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                    let report = match rest {
                        Rest::Short => play::short_rest(&mut sheet.file.character, &sheet.build, &sheet.ev),
                        Rest::Long => play::long_rest(&mut sheet.file.character, &sheet.build, &sheet.ev, sheet.edition),
                    };
                    let what = if rest == Rest::Short { "Short rest" } else { "Long rest" };
                    let detail = if report.restored.is_empty() {
                        "nothing to restore".to_string()
                    } else {
                        report.restored.join(", ")
                    };
                    return changed(sheet, lib, overlays, format!("{what}: {detail}"));
                }
                return Action::None;
            }
            Popup::Input(prompt, mut text) => {
                match key.code {
                    KeyCode::Esc => sheet.popup = None,
                    KeyCode::Backspace => {
                        text.pop();
                        sheet.popup = Some(Popup::Input(prompt, text));
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        text.push(c);
                        sheet.popup = Some(Popup::Input(prompt, text));
                    }
                    KeyCode::Enter => {
                        sheet.popup = None;
                        let Ok(n) = text.parse::<i64>() else { return Action::None };
                        let c = &mut sheet.file.character;
                        match prompt {
                            Prompt::Damage => play::damage(c, &sheet.ev, n),
                            Prompt::Heal => play::heal(c, &sheet.ev, n),
                            Prompt::TempHp => play::set_temp_hp(c, n),
                        }
                        return changed(sheet, lib, overlays, format!("{} {n}", prompt.label()));
                    }
                    _ => {}
                }
                return Action::None;
            }
        }
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => return Action::Quit,
        KeyCode::Tab | KeyCode::BackTab => {
            let i = Tab::ALL.iter().position(|t| *t == sheet.tab).unwrap();
            let d = if key.code == KeyCode::Tab { 1 } else { Tab::ALL.len() - 1 };
            sheet.tab = Tab::ALL[(i + d) % Tab::ALL.len()];
            return Action::None;
        }
        KeyCode::Char('d') if !ctrl => {
            sheet.popup = Some(Popup::Input(Prompt::Damage, String::new()));
            return Action::None;
        }
        KeyCode::Char('h') => {
            sheet.popup = Some(Popup::Input(Prompt::Heal, String::new()));
            return Action::None;
        }
        KeyCode::Char('t') => {
            sheet.popup = Some(Popup::Input(Prompt::TempHp, String::new()));
            return Action::None;
        }
        KeyCode::Char('s') => {
            sheet.popup = Some(Popup::Confirm(Rest::Short));
            return Action::None;
        }
        KeyCode::Char('L') => {
            sheet.popup = Some(Popup::Confirm(Rest::Long));
            return Action::None;
        }
        KeyCode::Char('e') => return Action::Edit(sheet.file.path.clone()),
        KeyCode::Char('u') => {
            let flow = LevelUp::start(lib, &sheet.file.character, &sheet.ev, filter);
            sheet.popup = Some(Popup::LevelUp(Box::new(flow)));
            return Action::None;
        }
        KeyCode::Char('c') => {
            match Fill::new(sheet.pending.clone()) {
                Some(fill) => sheet.popup = Some(Popup::Fill(Box::new(fill))),
                None => return Action::Status("No choices pending".into()),
            }
            return Action::None;
        }
        KeyCode::Char('U') if !sheet.file.character.levels.is_empty() => {
            sheet.popup = Some(Popup::Undo);
            return Action::None;
        }
        _ => {}
    }

    match sheet.tab {
        Tab::Overview => overview_key(sheet, key, lib, overlays),
        Tab::Features => {
            let n = sheet.build.features.len();
            match key.code {
                KeyCode::Char('j') | KeyCode::Down if n > 0 => {
                    sheet.feature_cursor = (sheet.feature_cursor + 1).min(n - 1);
                    sheet.feature_scroll = 0;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    sheet.feature_cursor = sheet.feature_cursor.saturating_sub(1);
                    sheet.feature_scroll = 0;
                }
                KeyCode::Char('J') => sheet.feature_scroll += 1,
                KeyCode::Char('K') => sheet.feature_scroll = sheet.feature_scroll.saturating_sub(1),
                KeyCode::Char('d') if ctrl => sheet.feature_scroll += 10,
                KeyCode::Char('u') if ctrl => sheet.feature_scroll = sheet.feature_scroll.saturating_sub(10),
                _ => {}
            }
            Action::None
        }
    }
}

fn overview_key(sheet: &mut Sheet, key: KeyEvent, lib: &Library, overlays: &OverlaySet) -> Action {
    let items = sheet.items();
    let n = items.len();
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if n > 0 => sheet.cursor = (sheet.cursor + 1).min(n - 1),
        KeyCode::Char('k') | KeyCode::Up => sheet.cursor = sheet.cursor.saturating_sub(1),
        KeyCode::Char('g') => sheet.cursor = 0,
        KeyCode::Char('G') => sheet.cursor = n.saturating_sub(1),
        KeyCode::Char('?') | KeyCode::Enter => {
            let key = match items.get(sheet.cursor) {
                Some(Item::Stat(k)) => Some(k.clone()),
                Some(Item::Resource(k)) => Some(format!("resource.{k}.max")),
                Some(Item::Slot(l)) => Some(format!("slots.{l}.max")),
                Some(Item::Pact) => Some("pact.slots.max".into()),
                Some(Item::HitDie(f)) => Some(format!("hit_dice.d{f}.max")),
                _ => None,
            };
            if let Some(k) = key {
                sheet.popup = Some(Popup::Breakdown(k));
            } else if let Some(Item::Toggle(_)) = items.get(sheet.cursor) {
                return toggle(sheet, lib, overlays);
            }
        }
        KeyCode::Char(' ' | '-') => return spend(sheet, lib, overlays, 1),
        KeyCode::Char('+' | '=') => return spend(sheet, lib, overlays, -1),
        _ => {}
    }
    Action::None
}

fn toggle(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet) -> Action {
    let Some(Item::Toggle(k)) = sheet.current_item() else { return Action::None };
    let toggles = &mut sheet.file.character.state.toggles;
    let on = !toggles.get(&k).copied().unwrap_or(false);
    if on {
        toggles.insert(k.clone(), true);
    } else {
        toggles.remove(&k);
    }
    changed(sheet, lib, overlays, format!("{k} {}", if on { "on" } else { "off" }))
}

/// Spend (n > 0) or restore (n < 0) the item under the cursor.
fn spend(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, n: i64) -> Action {
    let item = sheet.current_item();
    let c = &mut sheet.file.character;
    let ev = &sheet.ev;
    let (ok, what) = match item {
        Some(Item::Resource(k)) => (play::spend_resource(c, ev, &k, n), k),
        Some(Item::Slot(l)) => (play::spend_slot(c, ev, l, n), format!("level {l} slot")),
        Some(Item::Pact) => (play::spend_pact_slot(c, ev, n), "pact slot".into()),
        Some(Item::HitDie(f)) => (play::spend_hit_die(c, ev, f, n), format!("d{f} hit die")),
        Some(Item::Toggle(_)) if n > 0 => return toggle(sheet, lib, overlays),
        _ => return Action::None,
    };
    if !ok {
        return Action::Status(if n > 0 { format!("No {what} left") } else { format!("{what} is full") });
    }
    changed(sheet, lib, overlays, format!("{} {what}", if n > 0 { "Used" } else { "Restored" }))
}

fn changed(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, status: String) -> Action {
    sheet.rebuild(lib, overlays);
    Action::Save(status)
}
