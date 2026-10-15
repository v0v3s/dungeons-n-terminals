//! Characters section: roster and character sheet state, and key handling.

use dnt_data::{Library, SourceFilter};
use dnt_rules::builder::{build, Build};
use dnt_rules::equipment;
use dnt_rules::choices::{self, Pending};
use dnt_rules::character::{Character, Hand, Slot};
use dnt_rules::overlay::OverlaySet;
use dnt_rules::play::{self, Edition};
use dnt_rules::standard::{ABILITIES, SKILLS};
use dnt_rules::Evaluation;
use dnt_store::CharacterFile;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::levelup::{self, LevelUp};
use crate::spells_tab;
use crate::picker::{Picked, Picker};
use crate::wizard::{self, Wizard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Features,
    Spells,
    Inventory,
    Notes,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Overview, Tab::Features, Tab::Spells, Tab::Inventory, Tab::Notes];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Features => "Features",
            Tab::Spells => "Spells",
            Tab::Inventory => "Inventory",
            Tab::Notes => "Notes",
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
    /// An attack by slug (`attack.<slug>.*` stats).
    Attack(String),
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
    /// Adding an item from the library.
    AddItem(Box<Picker>),
    /// Free text: a custom item's name or a currency change.
    Text(TextPrompt, String),
    /// Adding spells to a class or a feature grant.
    SpellPick(Box<Picker>, spells_tab::Target),
    /// Choosing how to cast a spell.
    Cast(Box<Picker>, dnt_data::EntityId),
    /// Choosing where to wear an inventory line.
    Wear(Box<Picker>, usize),
    /// Choosing a container for an inventory line.
    MoveInto(Box<Picker>, usize),
    /// Swapping a weapon mastery: the mastered weapon being replaced.
    SwapMastery(Box<Picker>, dnt_data::EntityId),
    /// Writing a feature's rules.
    Overlay(Box<crate::overlay_editor::OverlayEditor>),
    /// Conditions and exhaustion (cursor row; the last is exhaustion).
    Conditions(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextPrompt {
    CustomItem,
    Currency,
    /// Container settings of the line under the cursor.
    Container,
}

impl TextPrompt {
    pub fn label(self) -> &'static str {
        match self {
            TextPrompt::CustomItem => "Custom item name",
            TextPrompt::Currency => "Coins (+15gp, -3sp, 20 sets gold)",
            TextPrompt::Container => "Container: 30 lb · weightless · no limit · off · empty resets",
        }
    }
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
    pub(crate) filter: SourceFilter,
    pub build: Build,
    pub ev: Evaluation,
    pub edition: Edition,
    pub tab: Tab,
    pub cursor: usize,
    /// The inventory line under the cursor (an index into the inventory).
    pub inventory_cursor: usize,
    /// Folded containers (by `id`).
    pub folded: std::collections::BTreeSet<String>,
    /// Dice, the next roll's mode and the roll log.
    pub roller: crate::rolls::Roller,
    pub spell_cursor: usize,
    pub feature_cursor: usize,
    pub feature_scroll: usize,
    pub popup: Option<Popup>,
}

impl Sheet {
    /// Path of this character's note (`notes/Party/<name>.md`).
    pub fn note_path(&self) -> Option<std::path::PathBuf> {
        let campaign = self.file.path.parent()?.parent()?;
        Some(campaign.join("notes/Party").join(format!("{}.md", dnt_store::notes::file_safe(&self.file.character.name))))
    }

    pub fn new(mut file: CharacterFile, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Sheet {
        // Older files: equipped weapons go in hand, worn items get a slot
        // (saved with the next change).
        equipment::normalize(lib, &mut file.character);
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
            inventory_cursor: 0,
            folded: Default::default(),
            roller: Default::default(),
            spell_cursor: 0,
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
        out.extend(self.build.attacks.iter().map(|a| Item::Attack(a.slug.clone())));
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
    /// Open this character's note in the Notes section.
    OpenNote(String),
    /// Create a campaign item and add it to the inventory.
    CreateItem,
    /// Open the rules editor for a feature.
    EditOverlay(dnt_data::EntityId),
    /// Save a feature's rules (overlay id, TOML; empty removes them).
    SaveOverlay(String, String),
    /// Export the open sheet: (file slug, Markdown, HTML).
    Export(String, String, String),
    /// A roll was made: its line, and whether to add it to the session note.
    Rolled(String, bool),
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
    /// Open the rules editor on the open sheet (`user`: the user's overlay text).
    pub fn open_overlay(&mut self, lib: &Library, overlays: &OverlaySet, entity: dnt_data::EntityId, user: Option<String>) {
        let Some(sheet) = self.sheet.as_mut() else { return };
        let mut ed = crate::overlay_editor::OverlayEditor::open(lib, entity, user);
        ed.check(lib, &sheet.file.character, overlays, &sheet.ev);
        sheet.popup = Some(Popup::Overlay(Box::new(ed)));
    }

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
                Some(
                    Popup::Input(..)
                        | Popup::LevelUp(_)
                        | Popup::Fill(_)
                        | Popup::AddItem(_)
                        | Popup::Text(..)
                        | Popup::SpellPick(..)
                        | Popup::Cast(..)
                        | Popup::Wear(..)
                        | Popup::MoveInto(..)
                        | Popup::SwapMastery(..)
                        | Popup::Overlay(..)
                )
            )
    }

    pub fn open(&mut self, index: usize, lib: &Library, overlays: &OverlaySet) {
        if let Some(file) = self.files.get(index).cloned() {
            self.selected = index;
            self.sheet = Some(Sheet::new(file, lib, overlays, &self.filter));
        }
    }

    /// Rebuild an open sheet against a rebuilt library.
    pub fn refresh(&mut self, lib: &Library, overlays: &OverlaySet) {
        if let Some(sheet) = self.sheet.as_mut() {
            sheet.filter = self.filter.clone();
            sheet.rebuild(lib, overlays);
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
            KeyCode::Char('g') | KeyCode::Home => self.selected = 0,
            KeyCode::Char('G') | KeyCode::End => self.selected = n.saturating_sub(1),
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
                        let action = changed(sheet, lib, overlays, status.clone());
                        // New spells to learn or prepare at the new level.
                        let casters = dnt_rules::spells::casters(lib, &sheet.build, &sheet.ev);
                        let gaps = spells_tab::spell_gaps(&sheet.file.character, &casters);
                        if !gaps.is_empty() {
                            if let Action::Save(_) = action {
                                return Action::Save(format!("{status} · to choose on the Spells tab: {}", spells_tab::describe_gaps(&gaps)));
                            }
                        }
                        return action;
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
            Popup::AddItem(mut picker) => {
                sheet.popup = None;
                match picker.key(key) {
                    Some(Picked::One(v)) if v == "custom" => {
                        sheet.popup = Some(Popup::Text(TextPrompt::CustomItem, String::new()));
                    }
                    Some(Picked::One(v)) if v == "create" => return Action::CreateItem,
                    Some(Picked::One(v)) => {
                        // New items go where the cursor is (into its container).
                        let c = &mut sheet.file.character;
                        let inside = c.inventory.get(sheet.inventory_cursor).and_then(|l| l.inside.clone());
                        let before = c.inventory.len();
                        equipment::add_item(c, &v, 1, None);
                        if c.inventory.len() > before {
                            c.inventory[before].inside = inside;
                            sheet.inventory_cursor = before;
                        } else if let Some(at) = c.inventory.iter().position(|l| l.item.as_deref() == Some(v.as_str()) && l.name.is_none()) {
                            sheet.inventory_cursor = at;
                        }
                        let name = dnt_rules::overlay::resolve_id(lib, &v).map(|e| lib.entity(e).name.clone()).unwrap_or(v);
                        return changed(sheet, lib, overlays, format!("Added {name}"));
                    }
                    _ if key.code == KeyCode::Esc => {}
                    _ => sheet.popup = Some(Popup::AddItem(picker)),
                }
                return Action::None;
            }
            Popup::Text(prompt, mut text) => {
                sheet.popup = None;
                match key.code {
                    KeyCode::Esc => {}
                    KeyCode::Enter => {
                        let c = &mut sheet.file.character;
                        match prompt {
                            TextPrompt::CustomItem if !text.trim().is_empty() => {
                                let inside = c.inventory.get(sheet.inventory_cursor).and_then(|l| l.inside.clone());
                                c.inventory.push(dnt_rules::character::InventoryItem {
                                    name: Some(text.trim().to_string()),
                                    quantity: 1,
                                    inside,
                                    ..Default::default()
                                });
                                sheet.inventory_cursor = c.inventory.len() - 1;
                                return changed(sheet, lib, overlays, format!("Added {}", text.trim()));
                            }
                            TextPrompt::Container => {
                                let i = sheet.inventory_cursor;
                                let Some(line) = c.inventory.get_mut(i) else { return Action::None };
                                return match equipment::set_container(line, &text) {
                                    Ok(()) => {
                                        let msg = match equipment::container(lib, &c.inventory[i]) {
                                            Some(k) => format!(
                                                "Container: {}{}",
                                                k.capacity.map(|w| format!("{w} lb")).unwrap_or_else(|| "no limit".into()),
                                                if k.weightless { ", weightless" } else { "" }
                                            ),
                                            None => "Not a container".into(),
                                        };
                                        changed(sheet, lib, overlays, msg)
                                    }
                                    Err(e) => Action::Status(e),
                                };
                            }
                            TextPrompt::Currency => {
                                if c.currency.apply(&text) {
                                    return changed(sheet, lib, overlays, format!("Coins {}", text.trim()));
                                }
                                return Action::Status(format!("Could not read {text:?}: try +15gp, -3sp or 20"));
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Backspace => {
                        text.pop();
                        sheet.popup = Some(Popup::Text(prompt, text));
                    }
                    KeyCode::Char(ch) if !ctrl => {
                        text.push(ch);
                        sheet.popup = Some(Popup::Text(prompt, text));
                    }
                    _ => sheet.popup = Some(Popup::Text(prompt, text)),
                }
                return Action::None;
            }
            Popup::SpellPick(mut picker, target) => {
                sheet.popup = None;
                let ids = match picker.key(key) {
                    Some(Picked::One(v)) => vec![v],
                    Some(Picked::Many(vs)) => vs,
                    None => {
                        if key.code != KeyCode::Esc {
                            sheet.popup = Some(Popup::SpellPick(picker, target));
                        }
                        return Action::None;
                    }
                };
                let names: Vec<String> = ids
                    .iter()
                    .filter_map(|i| dnt_rules::overlay::resolve_id(lib, i))
                    .map(|e| lib.entity(e).name.clone())
                    .collect();
                let casters = dnt_rules::spells::casters(lib, &sheet.build, &sheet.ev);
                let limits = match &target {
                    spells_tab::Target::Class { class_id, .. } => casters.iter().find(|c| &c.id == class_id).map(|c| &c.limits),
                    _ => None,
                };
                return match spells_tab::add(lib, &mut sheet.file.character, &target, ids, limits) {
                    Ok(()) => changed(sheet, lib, overlays, format!("Added {}", names.join(", "))),
                    Err(e) => Action::Status(e),
                };
            }
            Popup::Cast(mut picker, spell) => {
                sheet.popup = None;
                match picker.key(key) {
                    Some(Picked::One(how)) => {
                        return match spells_tab::cast(lib, &mut sheet.file.character, &sheet.ev, spell, &how) {
                            Ok(msg) => {
                                let action = changed(sheet, lib, overlays, msg.clone());
                                cast_roll(sheet, lib, spell, &how, &msg).unwrap_or(action)
                            }
                            Err(msg) => Action::Status(msg),
                        };
                    }
                    _ if key.code == KeyCode::Esc => {}
                    _ => sheet.popup = Some(Popup::Cast(picker, spell)),
                }
                return Action::None;
            }
            Popup::Overlay(mut ed) => {
                return match ed.key(key) {
                    crate::overlay_editor::Outcome::Continue => {
                        ed.check(lib, &sheet.file.character, overlays, &sheet.ev);
                        sheet.popup = Some(Popup::Overlay(ed));
                        Action::None
                    }
                    crate::overlay_editor::Outcome::Close => {
                        sheet.popup = None;
                        Action::Status("Rules unchanged".into())
                    }
                    crate::overlay_editor::Outcome::Save(text) => {
                        sheet.popup = None;
                        Action::SaveOverlay(ed.id.clone(), text)
                    }
                };
            }
            Popup::Conditions(cursor) => {
                let names = dnt_rules::conditions::CONDITIONS;
                let n = names.len() + 1;
                let st = &mut sheet.file.character.state;
                let msg = match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('C') => {
                        sheet.popup = None;
                        return Action::None;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        sheet.popup = Some(Popup::Conditions((cursor + 1).min(n - 1)));
                        return Action::None;
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        sheet.popup = Some(Popup::Conditions(cursor.saturating_sub(1)));
                        return Action::None;
                    }
                    KeyCode::Char('+' | '=') => {
                        st.exhaustion = (st.exhaustion + 1).min(6);
                        format!("Exhaustion {}", st.exhaustion)
                    }
                    KeyCode::Char('-') => {
                        st.exhaustion = (st.exhaustion - 1).max(0);
                        format!("Exhaustion {}", st.exhaustion)
                    }
                    KeyCode::Char(' ') | KeyCode::Enter if cursor < names.len() => {
                        let c = names[cursor];
                        if let Some(i) = st.conditions.iter().position(|x| x.eq_ignore_ascii_case(c)) {
                            st.conditions.remove(i);
                            format!("No longer {c}")
                        } else {
                            st.conditions.push(c.to_string());
                            c.to_string()
                        }
                    }
                    _ => {
                        sheet.popup = Some(Popup::Conditions(cursor));
                        return Action::None;
                    }
                };
                let action = changed(sheet, lib, overlays, msg);
                sheet.popup = Some(Popup::Conditions(cursor));
                return action;
            }
            Popup::SwapMastery(mut picker, old) => {
                sheet.popup = None;
                match picker.key(key) {
                    Some(Picked::One(new)) => {
                        let from = lib.entity(old).name.clone();
                        let to = dnt_rules::overlay::resolve_id(lib, &new).map(|e| lib.entity(e).name.clone()).unwrap_or_else(|| new.clone());
                        if choices::swap_mastery(lib, &mut sheet.file.character, old, &new) {
                            return changed(sheet, lib, overlays, format!("Weapon Mastery: {from} → {to}"));
                        }
                        return Action::Status(format!("{from} is not a mastery chosen at a level"));
                    }
                    _ if key.code == KeyCode::Esc => {}
                    _ => sheet.popup = Some(Popup::SwapMastery(picker, old)),
                }
                return Action::None;
            }
            Popup::MoveInto(mut picker, i) => {
                sheet.popup = None;
                match picker.key(key) {
                    Some(Picked::One(v)) => {
                        let into = v.parse::<usize>().ok();
                        return match equipment::put_in(lib, &mut sheet.file.character, i, into) {
                            Ok(msg) => {
                                // Show where it went.
                                if let Some(id) = into.and_then(|t| sheet.file.character.inventory[t].id.clone()) {
                                    sheet.folded.remove(&id);
                                }
                                changed(sheet, lib, overlays, msg)
                            }
                            Err(e) => Action::Status(e),
                        };
                    }
                    _ if key.code == KeyCode::Esc => {}
                    _ => sheet.popup = Some(Popup::MoveInto(picker, i)),
                }
                return Action::None;
            }
            Popup::Wear(mut picker, i) => {
                sheet.popup = None;
                match picker.key(key) {
                    Some(Picked::One(v)) => {
                        let Some(&slot) = v.parse::<usize>().ok().and_then(|n| Slot::ALL.get(n)) else { return Action::None };
                        let off = equipment::wear(lib, &mut sheet.file.character, i, slot);
                        let name = line_name(lib, &sheet.file.character.inventory[i]);
                        let mut msg = format!("Wearing {name} ({})", slot.label());
                        if !off.is_empty() {
                            msg.push_str(&format!(", took off {}", off.join(", ")));
                        }
                        return changed(sheet, lib, overlays, msg);
                    }
                    _ if key.code == KeyCode::Esc => {}
                    _ => sheet.popup = Some(Popup::Wear(picker, i)),
                }
                return Action::None;
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
                    let mut msg = format!("{what}: {detail}");
                    if rest == Rest::Long && sheet.build.features.iter().any(|f| f.source == dnt_rules::builder::FeatureSource::Mastery) {
                        msg.push_str(" · you may swap a Weapon Mastery (r on the Features tab)");
                    }
                    return changed(sheet, lib, overlays, msg);
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
        KeyCode::Char('X') => {
            let d = crate::export::character(lib, sheet);
            return Action::Export(sheet.file.slug.clone(), crate::export::to_markdown(&d), crate::export::to_html(&d));
        }
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
        Tab::Inventory => inventory_key(sheet, key, lib, overlays, filter),
        Tab::Spells => spells_key(sheet, key, lib, overlays, filter),
        Tab::Notes => match key.code {
            KeyCode::Enter | KeyCode::Char('i') => Action::OpenNote(sheet.file.character.name.clone()),
            _ => Action::None,
        },
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
                KeyCode::Char('O') => {
                    if let Some(f) = sheet.build.features.get(sheet.feature_cursor) {
                        return Action::EditOverlay(f.entity);
                    }
                }
                KeyCode::Char('r') => {
                    let Some(f) = sheet.build.features.get(sheet.feature_cursor) else { return Action::None };
                    if f.source != dnt_rules::builder::FeatureSource::Mastery {
                        return Action::Status("r swaps a Weapon Mastery: pick one in the Weapon Mastery group".into());
                    }
                    let items: Vec<crate::picker::PickItem> = choices::mastery_swap_options(lib, &sheet.file.character, filter)
                        .into_iter()
                        .map(|o| crate::picker::PickItem::new(o.label, o.hint, o.value))
                        .collect();
                    let name = lib.entity(f.entity).name.clone();
                    let picker = Picker::new(format!("Swap the mastery of {name} for…"), items);
                    sheet.popup = Some(Popup::SwapMastery(Box::new(picker), f.entity));
                    return Action::None;
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

/// After casting: roll the spell's attack and damage at the level used.
fn cast_roll(sheet: &mut Sheet, lib: &Library, spell: dnt_data::EntityId, how: &str, msg: &str) -> Option<Action> {
    if !crate::rolls::spell_rolls(lib, spell) {
        return None;
    }
    let (atk, dc) = crate::rolls::spell_numbers(sheet, None);
    let level = crate::rolls::cast_level(sheet, lib, spell, how);
    let r = crate::rolls::spell(sheet, lib, spell, level, atk, dc);
    Some(Action::Rolled(format!("{msg} · {}", r.line()), sheet.roller.to_notes))
}

/// Roll what the overview cursor is on (`save`: a saving throw for an ability).
fn roll_item(sheet: &mut Sheet, save: bool) -> Action {
    let rolled = match sheet.current_item() {
        Some(Item::Stat(k)) if ABILITIES.iter().any(|(a, _)| *a == k) => {
            if save {
                crate::rolls::save(sheet, &k)
            } else {
                crate::rolls::check(sheet, &k, None)
            }
        }
        Some(Item::Stat(k)) if k.starts_with("skill.") => {
            let key = k.trim_start_matches("skill.");
            let Some((_, name, ab)) = SKILLS.iter().find(|(s, _, _)| *s == key) else { return Action::None };
            crate::rolls::check(sheet, ab, Some((key, name)))
        }
        Some(Item::Stat(k)) if k == "initiative" => crate::rolls::initiative(sheet),
        Some(Item::Attack(slug)) => {
            let Some(a) = sheet.build.attacks.iter().find(|a| a.slug == slug).cloned() else { return Action::None };
            crate::rolls::attack(sheet, &a)
        }
        _ => return Action::Status("Roll an ability, skill, initiative or attack (r), or a save on an ability (R)".into()),
    };
    Action::Rolled(rolled.line(), sheet.roller.to_notes)
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
                Some(Item::Attack(s)) => Some(format!("attack.{s}")),
                _ => None,
            };
            if let Some(k) = key {
                sheet.popup = Some(Popup::Breakdown(k));
            } else if let Some(Item::Toggle(_)) = items.get(sheet.cursor) {
                return toggle(sheet, lib, overlays);
            }
        }
        KeyCode::Char('r') => return roll_item(sheet, false),
        KeyCode::Char('R') => return roll_item(sheet, true),
        KeyCode::Char('v') => {
            let m = sheet.roller.cycle();
            return Action::Status(match m {
                dnt_rules::roll::Advantage::Normal => "Next roll: normal".into(),
                m => format!("Next roll: {}", m.label()),
            });
        }
        KeyCode::Char('N') => {
            sheet.roller.to_notes = !sheet.roller.to_notes;
            return Action::Status(if sheet.roller.to_notes { "Rolls go into the latest session note" } else { "Rolls stay on the sheet" }.into());
        }
        KeyCode::Char('C') => {
            sheet.popup = Some(Popup::Conditions(0));
            return Action::None;
        }
        KeyCode::Char('b') if matches!(items.get(sheet.cursor), Some(Item::Attack(_))) => {
            let Some(Item::Attack(slug)) = items.get(sheet.cursor) else { unreachable!() };
            let Some(i) = attack_line(sheet, lib, slug) else { return Action::None };
            return hold(sheet, lib, overlays, i, Hand::Both);
        }
        KeyCode::Char(' ' | 'w' | 'o') if matches!(items.get(sheet.cursor), Some(Item::Attack(_))) => {
            let Some(Item::Attack(slug)) = items.get(sheet.cursor) else { unreachable!() };
            let Some(i) = attack_line(sheet, lib, slug) else {
                return Action::Status("Unarmed strikes need no hands".into());
            };
            let hand = if key.code == KeyCode::Char('o') { Hand::Off } else { Hand::Main };
            return hold(sheet, lib, overlays, i, hand);
        }
        KeyCode::Char('S') => {
            // Take up or put away the shield (the one in hand first).
            let inv = &sheet.file.character.inventory;
            let is_shield = |l: &dnt_rules::character::InventoryItem| {
                item_entity(lib, l).is_some_and(|e| equipment::armor_kind(&lib.entity(e).data) == Some("shield"))
            };
            let Some(i) = inv.iter().position(|l| is_shield(l) && l.hand.is_some()).or_else(|| inv.iter().position(is_shield)) else {
                return Action::Status("No shield in the inventory".into());
            };
            let hand = sheet.file.character.inventory[i].hand.unwrap_or(Hand::Off);
            return hold(sheet, lib, overlays, i, hand);
        }
        KeyCode::Char(' ' | '-') => return spend(sheet, lib, overlays, 1),
        KeyCode::Char('+' | '=') => return spend(sheet, lib, overlays, -1),
        _ => {}
    }
    Action::None
}

fn spells_key(sheet: &mut Sheet, key: KeyEvent, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Action {
    use spells_tab::Row;
    let data = spells_tab::rows(lib, &sheet.file.character, &sheet.build, &sheet.ev);
    let n = data.rows.len();
    let i = sheet.spell_cursor.min(n.saturating_sub(1));
    // A favorite acts like the spell's own row.
    let row = data.rows.get(i).map(|r| r.inner().clone());
    // The casting class the cursor is in (for adding).
    let caster_at = |i: usize| {
        data.rows[..=i.min(n.saturating_sub(1))].iter().rev().find_map(|r| match r {
            Row::ClassHeader(c) | Row::ClassSpell { caster: c, .. } => Some(*c),
            _ => None,
        })
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if n > 0 => sheet.spell_cursor = (i + 1).min(n - 1),
        KeyCode::Char('k') | KeyCode::Up => sheet.spell_cursor = i.saturating_sub(1),
        KeyCode::Char('g') | KeyCode::Home => sheet.spell_cursor = 0,
        KeyCode::Char('G') | KeyCode::End => sheet.spell_cursor = n.saturating_sub(1),
        KeyCode::Char('a') => {
            let Some(c) = caster_at(i).or((!data.casters.is_empty()).then_some(0)) else {
                return Action::Status("No spellcasting class".into());
            };
            let (picker, target) = spells_tab::class_picker(lib, &sheet.file.character, &data.casters[c], filter);
            sheet.popup = Some(Popup::SpellPick(Box::new(picker), target));
        }
        KeyCode::Enter => match row {
            Some(Row::GrantChoice { key, filter: f, missing, .. }) => {
                let (picker, target) = spells_tab::grant_picker(lib, &sheet.file.character, &key, &f, missing, filter);
                sheet.popup = Some(Popup::SpellPick(Box::new(picker), target));
            }
            Some(Row::ClassHeader(c)) => {
                let (picker, target) = spells_tab::class_picker(lib, &sheet.file.character, &data.casters[c], filter);
                sheet.popup = Some(Popup::SpellPick(Box::new(picker), target));
            }
            Some(Row::ClassSpell { ref id, .. } | Row::GrantSpell { ref id, .. }) => {
                let Some(spell) = dnt_rules::overlay::resolve_id(lib, id) else { return Action::None };
                if dnt_rules::spells::spell_level(lib, spell) == 0 {
                    return Action::Status(format!("{} is a cantrip: no slot needed", lib.entity(spell).name));
                }
                // Free (innate) casts first, if the grant has them.
                if let Some(Row::GrantSpell { how: dnt_rules::spells::Grant::Innate(Some(_)), .. }) = &row {
                    return match spells_tab::cast(lib, &mut sheet.file.character, &sheet.ev, spell, "free") {
                        Ok(msg) => {
                            let action = changed(sheet, lib, overlays, msg.clone());
                            cast_roll(sheet, lib, spell, "free", &msg).unwrap_or(action)
                        }
                        Err(msg) => Action::Status(msg),
                    };
                }
                match spells_tab::cast_picker(lib, &sheet.file.character, &sheet.ev, &sheet.build, spell) {
                    Some(p) => sheet.popup = Some(Popup::Cast(Box::new(p), spell)),
                    None => return Action::Status("No spell slots".into()),
                }
            }
            _ => {}
        },
        KeyCode::Char('p') => {
            if let Some(r) = &row {
                match spells_tab::toggle_prepared(&mut sheet.file.character, &data.casters, r) {
                    Some(Ok(on)) => return changed(sheet, lib, overlays, if on { "Prepared".into() } else { "Unprepared".into() }),
                    Some(Err(e)) => return Action::Status(e),
                    None => {}
                }
            }
        }
        KeyCode::Char('x') => {
            if let Some(r) = &row {
                if spells_tab::remove(&mut sheet.file.character, &data.casters, r) {
                    return changed(sheet, lib, overlays, "Removed".into());
                }
            }
        }
        KeyCode::Char('r') => {
            let Some(id) = row.as_ref().and_then(|r| r.spell_id()) else { return Action::None };
            let Some(spell) = dnt_rules::overlay::resolve_id(lib, id) else { return Action::None };
            if !crate::rolls::spell_rolls(lib, spell) {
                return Action::Status(format!("{} has nothing to roll", lib.entity(spell).name));
            }
            let slug = match row.as_ref() {
                Some(Row::ClassSpell { caster, .. }) => data.casters.get(*caster).map(|c| c.slug.clone()),
                _ => None,
            };
            let (atk, dc) = crate::rolls::spell_numbers(sheet, slug.as_deref());
            let level = dnt_rules::spells::spell_level(lib, spell);
            let r = crate::rolls::spell(sheet, lib, spell, level, atk, dc);
            return Action::Rolled(r.line(), sheet.roller.to_notes);
        }
        KeyCode::Char('f') => {
            if let Some(id) = row.as_ref().and_then(|r| r.spell_id()).map(str::to_string) {
                let name = dnt_rules::overlay::resolve_id(lib, &id).map(|e| lib.entity(e).name.clone()).unwrap_or_else(|| id.clone());
                let on = spells_tab::toggle_favorite(&mut sheet.file.character, &id);
                // Keep the cursor on the spell as rows move.
                let rows = spells_tab::rows(lib, &sheet.file.character, &sheet.build, &sheet.ev).rows;
                let at = rows.iter().position(|r| !matches!(r, Row::Favorite(_)) && r.spell_id() == Some(id.as_str()));
                sheet.spell_cursor = at.unwrap_or(sheet.spell_cursor);
                return Action::Save(if on { format!("{name} is a favorite") } else { format!("{name} is no longer a favorite") });
            }
        }
        KeyCode::Char('C') => {
            if let Some(name) = sheet.file.character.state.concentration.take() {
                return changed(sheet, lib, overlays, format!("Stopped concentrating on {name}"));
            }
        }
        _ => {}
    }
    Action::None
}

fn inventory_key(sheet: &mut Sheet, key: KeyEvent, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> Action {
    let n = sheet.file.character.inventory.len();
    let i = sheet.inventory_cursor.min(n.saturating_sub(1));
    // Moving follows the tree (containers, then their contents).
    let order: Vec<usize> = equipment::tree(&sheet.file.character, &sheet.folded).into_iter().map(|(l, _)| l).collect();
    let pos = order.iter().position(|&l| l == i).unwrap_or(0);
    let is_container = |sheet: &Sheet, i: usize| sheet.file.character.inventory.get(i).is_some_and(|l| equipment::container(lib, l).is_some());
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if n > 0 => sheet.inventory_cursor = order[(pos + 1).min(order.len() - 1)],
        KeyCode::Char('k') | KeyCode::Up if n > 0 => sheet.inventory_cursor = order[pos.saturating_sub(1)],
        KeyCode::Home if n > 0 => sheet.inventory_cursor = order[0],
        KeyCode::Char('G') | KeyCode::End if n > 0 => sheet.inventory_cursor = order[order.len() - 1],
        KeyCode::Enter | KeyCode::Right | KeyCode::Left if n > 0 => {
            let c = &sheet.file.character;
            if is_container(sheet, i) {
                // Containers fold like folders.
                let Some(id) = c.inventory[i].id.clone() else { return Action::Status("Empty".into()) };
                let fold = match key.code {
                    KeyCode::Right => false,
                    KeyCode::Left => true,
                    _ => !sheet.folded.contains(&id),
                };
                if fold && key.code == KeyCode::Left && sheet.folded.contains(&id) {
                    if let Some(p) = equipment::parent(c, i) {
                        sheet.inventory_cursor = p;
                    }
                } else if fold {
                    sheet.folded.insert(id);
                } else {
                    sheet.folded.remove(&id);
                }
            } else if key.code == KeyCode::Left {
                if let Some(p) = equipment::parent(c, i) {
                    sheet.inventory_cursor = p;
                }
            }
        }
        KeyCode::Char('M') if n > 0 => {
            let c = &sheet.file.character;
            let mut items = vec![crate::picker::PickItem::new("Carried loose", "not in a container", "loose")];
            for (t, depth) in equipment::tree(c, &Default::default()) {
                let Some(k) = equipment::container(lib, &c.inventory[t]) else { continue };
                // Not into itself or what it holds.
                let mut at = Some(t);
                let mut inside_self = false;
                while let Some(p) = at {
                    inside_self |= p == i;
                    at = equipment::parent(c, p);
                }
                if inside_self {
                    continue;
                }
                let fill = equipment::contents_weight(lib, c, t);
                let hint = match (k.capacity, k.weightless) {
                    (Some(cap), w) => format!("{fill}/{cap} lb{}", if w { " · weightless" } else { "" }),
                    (None, w) => format!("{fill} lb{}", if w { " · weightless" } else { "" }),
                };
                items.push(crate::picker::PickItem::new(format!("{}{}", "  ".repeat(depth), line_name(lib, &c.inventory[t])), hint, t.to_string()));
            }
            let name = line_name(lib, &c.inventory[i]);
            sheet.popup = Some(Popup::MoveInto(Box::new(Picker::new(format!("Put {name} in…"), items)), i));
        }
        KeyCode::Char('p') if n > 0 => {
            return match equipment::unpack(lib, &mut sheet.file.character, i) {
                Ok(msg) => {
                    sheet.inventory_cursor = sheet.inventory_cursor.min(sheet.file.character.inventory.len().saturating_sub(1));
                    changed(sheet, lib, overlays, msg)
                }
                Err(e) => Action::Status(e),
            };
        }
        KeyCode::Char('w') if n > 0 => {
            let line = &sheet.file.character.inventory[i];
            let current = match equipment::container(lib, line) {
                Some(k) => {
                    let mut t = k.capacity.map(|w| format!("{w}")).unwrap_or_else(|| "no limit".into());
                    if k.weightless {
                        t.push_str(" weightless");
                    }
                    t
                }
                None => String::new(),
            };
            sheet.popup = Some(Popup::Text(TextPrompt::Container, current));
        }
        KeyCode::Char('a') => {
            let mut items = vec![
                crate::picker::PickItem::new("Create new item…", "a campaign item with stats", "create"),
                crate::picker::PickItem::new("Custom item…", "just a name and a weight", "custom"),
            ];
            items.extend(equipment_catalog(lib, filter));
            sheet.popup = Some(Popup::AddItem(Box::new(Picker::new("Add an item", items))));
        }
        KeyCode::Char('g') => sheet.popup = Some(Popup::Text(TextPrompt::Currency, String::new())),
        KeyCode::Char('m') if n > 0 => {
            // Mark or unmark a monk weapon; back to the rules when they agree.
            let line = &sheet.file.character.inventory[i];
            let Some(e) = item_entity(lib, line) else { return Action::None };
            let ent = lib.entity(e);
            let Some(w) = dnt_rules::attacks::Weapon::from_item(&ent.name.to_lowercase(), &ent.data) else {
                return Action::Status(format!("{} is not a weapon", ent.name));
            };
            let preset = sheet.build.martial_arts.is_some_and(|m| m.preset(&w));
            let now = !line.monk_weapon.unwrap_or(preset);
            let name = ent.name.clone();
            sheet.file.character.inventory[i].monk_weapon = (now != preset).then_some(now);
            let msg = match (now, sheet.build.martial_arts.is_some()) {
                (true, true) => format!("{name} is a monk weapon"),
                (true, false) => format!("{name} is marked as a monk weapon (it counts once you have Martial Arts)"),
                (false, _) => format!("{name} is not a monk weapon"),
            };
            return changed(sheet, lib, overlays, msg);
        }
        KeyCode::Char('+' | '=') if n > 0 => {
            sheet.file.character.inventory[i].quantity += 1;
            return changed(sheet, lib, overlays, "Quantity +1".into());
        }
        KeyCode::Char('-') if n > 0 => {
            let line = &mut sheet.file.character.inventory[i];
            if line.quantity > 1 {
                line.quantity -= 1;
                return changed(sheet, lib, overlays, "Quantity -1".into());
            }
        }
        KeyCode::Char('x') if n > 0 => {
            // What was inside a removed container stays, where it was.
            let next = order.get(pos + 1).or(pos.checked_sub(1).and_then(|p| order.get(p))).copied();
            let removed = equipment::remove_line(&mut sheet.file.character, i);
            sheet.inventory_cursor = next.map(|j| if j > i { j - 1 } else { j }).unwrap_or(0);
            return changed(sheet, lib, overlays, format!("Removed {}", line_name(lib, &removed)));
        }
        KeyCode::Char(' ') if n > 0 => {
            let c = &sheet.file.character;
            let line = &c.inventory[i];
            let data = item_entity(lib, line).map(|e| &lib.entity(e).data);
            // Weapons and shields are held; the rest is worn in a slot.
            if data.is_some_and(equipment::is_held) {
                let shield = data.and_then(equipment::armor_kind) == Some("shield");
                return hold(sheet, lib, overlays, i, if shield { Hand::Off } else { Hand::Main });
            }
            if line.equipped {
                equipment::take_off(&mut sheet.file.character, i);
                let name = line_name(lib, &sheet.file.character.inventory[i]);
                return changed(sheet, lib, overlays, format!("Took off {name}"));
            }
            let picker = wear_picker(lib, c, i);
            sheet.popup = Some(Popup::Wear(Box::new(picker), i));
        }
        KeyCode::Char('o') if n > 0 => {
            let data = item_entity(lib, &sheet.file.character.inventory[i]).map(|e| &lib.entity(e).data);
            if !data.is_some_and(equipment::is_held) {
                return Action::Status("Only weapons and shields are held".into());
            }
            return hold(sheet, lib, overlays, i, Hand::Off);
        }
        KeyCode::Char('A') if n > 0 => {
            let line = &mut sheet.file.character.inventory[i];
            line.attuned = !line.attuned;
            let mut msg = format!("{} {}", if line.attuned { "Attuned to" } else { "No longer attuned to" }, line_name(lib, line));
            // Its magic works only while worn (or held).
            if line.attuned && !line.equipped && line.hand.is_none() {
                msg.push_str(" (not worn: space to wear it)");
            }
            return changed(sheet, lib, overlays, msg);
        }
        _ => {}
    }
    Action::None
}

/// Take an inventory line in hand (or put it away), keeping the overview
/// cursor on the same attack as the list reorders.
fn hold(sheet: &mut Sheet, lib: &Library, overlays: &OverlaySet, i: usize, hand: Hand) -> Action {
    let before = sheet.current_item();
    let msg = equipment::wield(lib, &mut sheet.file.character, i, hand);
    let action = changed(sheet, lib, overlays, msg);
    if let Some(item @ Item::Attack(_)) = before {
        if let Some(at) = sheet.items().iter().position(|x| *x == item) {
            sheet.cursor = at;
        }
    }
    action
}

/// The inventory line behind an attack row.
fn attack_line(sheet: &Sheet, lib: &Library, slug: &str) -> Option<usize> {
    let a = sheet.build.attacks.iter().find(|a| a.slug == slug)?;
    let id = a.item?;
    let inv = &sheet.file.character.inventory;
    inv.iter()
        .position(|l| item_entity(lib, l) == Some(id) && line_name(lib, l) == a.name)
        .or_else(|| inv.iter().position(|l| item_entity(lib, l) == Some(id)))
}

/// Where to wear a line: every slot, with what it would replace; the
/// likely slot is preselected.
fn wear_picker(lib: &Library, c: &dnt_rules::character::Character, i: usize) -> Picker {
    let line = &c.inventory[i];
    let name = line_name(lib, line);
    let guess = item_entity(lib, line).map(|e| equipment::guess_slot(c, &name, &lib.entity(e).data)).unwrap_or(Slot::Other);
    let items: Vec<crate::picker::PickItem> = Slot::ALL
        .iter()
        .enumerate()
        .map(|(n, &slot)| {
            let occupant = c
                .inventory
                .iter()
                .enumerate()
                .find(|(j, l)| *j != i && slot != Slot::Other && l.equipped && l.slot == Some(slot))
                .map(|(_, l)| line_name(lib, l));
            let label = match slot {
                Slot::Ring1 => "Ring 1".to_string(),
                Slot::Ring2 => "Ring 2".to_string(),
                s => s.label().to_string(),
            };
            let hint = occupant.map(|o| format!("takes off {o}")).unwrap_or_else(|| slot.examples().to_string());
            crate::picker::PickItem::new(label, hint, n.to_string())
        })
        .collect();
    let mut picker = Picker::new(format!("Wear {name}"), items);
    picker.selected = Slot::ALL.iter().position(|s| *s == guess).unwrap_or(0);
    picker
}

/// Every item and base item enabled by the campaign, for the add picker.
fn equipment_catalog(lib: &Library, filter: &SourceFilter) -> Vec<crate::picker::PickItem> {
    use dnt_data::Kind;
    let mut items: Vec<crate::picker::PickItem> = lib
        .entities()
        .iter()
                // Generic variants ("+1 Weapon") are offered as their specific items.
        .filter(|e| matches!(e.kind, Kind::Item | Kind::BaseItem) && filter.allows(e))
        .map(|e| {
            let rarity = e
                .data
                .get("rarity")
                .or_else(|| e.data.get("inherits").and_then(|i| i.get("rarity")))
                .and_then(|r| r.as_str())
                .filter(|r| !matches!(*r, "none" | "unknown"))
                .unwrap_or("");
            crate::picker::PickItem::new(e.name.clone(), format!("{rarity} {}", e.source).trim().to_string(), format!("item:{}|{}", e.name, e.source))
        })
        .collect();
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

pub fn item_entity(lib: &Library, line: &dnt_rules::character::InventoryItem) -> Option<dnt_data::EntityId> {
    dnt_rules::overlay::resolve_id(lib, line.item.as_deref()?)
}

pub fn line_name(lib: &Library, line: &dnt_rules::character::InventoryItem) -> String {
    line.name
        .clone()
        .or_else(|| item_entity(lib, line).map(|e| lib.entity(e).name.clone()))
        .or_else(|| line.item.clone())
        .unwrap_or_else(|| "?".into())
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
