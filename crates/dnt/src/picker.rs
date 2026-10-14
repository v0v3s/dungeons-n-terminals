//! A filterable choice list used by the level-up flow and creation wizard.
//! Supports picking one item, or exactly N items (multi-select).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub struct PickItem {
    pub label: String,
    pub hint: String,
    pub warning: Option<String>,
    pub value: String,
}

impl PickItem {
    pub fn new(label: impl Into<String>, hint: impl Into<String>, value: impl Into<String>) -> PickItem {
        PickItem {
            label: label.into(),
            hint: hint.into(),
            warning: None,
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Picker {
    pub title: String,
    pub items: Vec<PickItem>,
    pub query: String,
    pub selected: usize,
    /// `Some(n)`: pick exactly `n` items (space toggles).
    pub multi: Option<usize>,
    pub checked: Vec<String>,
}

pub enum Picked {
    One(String),
    Many(Vec<String>),
}

impl Picker {
    pub fn new(title: impl Into<String>, items: Vec<PickItem>) -> Picker {
        Picker {
            title: title.into(),
            items,
            query: String::new(),
            selected: 0,
            multi: None,
            checked: Vec::new(),
        }
    }

    pub fn multi(title: impl Into<String>, items: Vec<PickItem>, count: usize) -> Picker {
        let mut p = Picker::new(title, items);
        p.multi = Some(count.min(p.items.len()));
        p
    }

    /// Indices of items matching the query (all words, any order).
    pub fn visible(&self) -> Vec<usize> {
        let words: Vec<String> = self.query.to_lowercase().split_whitespace().map(str::to_string).collect();
        (0..self.items.len())
            .filter(|&i| {
                let hay = format!("{} {}", self.items[i].label, self.items[i].hint).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .collect()
    }

    pub fn current(&self) -> Option<&PickItem> {
        self.visible().get(self.selected).map(|&i| &self.items[i])
    }

    pub fn is_checked(&self, value: &str) -> bool {
        self.checked.iter().any(|v| v == value)
    }

    pub fn key(&mut self, key: KeyEvent) -> Option<Picked> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.visible().len();
        match key.code {
            KeyCode::Down | KeyCode::Tab => self.selected = (self.selected + 1).min(n.saturating_sub(1)),
            KeyCode::Up | KeyCode::BackTab => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('n' | 'j') if ctrl => self.selected = (self.selected + 1).min(n.saturating_sub(1)),
            KeyCode::Char('p' | 'k') if ctrl => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('u') if ctrl => {
                self.query.clear();
                self.selected = 0;
            }
            KeyCode::Char(' ') if self.multi.is_some() => {
                let value = self.current().map(|i| i.value.clone())?;
                if let Some(pos) = self.checked.iter().position(|v| *v == value) {
                    self.checked.remove(pos);
                } else if self.checked.len() < self.multi.unwrap_or(0) {
                    self.checked.push(value);
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.selected = 0;
            }
            KeyCode::Enter => match self.multi {
                Some(count) if self.checked.len() == count => return Some(Picked::Many(self.checked.clone())),
                Some(_) => {}
                None => return self.current().map(|i| Picked::One(i.value.clone())),
            },
            _ => {}
        }
        None
    }
}
