//! Gaining a level as questions and answers: class → subclass (when due)
//! → hit points → ability increase or feat (when due) → class feature
//! choices → confirm. Also used for a character's very first level. A
//! frontend shows the current question and sends back the answer.

use dnt_data::{EntityId, Library, SourceFilter};
use dnt_rules::character::{Character, HitPoints, HpMethod, Level, LevelChoices};
use dnt_rules::choices;
use dnt_rules::levelup::{self, LevelPerk};
use dnt_rules::overlay::resolve_id;
use dnt_rules::standard::ABILITIES;
use dnt_rules::Evaluation;

use crate::options::Choice;

/// What a flow asks next.
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// Pick `count` of the choices (by value).
    Pick { title: String, choices: Vec<Choice>, count: usize },
    /// Type a number (hit points rolled at the table).
    Number { title: String },
    /// Everything is chosen: apply or cancel.
    Confirm,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Pick(Vec<String>),
    Number(i64),
}

/// A question for a class feature choice ("Fighting Style: choose one").
pub fn choice_question(choice: &choices::Choice, count: usize) -> Question {
    let choices = choice
        .options
        .iter()
        .map(|o| Choice { label: o.label.clone(), hint: o.hint.clone(), warning: o.warning.clone(), value: o.value.clone() })
        .collect();
    let title = if count > 1 { format!("{}: choose {count}", choice.key) } else { format!("{}: choose one", choice.key) };
    Question::Pick { title, choices, count }
}

fn pick(title: impl Into<String>, choices: Vec<Choice>) -> Question {
    Question::Pick { title: title.into(), choices, count: 1 }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    Class,
    Subclass,
    Hp,
    HpRoll,
    Asi,
    AsiAbility,
    Feat,
    Choice,
    Done,
}

#[derive(Debug, Clone)]
pub struct LevelUp {
    pub question: Question,
    step: Step,
    /// The class gaining the level (its id), once chosen.
    pub class: Option<String>,
    class_entity: Option<EntityId>,
    /// The class's level after this one.
    pub new_level: i64,
    pub subclass: Option<String>,
    pub hp: HitPoints,
    pub choices: LevelChoices,
    /// Ability increases still to assign (`[2]` or `[1, 1]`).
    asi_left: Vec<i64>,
    perk: LevelPerk,
    /// What was chosen so far, one line each.
    pub summary: Vec<String>,
    /// Feature choices still to make; `None` until computed.
    feature_choices: Option<Vec<choices::Choice>>,
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
                Choice { label, hint: o.source, warning: o.warning, value: o.id }
            })
            .collect();
        let title = if c.levels.is_empty() { "First level: choose a class" } else { "Level up: choose a class" };
        LevelUp {
            question: pick(title, items),
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
            feature_choices: None,
        }
    }

    /// Answer the current question and move on.
    pub fn answer(&mut self, lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter, answer: Answer) -> Result<(), String> {
        let values = match (answer, self.step) {
            (Answer::Number(n), Step::HpRoll) => {
                self.hp = HitPoints::Rolled(n.max(1));
                self.summary.push(format!("Hit points: rolled {}", n.max(1)));
                self.after_hp(lib, c, filter);
                return Ok(());
            }
            (Answer::Pick(values), step) if !matches!(step, Step::HpRoll | Step::Done) => values,
            _ => return Err("That does not answer the question".into()),
        };
        let Question::Pick { choices, count, .. } = &self.question else { return Err("Nothing to pick".into()) };
        if values.is_empty() || values.len() > *count {
            return Err(format!("Choose {count}"));
        }
        let mut labels = Vec::new();
        for v in &values {
            let c = choices.iter().find(|c| &c.value == v).ok_or_else(|| format!("{v} is not one of the choices"))?;
            labels.push(c.label.clone());
        }
        if self.step == Step::Choice {
            let choice = self.feature_choices.as_mut().map(|q| q.remove(0)).ok_or("No choice in progress")?;
            self.summary.push(format!("{}: {}", choice.key, labels.join(", ")));
            self.choices.picks.entry(choice.key).or_default().extend(values);
            self.next_choice(lib, c, filter);
            return Ok(());
        }
        let value = values.into_iter().next().unwrap_or_default();
        let label = labels.into_iter().next().unwrap_or_default();
        self.chosen(value, label, lib, c, ev, filter);
        Ok(())
    }

    fn chosen(&mut self, value: String, label: String, lib: &Library, c: &Character, ev: &Evaluation, filter: &SourceFilter) {
        match self.step {
            Step::Class => {
                let entity = resolve_id(lib, &value);
                self.new_level = c.class_level(&value) + 1;
                self.class_entity = entity;
                self.summary.push(format!("Class: {}", label));
                self.class = Some(value.clone());
                let due = entity.and_then(|e| levelup::subclass_level(&lib.entity(e).data)).is_some_and(|l| l == self.new_level);
                if due && !c.subclasses.contains_key(&value) {
                    let items: Vec<Choice> = levelup::subclass_options(lib, entity.unwrap(), filter)
                        .into_iter()
                        .map(|s| Choice::new(lib.entity(s).name.clone(), lib.entity(s).source.clone(), levelup::subclass_id(lib, s)))
                        .collect();
                    if !items.is_empty() {
                        self.step = Step::Subclass;
                        self.question = pick("Choose a subclass", items);
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
                "roll" => {
                    self.step = Step::HpRoll;
                    self.question = Question::Number { title: "Hit points for this level".into() };
                }
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
                    self.next_choice(lib, c, filter);
                } else {
                    self.ability_stage(ev);
                }
            }
            Step::Feat => {
                self.summary.push(format!("Feat: {label}"));
                self.choices.feat = Some(value);
                self.next_choice(lib, c, filter);
            }
            Step::HpRoll | Step::Choice | Step::Done => {}
        }
    }

    /// Ask the next class feature choice, or move to confirmation.
    fn next_choice(&mut self, lib: &Library, c: &Character, filter: &SourceFilter) {
        if self.feature_choices.is_none() {
            let (Some(class), Some(class_id)) = (self.class_entity, self.class.clone()) else {
                self.done();
                return;
            };
            let subclass = self.subclass.clone().or_else(|| c.subclasses.get(&class_id).cloned()).and_then(|s| resolve_id(lib, &s));
            let skills = c.proficiencies.skills.clone();
            let found = choices::level_choices(lib, c, class, subclass, self.new_level, filter, &skills);
            self.feature_choices = Some(found.into_iter().filter(|ch| !ch.options.is_empty()).collect());
        }
        match self.feature_choices.as_ref().and_then(|q| q.first()) {
            Some(choice) => {
                self.step = Step::Choice;
                self.question = choice_question(choice, choice.count);
            }
            None => self.done(),
        }
    }

    fn done(&mut self) {
        self.step = Step::Done;
        self.question = Question::Confirm;
    }

    fn hp_stage(&mut self, lib: &Library, c: &Character, filter: &SourceFilter) {
        let faces =
            self.class_entity.and_then(|e| lib.entity(e).data.get("hd").and_then(|h| h.get("faces")).and_then(|f| f.as_i64())).unwrap_or(8);
        if c.levels.is_empty() {
            // The first character level always gets the maximum.
            self.summary.push(format!("Hit points: {faces} (maximum at 1st level)"));
            self.after_hp(lib, c, filter);
            return;
        }
        let items = vec![
            Choice::new(format!("Average  {}", faces / 2 + 1), format!("d{faces}"), "average"),
            Choice::new("Roll it at the table…", "enter the result", "roll"),
            Choice::new(format!("Maximum  {faces}"), "house rule", "max"),
        ];
        self.step = Step::Hp;
        self.question = pick("Hit points for this level", items);
    }

    fn after_hp(&mut self, lib: &Library, c: &Character, filter: &SourceFilter) {
        self.perk = self.class_entity.map(|e| levelup::perk_at(&lib.entity(e).data, self.new_level)).unwrap_or(LevelPerk::None);
        match self.perk {
            LevelPerk::None => self.next_choice(lib, c, filter),
            LevelPerk::EpicBoon => self.feat_stage(lib, filter),
            LevelPerk::Asi => {
                let items = vec![
                    Choice::new("+2 to one ability score", "", "2"),
                    Choice::new("+1 to two ability scores", "", "11"),
                    Choice::new("Take a feat instead…", "", "feat"),
                ];
                self.step = Step::Asi;
                self.question = pick("Ability Score Improvement", items);
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
                let mut item = Choice::new(format!("{name}  {score} → {}", score + n), a.to_uppercase(), a.to_string());
                if score + n > 20 {
                    item.warning = Some("above 20".into());
                }
                item
            })
            .collect();
        self.step = Step::AsiAbility;
        self.question = pick(format!("+{n} to which ability?"), items);
    }

    fn feat_stage(&mut self, lib: &Library, filter: &SourceFilter) {
        let items = levelup::feat_options(lib, self.perk, filter)
            .into_iter()
            .map(|f| {
                let e = lib.entity(f);
                let category = e.data.get("category").and_then(|c| c.as_str()).map(dnt_data::view::feat_category).unwrap_or("");
                Choice::new(e.name.clone(), format!("{category} {}", e.source).trim().to_string(), levelup::feat_id(lib, f))
            })
            .collect();
        let title = if self.perk == LevelPerk::EpicBoon { "Epic Boon: choose a feat" } else { "Choose a feat" };
        self.step = Step::Feat;
        self.question = pick(title, items);
    }

    /// Apply the finished level-up to the character.
    pub fn apply(self, c: &mut Character) -> String {
        let Some(class) = self.class else { return "Nothing chosen".into() };
        c.levels.push(Level { class: class.clone(), hp: self.hp, choices: self.choices });
        if let Some(s) = self.subclass {
            c.subclasses.insert(class, s);
        }
        format!("Now level {}", c.total_level())
    }
}

/// Collapse the column padding used in choice labels.
fn tidy(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dnt_rules::overlay::OverlaySet;
    use serde_json::json;

    fn values(q: &Question) -> Vec<String> {
        match q {
            Question::Pick { choices, .. } => choices.iter().map(|c| c.value.clone()).collect(),
            other => panic!("not a pick: {other:?}"),
        }
    }

    #[test]
    fn levels_are_gained_by_answering() {
        let mut lib = Library::new();
        lib.load_json(&json!({"class": [{"name": "Fighter", "source": "T", "hd": {"number": 1, "faces": 10}}]}));
        let (filter, overlays) = (SourceFilter::default(), OverlaySet::default());
        let mut c = Character { name: "A".into(), ..Default::default() };
        let ev = dnt_rules::builder::build(&c, &lib, &overlays).evaluate();

        // First level: the class, then maximum hit points without asking.
        let mut flow = LevelUp::start(&lib, &c, &ev, &filter);
        let fighter = values(&flow.question).into_iter().find(|v| v.contains("Fighter")).expect("Fighter is offered");
        assert!(flow.answer(&lib, &c, &ev, &filter, Answer::Number(3)).is_err(), "a number does not pick a class");
        assert!(flow.answer(&lib, &c, &ev, &filter, Answer::Pick(vec!["class:Nope|T".into()])).is_err());
        flow.answer(&lib, &c, &ev, &filter, Answer::Pick(vec![fighter.clone()])).unwrap();
        assert_eq!(flow.question, Question::Confirm);
        assert_eq!(flow.summary, ["Class: Fighter", "Hit points: 10 (maximum at 1st level)"]);
        assert_eq!(flow.apply(&mut c), "Now level 1");

        // Second level: hit points rolled at the table.
        let ev = dnt_rules::builder::build(&c, &lib, &overlays).evaluate();
        let mut flow = LevelUp::start(&lib, &c, &ev, &filter);
        flow.answer(&lib, &c, &ev, &filter, Answer::Pick(vec![fighter])).unwrap();
        assert_eq!(values(&flow.question), ["average", "roll", "max"]);
        flow.answer(&lib, &c, &ev, &filter, Answer::Pick(vec!["roll".into()])).unwrap();
        assert!(matches!(flow.question, Question::Number { .. }));
        flow.answer(&lib, &c, &ev, &filter, Answer::Number(0)).unwrap();
        assert_eq!(flow.hp, HitPoints::Rolled(1), "at least 1");
        assert_eq!(flow.question, Question::Confirm);
        assert_eq!(flow.apply(&mut c), "Now level 2");
    }
}
