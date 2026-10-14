//! The level-up flow: class → subclass (when due) → hit points → ASI/feat
//! (when due) → confirm. Also used for a character's very first level.

use dnt_data::{Library, SourceFilter};
use dnt_rules::character::{Character, HitPoints, HpMethod, Level, LevelChoices};
use dnt_rules::levelup::{self, LevelPerk};
use dnt_rules::standard::ABILITIES;
use dnt_rules::Evaluation;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::picker::{PickItem, Picked, Picker};

#[derive(Debug, Clone)]
pub enum Stage {
    Pick(Picker),
    /// Typing a rolled hit point value.
    Roll(String),
    Confirm,
}

#[derive(Debug, Clone)]
pub struct LevelUp {
    pub stage: Stage,
    /// What is being chosen in a `Pick` stage.
    step: Step,
    pub class: Option<String>,
    class_entity: Option<dnt_data::EntityId>,
    pub new_level: i64,
    pub subclass: Option<String>,
    pub hp: HitPoints,
    pub choices: LevelChoices,
    /// Ability increases still to assign (`[2]` or `[1, 1]`).
    asi_left: Vec<i64>,
    perk: LevelPerk,
    pub summary: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Class,
    Subclass,
    Hp,
    Asi,
    AsiAbility,
    Feat,
}

pub enum Outcome {
    Continue,
    Cancel,
    Apply,
}

impl LevelUp {
    pub fn start(lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> LevelUp {
        let items = levelup::class_options(lib, c, ev, filter)
            .into_iter()
            .map(|o| {
                let label = if o.level > 0 {
                    format!("{} {} → {}", o.name, o.level, o.level + 1)
                } else if c.levels.is_empty() {
                    o.name.clone()
                } else {
                    format!("New class: {}", o.name)
                };
                PickItem {
                    label,
                    hint: o.source,
                    warning: o.warning,
                    value: o.id,
                }
            })
            .collect();
        let title = if c.levels.is_empty() { "First level: choose a class" } else { "Level up: choose a class" };
        LevelUp {
            stage: Stage::Pick(Picker::new(title, items)),
            step: Step::Class,
            class: None,
            class_entity: None,
            new_level: 0,
            subclass: None,
            hp: HitPoints::Average,
            choices: LevelChoices::default(),
            asi_left: Vec::new(),
            perk: LevelPerk::None,
            summary: Vec::new(),
        }
    }

    pub fn key(&mut self, key: KeyEvent, lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) -> Outcome {
        if key.code == KeyCode::Esc {
            return Outcome::Cancel;
        }
        match &mut self.stage {
            Stage::Confirm => match key.code {
                KeyCode::Enter | KeyCode::Char('y') => Outcome::Apply,
                KeyCode::Char('n') => Outcome::Cancel,
                _ => Outcome::Continue,
            },
            Stage::Roll(text) => {
                match key.code {
                    KeyCode::Char(ch) if ch.is_ascii_digit() => text.push(ch),
                    KeyCode::Backspace => {
                        text.pop();
                    }
                    KeyCode::Enter => {
                        if let Ok(n) = text.parse::<i64>() {
                            self.hp = HitPoints::Rolled(n.max(1));
                            self.summary.push(format!("Hit points: rolled {}", n.max(1)));
                            self.after_hp(lib, c, filter);
                        }
                    }
                    _ => {}
                }
                Outcome::Continue
            }
            Stage::Pick(picker) => {
                let Some(Picked::One(value)) = picker.key(key) else { return Outcome::Continue };
                let label = picker.current().map(|i| i.label.clone()).unwrap_or_default();
                self.chosen(value, label, lib, c, ev, filter);
                Outcome::Continue
            }
        }
    }

    fn chosen(&mut self, value: String, label: String, lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) {
        match self.step {
            Step::Class => {
                let entity = dnt_rules::overlay::resolve_id(lib, &value);
                self.new_level = c.class_level(&value) + 1;
                self.class_entity = entity;
                self.summary.push(format!("Class: {}", label));
                self.class = Some(value.clone());
                let due = entity
                    .and_then(|e| levelup::subclass_level(&lib.entity(e).data))
                    .is_some_and(|l| l == self.new_level);
                if due && !c.subclasses.contains_key(&value) {
                    let items: Vec<PickItem> = levelup::subclass_options(lib, entity.unwrap(), filter)
                        .into_iter()
                        .map(|s| PickItem::new(lib.entity(s).name.clone(), lib.entity(s).source.clone(), levelup::subclass_id(lib, s)))
                        .collect();
                    if !items.is_empty() {
                        self.step = Step::Subclass;
                        self.stage = Stage::Pick(Picker::new("Choose a subclass", items));
                        return;
                    }
                }
                self.hp_stage(lib, c, filter);
            }
            Step::Subclass => {
                self.summary.push(format!("Subclass: {label}"));
                self.subclass = Some(value);
                self.hp_stage(lib, c, filter);
            }
            Step::Hp => match value.as_str() {
                "roll" => self.stage = Stage::Roll(String::new()),
                "max" => {
                    self.hp = HitPoints::Method(HpMethod::Max);
                    self.summary.push(format!("Hit points: {}", tidy(&label)));
                    self.after_hp(lib, c, filter);
                }
                _ => {
                    self.hp = HitPoints::Average;
                    self.summary.push(format!("Hit points: {}", tidy(&label)));
                    self.after_hp(lib, c, filter);
                }
            },
            Step::Asi => match value.as_str() {
                "feat" => self.feat_stage(lib, filter),
                mode => {
                    self.asi_left = if mode == "2" { vec![2] } else { vec![1, 1] };
                    self.ability_stage(ev);
                }
            },
            Step::AsiAbility => {
                let n = self.asi_left.remove(0);
                *self.choices.ability.entry(value.clone()).or_default() += n;
                self.summary.push(format!("+{n} {}", tidy(&label)));
                if self.asi_left.is_empty() {
                    self.stage = Stage::Confirm;
                } else {
                    self.ability_stage(ev);
                }
            }
            Step::Feat => {
                self.summary.push(format!("Feat: {label}"));
                self.choices.feat = Some(value);
                self.stage = Stage::Confirm;
            }
        }
    }

    fn hp_stage(&mut self, lib: &Library, c: &Character, filter: &SourceFilter) {
        let faces = self
            .class_entity
            .and_then(|e| lib.entity(e).data.get("hd").and_then(|h| h.get("faces")).and_then(|f| f.as_i64()))
            .unwrap_or(8);
        if c.levels.is_empty() {
            // The first character level always gets the maximum.
            self.summary.push(format!("Hit points: {faces} (maximum at 1st level)"));
            self.after_hp(lib, c, filter);
            return;
        }
        let items = vec![
            PickItem::new(format!("Average  {}", faces / 2 + 1), format!("d{faces}"), "average"),
            PickItem::new("Roll it at the table…", "enter the result", "roll"),
            PickItem::new(format!("Maximum  {faces}"), "house rule", "max"),
        ];
        self.step = Step::Hp;
        self.stage = Stage::Pick(Picker::new("Hit points for this level", items));
    }

    fn after_hp(&mut self, lib: &Library, _c: &Character, filter: &SourceFilter) {
        self.perk = self
            .class_entity
            .map(|e| levelup::perk_at(&lib.entity(e).data, self.new_level))
            .unwrap_or(LevelPerk::None);
        match self.perk {
            LevelPerk::None => self.stage = Stage::Confirm,
            LevelPerk::EpicBoon => self.feat_stage(lib, filter),
            LevelPerk::Asi => {
                let items = vec![
                    PickItem::new("+2 to one ability score", "", "2"),
                    PickItem::new("+1 to two ability scores", "", "11"),
                    PickItem::new("Take a feat instead…", "", "feat"),
                ];
                self.step = Step::Asi;
                self.stage = Stage::Pick(Picker::new("Ability Score Improvement", items));
            }
        }
    }

    fn ability_stage(&mut self, ev: &Evaluation) {
        let n = self.asi_left[0];
        let items = ABILITIES
            .iter()
            .filter(|(a, _)| !(self.asi_left.len() == 1 && n == 1 && self.choices.ability.contains_key(*a)))
            .map(|(a, name)| {
                let score = ev.int(a).unwrap_or(10) + self.choices.ability.get(*a).copied().unwrap_or(0);
                let mut item = PickItem::new(format!("{name}  {score} → {}", score + n), a.to_uppercase(), a.to_string());
                if score + n > 20 {
                    item.warning = Some("above 20".into());
                }
                item
            })
            .collect();
        self.step = Step::AsiAbility;
        self.stage = Stage::Pick(Picker::new(format!("+{n} to which ability?"), items));
    }

    fn feat_stage(&mut self, lib: &Library, filter: &SourceFilter) {
        let items = levelup::feat_options(lib, self.perk, filter)
            .into_iter()
            .map(|f| {
                let e = lib.entity(f);
                let category = e.data.get("category").and_then(|c| c.as_str()).map(dnt_data::view::feat_category).unwrap_or("");
                PickItem::new(e.name.clone(), format!("{category} {}", e.source).trim().to_string(), levelup::feat_id(lib, f))
            })
            .collect();
        let title = if self.perk == LevelPerk::EpicBoon { "Epic Boon: choose a feat" } else { "Choose a feat" };
        self.step = Step::Feat;
        self.stage = Stage::Pick(Picker::new(title, items));
    }

    /// Apply the finished level-up to the character.
    pub fn apply(self, c: &mut Character) -> String {
        let Some(class) = self.class else { return "Nothing chosen".into() };
        c.levels.push(Level {
            class: class.clone(),
            hp: self.hp,
            choices: self.choices,
        });
        if let Some(s) = self.subclass {
            c.subclasses.insert(class, s);
        }
        format!("Now level {}", c.total_level())
    }
}

/// Collapse the column padding used in picker labels.
fn tidy(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}
