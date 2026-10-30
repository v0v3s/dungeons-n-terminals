//! The rules library (L on Campaigns): its folders, files and rules as a
//! tree to check, then imported as the user's own rules; rules that
//! differ from the user's ask, each or all at once, to overwrite or keep.

use std::collections::BTreeSet;

use dnt_core::rules_library::{self as library, RulesLibrary, Status};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;
use crate::ui::{pane, pane_title};

pub struct LibraryView {
    /// The library's files, to see them again against the user's rules.
    pub files: Vec<(String, String)>,
    pub lib: RulesLibrary,
    checked: BTreeSet<String>,
    /// Folded folders and files (by name or path).
    folded: BTreeSet<String>,
    cursor: usize,
    /// Asking about the rules that differ: (id, label, overwrite).
    conflicts: Option<Vec<(String, String, bool)>>,
    conflict_cursor: usize,
}

pub enum Action {
    None,
    Close,
    Status(String),
    /// Import these ids, overwriting the user's rules for those in the second.
    Import(BTreeSet<String>, BTreeSet<String>),
}

/// A row of the tree: a folder, a file, or a rule (indexes into the tree).
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Folder(usize),
    File(usize, usize),
    Rule(usize, usize, usize),
}

impl LibraryView {
    /// Folded at first: the folders, then what is in one when unfolded.
    pub fn new(files: Vec<(String, String)>, lib: RulesLibrary) -> LibraryView {
        let folded =
            lib.folders.iter().flat_map(|f| std::iter::once(f.name.clone()).chain(f.files.iter().map(|x| x.path.clone()))).collect();
        LibraryView { files, lib, checked: BTreeSet::new(), folded, cursor: 0, conflicts: None, conflict_cursor: 0 }
    }

    /// After an import: the statuses again, nothing checked.
    pub fn refresh(&mut self, lib: RulesLibrary) {
        self.lib = lib;
        self.checked.clear();
        self.conflicts = None;
    }

    fn rows(&self) -> Vec<Row> {
        let mut out = Vec::new();
        for (i, folder) in self.lib.folders.iter().enumerate() {
            out.push(Row::Folder(i));
            if self.folded.contains(&folder.name) {
                continue;
            }
            for (j, file) in folder.files.iter().enumerate() {
                out.push(Row::File(i, j));
                if self.folded.contains(&file.path) {
                    continue;
                }
                out.extend((0..file.rules.len()).map(|k| Row::Rule(i, j, k)));
            }
        }
        out
    }

    /// The rule ids under a row.
    fn ids(&self, row: Row) -> Vec<String> {
        let folder = |i: usize| &self.lib.folders[i];
        match row {
            Row::Folder(i) => folder(i).files.iter().flat_map(|f| &f.rules).map(|r| r.id.clone()).collect(),
            Row::File(i, j) => folder(i).files[j].rules.iter().map(|r| r.id.clone()).collect(),
            Row::Rule(i, j, k) => vec![folder(i).files[j].rules[k].id.clone()],
        }
    }

    /// ● all checked, ◐ some, ○ none.
    fn mark(&self, row: Row) -> &'static str {
        let ids = self.ids(row);
        let n = ids.iter().filter(|id| self.checked.contains(*id)).count();
        if n == 0 {
            "○"
        } else if n == ids.len() {
            "●"
        } else {
            "◐"
        }
    }

    fn key_of(&self, row: Row) -> Option<String> {
        match row {
            Row::Folder(i) => Some(self.lib.folders[i].name.clone()),
            Row::File(i, j) => Some(self.lib.folders[i].files[j].path.clone()),
            Row::Rule(..) => None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if self.conflicts.is_some() {
            return self.conflict_key(key);
        }
        let rows = self.rows();
        let n = rows.len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
        let row = rows.get(self.cursor).copied();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('j') | KeyCode::Down if n > 0 => self.cursor = (self.cursor + 1).min(n - 1),
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => self.cursor = 0,
            KeyCode::Char('G') | KeyCode::End => self.cursor = n.saturating_sub(1),
            KeyCode::Char(' ') => {
                if let Some(row) = row {
                    let ids = self.ids(row);
                    if self.mark(row) == "●" {
                        ids.iter().for_each(|id| {
                            self.checked.remove(id);
                        });
                    } else {
                        self.checked.extend(ids);
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => match row.and_then(|r| self.key_of(r)) {
                Some(k) => {
                    self.folded.insert(k);
                }
                // On a rule: to its file.
                None => {
                    if let Some(Row::Rule(i, j, _)) = row {
                        self.cursor = rows.iter().position(|r| *r == Row::File(i, j)).unwrap_or(self.cursor);
                    }
                }
            },
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(k) = row.and_then(|r| self.key_of(r)) {
                    self.folded.remove(&k);
                }
            }
            KeyCode::Enter => {
                if self.checked.is_empty() {
                    return Action::Status("Check rules to import: space on a folder, a file or a rule".into());
                }
                let differ: Vec<(String, String, bool)> = self
                    .lib
                    .folders
                    .iter()
                    .flat_map(|f| &f.files)
                    .flat_map(|f| &f.rules)
                    .filter(|r| self.checked.contains(&r.id) && r.status == Status::Differs)
                    .map(|r| (r.id.clone(), r.label.clone(), false))
                    .collect();
                if differ.is_empty() {
                    return Action::Import(self.checked.clone(), BTreeSet::new());
                }
                self.conflicts = Some(differ);
                self.conflict_cursor = 0;
            }
            _ => {}
        }
        Action::None
    }

    fn conflict_key(&mut self, key: KeyEvent) -> Action {
        let list = self.conflicts.as_mut().expect("asking");
        let n = list.len();
        match key.code {
            KeyCode::Esc => self.conflicts = None,
            // `k` keeps: up is the arrow.
            KeyCode::Char('j') | KeyCode::Down => self.conflict_cursor = (self.conflict_cursor + 1).min(n - 1),
            KeyCode::Up => self.conflict_cursor = self.conflict_cursor.saturating_sub(1),
            KeyCode::Char('o') => list[self.conflict_cursor].2 = true,
            KeyCode::Char('k') => list[self.conflict_cursor].2 = false,
            KeyCode::Char(' ') => list[self.conflict_cursor].2 = !list[self.conflict_cursor].2,
            KeyCode::Char('O') => list.iter_mut().for_each(|c| c.2 = true),
            KeyCode::Char('K') => list.iter_mut().for_each(|c| c.2 = false),
            KeyCode::Enter => {
                let overwrite = list.iter().filter(|c| c.2).map(|c| c.0.clone()).collect();
                return Action::Import(self.checked.clone(), overwrite);
            }
            _ => {}
        }
        Action::None
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        frame.render_widget(Clear, area);
        let asking = self.conflicts.is_some();
        let keys = if asking {
            " j ↑ move · o overwrite · k keep · O overwrite all · K keep all · enter import · esc back "
        } else {
            " space check · h l fold · enter import · esc close "
        };
        let block = pane(pane_title("Rules library", true, theme), true, theme)
            .padding(Padding::new(2, 2, 1, 0))
            .title_bottom(Line::styled(keys, theme.dim).right_aligned());
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut lines: Vec<Line> = Vec::new();
        if let Some(list) = &self.conflicts {
            lines.push(Line::styled("These differ from your rules for the same features: overwrite yours, or keep them?", theme.label));
            lines.push(Line::default());
            for (i, (_, label, overwrite)) in list.iter().enumerate() {
                let style = if i == self.conflict_cursor { theme.selected } else { Style::new() };
                let on = |yes: bool, text: &'static str| {
                    if yes {
                        Span::styled(format!("‹{text}›"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled(format!(" {text} "), theme.dim)
                    }
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{label:<44}"), style),
                    on(*overwrite, "overwrite"),
                    Span::raw("  "),
                    on(!*overwrite, "keep"),
                ]));
            }
        } else {
            lines.push(Line::styled(format!("Rules for features, from {}", source_name()), theme.muted));
            lines.push(Line::default());
            let rows = self.rows();
            let height = inner.height.saturating_sub(2) as usize;
            let start = self.cursor.saturating_sub(height.saturating_sub(1));
            for (n, &row) in rows.iter().enumerate().skip(start).take(height) {
                let style = if n == self.cursor { theme.selected } else { Style::new() };
                let fold = |k: Option<String>| if k.is_some_and(|k| self.folded.contains(&k)) { "▸ " } else { "▾ " };
                let line = match row {
                    Row::Folder(i) => {
                        let f = &self.lib.folders[i];
                        let n: usize = f.files.iter().map(|x| x.rules.len()).sum();
                        vec![
                            Span::raw(fold(self.key_of(row))),
                            Span::raw(format!("{} ", self.mark(row))),
                            Span::styled(f.label.clone(), theme.label),
                            Span::styled(format!("  {n} rule(s)"), theme.dim),
                        ]
                    }
                    Row::File(i, j) => {
                        let f = &self.lib.folders[i].files[j];
                        vec![
                            Span::raw("  "),
                            Span::raw(fold(self.key_of(row))),
                            Span::raw(format!("{} ", self.mark(row))),
                            Span::raw(f.label.clone()),
                            Span::styled(format!("  {} rule(s)", f.rules.len()), theme.dim),
                        ]
                    }
                    Row::Rule(i, j, k) => {
                        let r = &self.lib.folders[i].files[j].rules[k];
                        vec![
                            Span::raw(format!("      {} ", self.mark(row))),
                            Span::raw(format!("{:<44}", r.label)),
                            Span::styled(library::status_text(r), theme.dim),
                        ]
                    }
                };
                lines.push(Line::from(line).style(style));
            }
            if rows.is_empty() {
                lines.push(Line::styled("The library has no rules.", theme.muted));
            }
        }
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// The library's place, shortly: `github.com/v0v3s/dnt-rules` or the
/// folder or address set.
fn source_name() -> String {
    let at = library::location();
    if at == library::DEFAULT_URL {
        "github.com/v0v3s/dnt-rules".into()
    } else {
        at
    }
}
