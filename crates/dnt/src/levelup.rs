//! The level-up popup: shows the questions of `dnt_core::levelup` as
//! pickers and prompts. Also used for a character's very first level.

use dnt_core::flow::{self, Answer, Question};
use dnt_core::levelup;
use dnt_data::{Library, SourceFilter};
use dnt_rules::character::Character;
use dnt_rules::Evaluation;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::picker::{Picked, Picker};

#[derive(Debug, Clone)]
pub enum Stage {
    Pick(Picker),
    /// Typing a rolled hit point value.
    Roll(String),
    Confirm,
}

#[derive(Debug, Clone)]
pub struct LevelUp {
    pub flow: levelup::LevelUp,
    pub stage: Stage,
}

pub enum Outcome {
    Continue,
    Cancel,
    Apply,
}

/// A picker (or multi-picker) for a pick question.
pub fn question_picker(q: &Question) -> Option<Picker> {
    let Question::Pick { title, choices, count } = q else { return None };
    Some(if *count > 1 { Picker::multi(title.clone(), choices.clone(), *count) } else { Picker::new(title.clone(), choices.clone()) })
}

/// A picker for a class feature choice (multi-select when more than one).
pub fn choice_picker(choice: &dnt_rules::choices::Choice, count: usize) -> Picker {
    question_picker(&flow::choice_question(choice, count)).expect("a pick question")
}

fn stage(q: &Question) -> Stage {
    match q {
        Question::Pick { .. } => Stage::Pick(question_picker(q).expect("a pick question")),
        Question::Number { .. } => Stage::Roll(String::new()),
        Question::Abilities(_) | Question::Confirm => Stage::Confirm,
    }
}

impl LevelUp {
    pub fn start(lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> LevelUp {
        let flow = levelup::LevelUp::start(lib, c, ev, filter);
        LevelUp { stage: stage(&flow.question), flow }
    }

    pub fn key(&mut self, key: KeyEvent, lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> Outcome {
        if key.code == KeyCode::Esc {
            return Outcome::Cancel;
        }
        let answer = match &mut self.stage {
            Stage::Confirm => {
                return match key.code {
                    KeyCode::Enter | KeyCode::Char('y') => Outcome::Apply,
                    KeyCode::Char('n') => Outcome::Cancel,
                    _ => Outcome::Continue,
                }
            }
            Stage::Roll(text) => match key.code {
                KeyCode::Char(ch) if ch.is_ascii_digit() => {
                    text.push(ch);
                    return Outcome::Continue;
                }
                KeyCode::Backspace => {
                    text.pop();
                    return Outcome::Continue;
                }
                KeyCode::Enter => match text.parse::<i64>() {
                    Ok(n) => Answer::Number(n),
                    Err(_) => return Outcome::Continue,
                },
                _ => return Outcome::Continue,
            },
            Stage::Pick(picker) => match picker.key(key) {
                Some(Picked::One(v)) => Answer::Pick(vec![v]),
                Some(Picked::Many(vs)) => Answer::Pick(vs),
                None => return Outcome::Continue,
            },
        };
        if self.flow.answer(lib, c, ev, filter, answer).is_ok() {
            self.stage = stage(&self.flow.question);
        }
        Outcome::Continue
    }

    /// Apply the finished level-up to the character.
    pub fn apply(self, c: &mut Character) -> String {
        self.flow.apply(c)
    }
}
