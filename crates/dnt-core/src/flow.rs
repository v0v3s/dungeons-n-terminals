//! Step-by-step flows (gaining a level, creating a character) ask one
//! question at a time; a frontend shows it and sends back the answer.

use serde::{Deserialize, Serialize};

use crate::options::Choice;

/// What a flow asks next.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    /// Pick `count` of the choices (by value).
    Pick { title: String, choices: Vec<Choice>, count: usize },
    /// Type a number (hit points rolled at the table).
    Number { title: String },
    /// Set the six ability scores, starting from these.
    Abilities { scores: AbilityScores },
    /// Everything is chosen: apply or cancel.
    Confirm,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Answer {
    Pick(Vec<String>),
    Number(i64),
    /// Strength to Charisma.
    Scores([i64; 6]),
}

pub(crate) fn pick(title: impl Into<String>, choices: Vec<Choice>) -> Question {
    Question::Pick { title: title.into(), choices, count: 1 }
}

/// A question for a class feature choice ("Fighting Style: choose one").
pub fn choice_question(choice: &dnt_rules::choices::Choice, count: usize) -> Question {
    let choices = choice
        .options
        .iter()
        .map(|o| Choice { label: o.label.clone(), hint: o.hint.clone(), warning: o.warning.clone(), value: o.value.clone() })
        .collect();
    let title = if count > 1 { format!("{}: choose {count}", choice.key) } else { format!("{}: choose one", choice.key) };
    Question::Pick { title, choices, count }
}

/// How ability scores are set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    StandardArray,
    PointBuy,
    Manual,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::StandardArray => "Standard array",
            Method::PointBuy => "Point buy",
            Method::Manual => "Manual entry",
        }
    }
}

pub const STANDARD_ARRAY: [i64; 6] = [15, 14, 13, 12, 10, 8];
pub const POINT_BUY_BUDGET: i64 = 27;

pub fn point_cost(score: i64) -> i64 {
    match score {
        ..=8 => 0,
        9 => 1,
        10 => 2,
        11 => 3,
        12 => 4,
        13 => 5,
        14 => 7,
        _ => 9,
    }
}

/// Six scores being set by a method, within its rules.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AbilityScores {
    pub method: Method,
    pub scores: [i64; 6],
}

impl AbilityScores {
    pub fn new(method: Method) -> AbilityScores {
        let scores = match method {
            Method::StandardArray => STANDARD_ARRAY,
            Method::PointBuy => [8; 6],
            Method::Manual => [10; 6],
        };
        AbilityScores { method, scores }
    }

    pub fn points_left(&self) -> i64 {
        POINT_BUY_BUDGET - self.scores.iter().map(|s| point_cost(*s)).sum::<i64>()
    }

    /// Raise (`up`) or lower score `i` within the method's rules.
    pub fn adjust(&mut self, i: usize, up: bool) {
        match self.method {
            Method::StandardArray => {
                // Swap with whichever ability holds the next value in the array.
                let mut values = STANDARD_ARRAY.to_vec();
                values.sort();
                let pos = values.iter().position(|v| *v == self.scores[i]).unwrap_or(0);
                let target = if up { values.get(pos + 1) } else { pos.checked_sub(1).and_then(|p| values.get(p)) };
                if let Some(&t) = target {
                    if let Some(j) = self.scores.iter().position(|v| *v == t) {
                        self.scores.swap(i, j);
                    }
                }
            }
            Method::PointBuy => {
                let new = self.scores[i] + if up { 1 } else { -1 };
                if (8..=15).contains(&new) {
                    let old = self.scores[i];
                    self.scores[i] = new;
                    if self.points_left() < 0 {
                        self.scores[i] = old;
                    }
                }
            }
            Method::Manual => self.scores[i] = (self.scores[i] + if up { 1 } else { -1 }).clamp(1, 30),
        }
    }

    /// Whether `scores` follow this method's rules.
    pub fn check(&self, scores: &[i64; 6]) -> Result<(), String> {
        match self.method {
            Method::StandardArray => {
                let (mut a, mut b) = (*scores, STANDARD_ARRAY);
                a.sort();
                b.sort();
                if a != b {
                    return Err("The standard array is 15, 14, 13, 12, 10 and 8, each used once".into());
                }
            }
            Method::PointBuy => {
                if scores.iter().any(|s| !(8..=15).contains(s)) {
                    return Err("Point buy scores are 8 to 15".into());
                }
                let spent: i64 = scores.iter().map(|s| point_cost(*s)).sum();
                if spent > POINT_BUY_BUDGET {
                    return Err(format!("{spent} points spent of {POINT_BUY_BUDGET}"));
                }
            }
            Method::Manual => {
                if scores.iter().any(|s| !(1..=30).contains(s)) {
                    return Err("Scores are 1 to 30".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_buy_respects_budget() {
        let mut ed = AbilityScores::new(Method::PointBuy);
        for _ in 0..7 {
            ed.adjust(0, true);
        }
        assert_eq!(ed.scores[0], 15, "capped at 15");
        assert_eq!(ed.points_left(), 18);
        for i in [1, 2] {
            for _ in 0..7 {
                ed.adjust(i, true);
            }
        }
        assert_eq!(ed.points_left(), 0);
        assert_eq!(ed.scores[2], 15);
        ed.adjust(3, true);
        assert_eq!(ed.scores[3], 8, "no points left");
        assert!(ed.check(&ed.scores).is_ok());
        assert!(ed.check(&[15, 15, 15, 9, 8, 8]).is_err(), "over budget");
    }

    #[test]
    fn standard_array_swaps_values() {
        let mut ed = AbilityScores::new(Method::StandardArray);
        ed.adjust(5, true); // Charisma 8
        assert_eq!(ed.scores, [15, 14, 13, 12, 8, 10]);
        assert!(ed.check(&ed.scores).is_ok(), "always a permutation");
        assert!(ed.check(&[15, 15, 13, 12, 10, 8]).is_err());
    }
}
