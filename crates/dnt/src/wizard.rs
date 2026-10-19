//! The character creation wizard: shows the questions of
//! `dnt_core::creation` as pickers and the ability score editor. Esc goes
//! back a step.

use dnt_core::creation::NewCharacter;
use dnt_core::flow::{AbilityScores, Answer, Question};
use dnt_data::{EntityId, Library, SourceFilter};
use dnt_rules::character::Character;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::levelup::question_picker;
use crate::picker::{Picked, Picker};

/// Ability scores being set, with the highlighted ability.
#[derive(Debug, Clone)]
pub struct AbilityEditor {
    pub scores: AbilityScores,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub enum View {
    Pick(Picker),
    Abilities(AbilityEditor),
    Review,
}

pub struct Wizard {
    flow: NewCharacter,
    view: View,
}

pub enum Outcome {
    Continue,
    Cancel,
    Finish(Box<Character>),
}

fn view(q: &Question) -> View {
    match q {
        Question::Pick { .. } => View::Pick(question_picker(q).expect("a pick question")),
        Question::Abilities(scores) => View::Abilities(AbilityEditor { scores: scores.clone(), cursor: 0 }),
        Question::Number { .. } | Question::Confirm => View::Review,
    }
}

impl Wizard {
    pub fn new(name: String, lib: &Library, filter: &SourceFilter) -> Wizard {
        let flow = NewCharacter::new(name, lib, filter);
        Wizard { view: view(flow.question()), flow }
    }

    pub fn name(&self) -> &str {
        self.flow.name()
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn summary(&self) -> &[(String, String)] {
        self.flow.summary()
    }

    pub fn progress(&self) -> Vec<(&'static str, bool, bool)> {
        self.flow.progress()
    }

    fn highlighted(&self) -> Option<&str> {
        match &self.view {
            View::Pick(p) => p.current().map(|i| i.value.as_str()),
            _ => None,
        }
    }

    /// Contents of the highlighted starting-equipment package, if choosing one.
    pub fn equipment_preview(&self, lib: &Library) -> Option<(String, Vec<String>)> {
        self.flow.equipment_preview(lib, self.highlighted()?)
    }

    /// Entity to preview for the highlighted choice, if any.
    pub fn preview(&self, lib: &Library) -> Option<EntityId> {
        self.flow.preview(lib, self.highlighted()?)
    }

    /// The character as it would be created now (for the review preview).
    pub fn preview_character(&self) -> Character {
        let scores = match &self.view {
            View::Abilities(ed) => Some(&ed.scores.scores),
            _ => None,
        };
        self.flow.preview_character(scores)
    }

    pub fn key(&mut self, key: KeyEvent, lib: &Library, filter: &SourceFilter) -> Outcome {
        if key.code == KeyCode::Esc {
            if !self.flow.back() {
                return Outcome::Cancel;
            }
            self.view = view(self.flow.question());
            return Outcome::Continue;
        }
        let answer = match &mut self.view {
            View::Review => {
                if key.code == KeyCode::Enter {
                    if let Ok(ch) = self.flow.finish() {
                        return Outcome::Finish(Box::new(ch));
                    }
                }
                return Outcome::Continue;
            }
            View::Abilities(ed) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    ed.cursor = (ed.cursor + 1).min(5);
                    return Outcome::Continue;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    ed.cursor = ed.cursor.saturating_sub(1);
                    return Outcome::Continue;
                }
                KeyCode::Char('l' | '+' | '=') | KeyCode::Right => {
                    ed.scores.adjust(ed.cursor, true);
                    return Outcome::Continue;
                }
                KeyCode::Char('h' | '-') | KeyCode::Left => {
                    ed.scores.adjust(ed.cursor, false);
                    return Outcome::Continue;
                }
                KeyCode::Enter => Answer::Scores(ed.scores.scores),
                _ => return Outcome::Continue,
            },
            View::Pick(p) => match p.key(key) {
                Some(Picked::One(v)) => Answer::Pick(vec![v]),
                Some(Picked::Many(vs)) => Answer::Pick(vs),
                None => return Outcome::Continue,
            },
        };
        if self.flow.answer(lib, filter, answer).is_ok() {
            self.view = view(self.flow.question());
        }
        Outcome::Continue
    }
}
