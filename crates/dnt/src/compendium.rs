//! Compendium browser state and key handling.

pub use dnt_core::compendium::Category;
use dnt_core::compendium::Search;
use dnt_data::tags::Link;
use dnt_data::{doc::Block, view, EntityId, Library, SourceFilter};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    List,
    Detail,
}

/// The entity shown in the detail pane, and where we were in it.
pub struct DetailView {
    pub entity: EntityId,
    pub doc: Vec<Block>,
    /// Showing the book version of a campaign override.
    pub original: bool,
    pub scroll: usize,
    pub focused_link: Option<usize>,
}

impl DetailView {
    fn new(lib: &Library, entity: EntityId, filter: &SourceFilter) -> DetailView {
        DetailView { entity, doc: view::document(lib, entity, filter), original: false, scroll: 0, focused_link: None }
    }
}

pub enum Outcome {
    None,
    Quit,
    Status(String),
    /// Clone this entity into the campaign's homebrew.
    Clone(EntityId),
    /// Edit this campaign entity.
    Edit(EntityId),
    /// Create a new campaign entity.
    New,
}

pub struct Compendium {
    pub search: Search,
    pub selected: usize,
    pub list_offset: usize,
    pub focus: Focus,
    /// The detail pane; follows the list selection until a link is followed.
    pub detail: Option<DetailView>,
    /// Back stack of followed links.
    pub history: Vec<DetailView>,
    /// Set by the UI each frame: rendered links (with their line) and viewport size.
    pub detail_links: Vec<(Link, usize)>,
    pub detail_height: usize,
    pub detail_lines: usize,
    pub list_height: usize,
}

/// Reading the search's query, results and counts.
impl std::ops::Deref for Compendium {
    type Target = Search;
    fn deref(&self) -> &Search {
        &self.search
    }
}

impl Compendium {
    pub fn new(lib: &Library, filter: SourceFilter) -> Compendium {
        let mut c = Compendium {
            search: Search::new(lib, filter),
            selected: 0,
            list_offset: 0,
            focus: Focus::List,
            detail: None,
            history: Vec::new(),
            detail_links: Vec::new(),
            detail_height: 0,
            detail_lines: 0,
            list_height: 0,
        };
        c.searched(lib);
        c
    }

    /// Restore the category and query after a rebuild.
    pub fn restore(&mut self, lib: &Library, category: Category, query: String) {
        self.search.query = query;
        self.search.set_category(lib, category);
        self.searched(lib);
    }

    /// Show an entity in the detail pane (e.g. from a note link).
    pub fn show(&mut self, lib: &Library, id: EntityId) {
        let next = DetailView::new(lib, id, &self.filter);
        if let Some(prev) = self.detail.replace(next) {
            self.history.push(prev);
        }
        self.focus = Focus::Detail;
    }

    pub fn set_filter(&mut self, lib: &Library, filter: SourceFilter) {
        self.search.set_filter(lib, filter);
        self.searched(lib);
    }

    /// Search again after the query changed.
    fn research(&mut self, lib: &Library) {
        self.search.run(lib);
        self.searched(lib);
    }

    /// New results: back to the top.
    fn searched(&mut self, lib: &Library) {
        self.selected = 0;
        self.list_offset = 0;
        self.sync_detail(lib);
    }

    fn complete(&mut self, lib: &Library) -> bool {
        let done = self.search.complete(lib);
        if done {
            self.searched(lib);
        }
        done
    }

    /// Show the selected result in the detail pane (preview behaviour).
    fn sync_detail(&mut self, lib: &Library) {
        self.history.clear();
        self.detail = self.results.get(self.selected).map(|&id| DetailView::new(lib, id, &self.filter));
    }

    fn move_selection(&mut self, lib: &Library, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let max = self.results.len() as isize - 1;
        let new = (self.selected as isize + delta).clamp(0, max) as usize;
        if new != self.selected {
            self.selected = new;
            self.sync_detail(lib);
        }
    }

    /// Whether keystrokes are currently text input (so global keys must not fire).
    pub fn wants_text(&self) -> bool {
        self.focus == Focus::Search
    }

    pub fn handle_key(&mut self, lib: &Library, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Outcome::Quit;
        }
        match key.code {
            KeyCode::Tab if self.focus == Focus::Search && self.complete(lib) => return Outcome::None,
            KeyCode::Tab if self.focus != Focus::Detail => return self.set_category(lib, self.category.cycle(1)),
            KeyCode::BackTab if self.focus != Focus::Detail => return self.set_category(lib, self.category.cycle(-1)),
            _ => {}
        }
        match self.focus {
            Focus::Search => self.key_search(lib, key, ctrl),
            Focus::List => self.key_list(lib, key, ctrl),
            Focus::Detail => self.key_detail(lib, key, ctrl),
        }
    }

    fn set_category(&mut self, lib: &Library, category: Category) -> Outcome {
        self.search.set_category(lib, category);
        self.searched(lib);
        Outcome::None
    }

    fn key_search(&mut self, lib: &Library, key: KeyEvent, ctrl: bool) -> Outcome {
        let half = (self.list_height / 2).max(1) as isize;
        match key.code {
            KeyCode::Esc => self.focus = Focus::List,
            KeyCode::Enter => {
                if self.detail.is_some() {
                    self.focus = Focus::Detail;
                }
            }
            KeyCode::Down => self.move_selection(lib, 1),
            KeyCode::Up => self.move_selection(lib, -1),
            KeyCode::Char('n' | 'j') if ctrl => self.move_selection(lib, 1),
            KeyCode::Char('p' | 'k') if ctrl => self.move_selection(lib, -1),
            KeyCode::Char('d') if ctrl => self.move_selection(lib, half),
            KeyCode::Char('u') if ctrl => {
                if self.query.is_empty() {
                    self.move_selection(lib, -half);
                } else {
                    self.search.query.clear();
                    self.research(lib);
                }
            }
            KeyCode::Char('w') if ctrl => {
                let trimmed = self.query.trim_end().len();
                let cut = self.query[..trimmed].rfind(' ').map_or(0, |i| i + 1);
                self.search.query.truncate(cut);
                self.research(lib);
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.research(lib);
            }
            KeyCode::Char(c) if !ctrl => {
                self.search.query.push(c);
                self.research(lib);
            }
            _ => {}
        }
        Outcome::None
    }

    fn key_list(&mut self, lib: &Library, key: KeyEvent, ctrl: bool) -> Outcome {
        let half = (self.list_height / 2).max(1) as isize;
        match key.code {
            KeyCode::Char('q') => return Outcome::Quit,
            KeyCode::Char('c') => {
                if let Some(&id) = self.results.get(self.selected) {
                    return Outcome::Clone(id);
                }
            }
            KeyCode::Char('e') => {
                if let Some(&id) = self.results.get(self.selected) {
                    return Outcome::Edit(id);
                }
            }
            KeyCode::Char('n') => return Outcome::New,
            KeyCode::Char('/' | 'i') => self.focus = Focus::Search,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(lib, 1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(lib, -1),
            KeyCode::Char('d') if ctrl => self.move_selection(lib, half),
            KeyCode::Char('u') if ctrl => self.move_selection(lib, -half),
            KeyCode::PageDown => self.move_selection(lib, half * 2),
            KeyCode::PageUp => self.move_selection(lib, -half * 2),
            KeyCode::Char('g') | KeyCode::Home => self.move_selection(lib, isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.move_selection(lib, isize::MAX / 2),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if self.detail.is_some() {
                    self.focus = Focus::Detail;
                }
            }
            KeyCode::Esc if !self.query.is_empty() => {
                self.search.query.clear();
                self.research(lib);
            }
            _ => {}
        }
        Outcome::None
    }

    fn key_detail(&mut self, lib: &Library, key: KeyEvent, ctrl: bool) -> Outcome {
        let half = (self.detail_height / 2).max(1);
        let Some(detail) = self.detail.as_mut() else {
            self.focus = Focus::List;
            return Outcome::None;
        };
        let max_scroll = self.detail_lines.saturating_sub(self.detail_height);
        match key.code {
            KeyCode::Char('q') => return Outcome::Quit,
            KeyCode::Char('/') => self.focus = Focus::Search,
            KeyCode::Char('j') | KeyCode::Down => detail.scroll = (detail.scroll + 1).min(max_scroll),
            KeyCode::Char('k') | KeyCode::Up => detail.scroll = detail.scroll.saturating_sub(1),
            KeyCode::Char('d') if ctrl => detail.scroll = (detail.scroll + half).min(max_scroll),
            KeyCode::Char('u') if ctrl => detail.scroll = detail.scroll.saturating_sub(half),
            KeyCode::PageDown | KeyCode::Char(' ') => detail.scroll = (detail.scroll + half * 2).min(max_scroll),
            KeyCode::PageUp => detail.scroll = detail.scroll.saturating_sub(half * 2),
            KeyCode::Char('g') | KeyCode::Home => detail.scroll = 0,
            KeyCode::Char('G') | KeyCode::End => detail.scroll = max_scroll,
            KeyCode::Char('c') if !ctrl => return Outcome::Clone(detail.entity),
            KeyCode::Char('e') if !ctrl => return Outcome::Edit(detail.entity),
            KeyCode::Char('o') if !ctrl => {
                // Toggle the book version of a campaign override.
                let Some(orig) = lib.original(detail.entity) else {
                    return Outcome::Status("Not a campaign version".into());
                };
                detail.original = !detail.original;
                detail.doc = if detail.original {
                    view::document_of(lib, detail.entity, orig, &self.search.filter)
                } else {
                    view::document(lib, detail.entity, &self.search.filter)
                };
                detail.scroll = 0;
                detail.focused_link = None;
            }
            KeyCode::Tab | KeyCode::Char('n') => self.cycle_link(1),
            KeyCode::BackTab | KeyCode::Char('N') => self.cycle_link(-1),
            KeyCode::Enter => return self.follow_link(lib),
            KeyCode::Backspace => self.back(),
            KeyCode::Char('o') if ctrl => self.back(),
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
                if detail.focused_link.is_some() && key.code == KeyCode::Esc {
                    detail.focused_link = None;
                } else if !self.history.is_empty() && key.code != KeyCode::Esc {
                    self.back();
                } else {
                    self.focus = Focus::List;
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// Focus the next/previous link, preferring links in the visible area.
    fn cycle_link(&mut self, delta: isize) {
        let Some(detail) = self.detail.as_mut() else { return };
        let n = self.detail_links.len();
        if n == 0 {
            return;
        }
        let next = match detail.focused_link {
            Some(i) => (i as isize + delta).rem_euclid(n as isize) as usize,
            None => {
                // Start from the first link on screen.
                let top = detail.scroll;
                let on_screen = self.detail_links.iter().position(|(_, line)| *line >= top);
                match (delta > 0, on_screen) {
                    (true, Some(i)) => i,
                    (true, None) => 0,
                    (false, _) => self.detail_links.iter().rposition(|(_, line)| *line < top + self.detail_height).unwrap_or(n - 1),
                }
            }
        };
        detail.focused_link = Some(next);
        let line = self.detail_links[next].1;
        if line < detail.scroll || line >= detail.scroll + self.detail_height {
            detail.scroll = line.saturating_sub(self.detail_height / 3);
        }
    }

    fn follow_link(&mut self, lib: &Library) -> Outcome {
        let Some(detail) = self.detail.as_ref() else { return Outcome::None };
        let Some(i) = detail.focused_link else { return Outcome::None };
        let Some((link, _)) = self.detail_links.get(i) else { return Outcome::None };
        match lib.resolve(link) {
            Some(target) => {
                let next = DetailView::new(lib, target, &self.filter);
                let prev = self.detail.replace(next).unwrap();
                self.history.push(prev);
                Outcome::None
            }
            None => Outcome::Status(format!("Not in library: {}", link.args.split('|').next().unwrap_or(""))),
        }
    }

    fn back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.detail = Some(prev);
        }
    }
}
