//! Character creation wizard: species → subspecies → background → ability
//! scores → ability increases → feats → class → subclass → skills → armor →
//! review. Steps with nothing to choose are skipped; Esc goes back a step.

use std::collections::BTreeMap;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use dnt_rules::character::{AbilityBonus, Character, Level};
use dnt_rules::levelup;
use dnt_rules::origin::{self, AbilityGrant};
use dnt_rules::overlay::resolve_id;
use dnt_rules::standard::{ABILITIES, SKILLS};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::Value as Json;

use dnt_rules::choices::{self, Choice};

use crate::levelup::choice_picker;
use crate::picker::{PickItem, Picked, Picker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct AbilityEditor {
    pub method: Method,
    pub scores: [i64; 6],
    pub cursor: usize,
}

impl AbilityEditor {
    fn new(method: Method) -> AbilityEditor {
        let scores = match method {
            Method::StandardArray => STANDARD_ARRAY,
            Method::PointBuy => [8; 6],
            Method::Manual => [10; 6],
        };
        AbilityEditor { method, scores, cursor: 0 }
    }

    pub fn points_left(&self) -> i64 {
        POINT_BUY_BUDGET - self.scores.iter().map(|s| point_cost(*s)).sum::<i64>()
    }

    /// Raise (`up`) or lower the score under the cursor within the method's rules.
    fn adjust(&mut self, up: bool) {
        let i = self.cursor;
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
}

/// One pending ability-increase choice.
#[derive(Debug, Clone)]
struct BonusTask {
    origin: String,
    /// Remaining amounts to place, each on a different ability.
    amounts: Vec<i64>,
    from: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Species,
    Subspecies,
    Background,
    Method,
    Abilities,
    BonusAlternative,
    Bonus,
    Feat,
    Class,
    Subclass,
    ClassSkills,
    OriginSkills,
    FeatureChoice,
    Armor,
    Shield,
    Review,
}

const ORDER: [Step; 16] = [
    Step::Species,
    Step::Subspecies,
    Step::Background,
    Step::Method,
    Step::Abilities,
    Step::BonusAlternative,
    Step::Bonus,
    Step::Feat,
    Step::Class,
    Step::Subclass,
    Step::ClassSkills,
    Step::OriginSkills,
    Step::FeatureChoice,
    Step::Armor,
    Step::Shield,
    Step::Review,
];

#[derive(Debug, Clone)]
pub enum View {
    Pick(Picker),
    Abilities(AbilityEditor),
    Review,
}

/// Everything that changes as the wizard advances (snapshotted for Esc).
#[derive(Debug, Clone)]
struct State {
    step: Step,
    view: View,
    ch: Character,
    summary: Vec<(String, String)>,
    species: Option<EntityId>,
    subspecies: Option<EntityId>,
    background: Option<EntityId>,
    class: Option<EntityId>,
    /// Ability-increase alternatives still to resolve, per origin.
    alternatives: Vec<(String, Vec<Vec<AbilityGrant>>)>,
    bonus_tasks: Vec<BonusTask>,
    bonuses: BTreeMap<String, BTreeMap<String, i64>>,
    feat_choice: Option<Vec<String>>,
    origin_skill_choices: Vec<(String, origin::SkillGrant)>,
    /// Class feature choices for the first level; `None` until computed.
    feature_choices: Option<Vec<Choice>>,
}

pub struct Wizard {
    state: State,
    history: Vec<State>,
}

pub enum Outcome {
    Continue,
    Cancel,
    Finish(Box<Character>),
}

fn name_of(lib: &Library, id: Option<EntityId>) -> String {
    id.map(|e| lib.entity(e).name.clone()).unwrap_or_default()
}

fn entity_items(lib: &Library, kind: Kind, filter: &SourceFilter, id: impl Fn(EntityId) -> String) -> Vec<PickItem> {
    let mut ids: Vec<EntityId> = (0..lib.entities().len())
        .filter(|&i| lib.entity(i).kind == kind && filter.allows(lib.entity(i)))
        .collect();
    ids.sort_by(|&a, &b| lib.entity(a).name.cmp(&lib.entity(b).name));
    ids.into_iter()
        .map(|i| PickItem::new(lib.entity(i).name.clone(), lib.entity(i).source.clone(), id(i)))
        .collect()
}

fn ability_name(a: &str) -> &'static str {
    ABILITIES.iter().find(|(k, _)| *k == a).map_or("?", |(_, n)| n)
}

fn skill_name(k: &str) -> &'static str {
    SKILLS.iter().find(|(s, _, _)| *s == k).map_or("?", |(_, n, _)| n)
}

fn describe_alternative(parts: &[AbilityGrant]) -> String {
    parts
        .iter()
        .map(|p| match p {
            AbilityGrant::Fixed(m) => m.iter().map(|(a, n)| format!("{} {n:+}", ability_name(a))).collect::<Vec<_>>().join(", "),
            AbilityGrant::Choose { from, count, amount } => format!(
                "{amount:+} to {count} of {}",
                from.iter().map(|a| a.to_uppercase()).collect::<Vec<_>>().join("/")
            ),
            AbilityGrant::Weighted { from, weights } => format!(
                "{} among {}",
                weights.iter().map(|w| format!("{w:+}")).collect::<Vec<_>>().join(" / "),
                from.iter().map(|a| ability_name(a)).collect::<Vec<_>>().join(", ")
            ),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl Wizard {
    pub fn new(name: String, lib: &Library, filter: &SourceFilter) -> Wizard {
        let state = State {
            step: Step::Species,
            view: View::Review,
            ch: Character {
                name,
                ..Character::default()
            },
            summary: Vec::new(),
            species: None,
            subspecies: None,
            background: None,
            class: None,
            alternatives: Vec::new(),
            bonus_tasks: Vec::new(),
            bonuses: BTreeMap::new(),
            feat_choice: None,
            origin_skill_choices: Vec::new(),
            feature_choices: None,
        };
        let mut w = Wizard { state, history: Vec::new() };
        w.enter(Step::Species, lib, filter);
        w
    }

    pub fn name(&self) -> &str {
        &self.state.ch.name
    }

    pub fn view(&self) -> &View {
        &self.state.view
    }

    pub fn summary(&self) -> &[(String, String)] {
        &self.state.summary
    }

    /// Step titles for the sidebar, with whether each is done/current.
    pub fn progress(&self) -> Vec<(&'static str, bool, bool)> {
        let cur = ORDER.iter().position(|s| *s == self.state.step).unwrap_or(0);
        [
            ("Species", Step::Species),
            ("Background", Step::Background),
            ("Ability scores", Step::Method),
            ("Ability increases", Step::BonusAlternative),
            ("Feats", Step::Feat),
            ("Class", Step::Class),
            ("Skills & features", Step::ClassSkills),
            ("Equipment", Step::Armor),
            ("Review", Step::Review),
        ]
        .into_iter()
        .map(|(label, step)| {
            let at = ORDER.iter().position(|s| *s == step).unwrap();
            let next = ORDER
                .iter()
                .skip(at + 1)
                .position(|s| matches!(s, Step::Background | Step::Method | Step::BonusAlternative | Step::Feat | Step::Class | Step::ClassSkills | Step::Armor | Step::Review))
                .map_or(ORDER.len(), |p| at + 1 + p);
            (label, cur >= next, (at..next).contains(&cur))
        })
        .collect()
    }

    /// Entity to preview for the highlighted choice, if any.
    pub fn preview(&self, lib: &Library) -> Option<EntityId> {
        match &self.state.view {
            View::Pick(p) => p
                .current()
                .and_then(|i| resolve_id(lib, i.value.strip_prefix("version:").unwrap_or(&i.value))),
            _ => None,
        }
    }

    pub fn key(&mut self, key: KeyEvent, lib: &Library, filter: &SourceFilter) -> Outcome {
        if key.code == KeyCode::Esc {
            return match self.history.pop() {
                Some(prev) => {
                    self.state = prev;
                    Outcome::Continue
                }
                None => Outcome::Cancel,
            };
        }
        match &mut self.state.view {
            View::Review => {
                if key.code == KeyCode::Enter {
                    return Outcome::Finish(Box::new(self.finish()));
                }
            }
            View::Abilities(ed) => match key.code {
                KeyCode::Char('j') | KeyCode::Down => ed.cursor = (ed.cursor + 1).min(5),
                KeyCode::Char('k') | KeyCode::Up => ed.cursor = ed.cursor.saturating_sub(1),
                KeyCode::Char('l' | '+' | '=') | KeyCode::Right => ed.adjust(true),
                KeyCode::Char('h' | '-') | KeyCode::Left => ed.adjust(false),
                KeyCode::Enter => {
                    let ed = ed.clone();
                    self.snapshot();
                    let st = &mut self.state;
                    for (i, (a, _)) in ABILITIES.iter().enumerate() {
                        set_score(&mut st.ch, a, ed.scores[i]);
                    }
                    let scores = ABILITIES
                        .iter()
                        .enumerate()
                        .map(|(i, (a, _))| format!("{} {}", a.to_uppercase(), ed.scores[i]))
                        .collect::<Vec<_>>()
                        .join(" ");
                    st.summary.push(("Abilities".into(), format!("{scores} ({})", ed.method.label())));
                    self.advance(Step::Abilities, lib, filter);
                }
                _ => {}
            },
            View::Pick(p) => {
                let Some(picked) = p.key(key) else { return Outcome::Continue };
                let label = p.current().map(|i| i.label.clone()).unwrap_or_default();
                self.snapshot();
                self.picked(picked, label, lib, filter);
            }
        }
        Outcome::Continue
    }

    fn snapshot(&mut self) {
        self.history.push(self.state.clone());
    }

    /// Move to the first step after `from` that has something to choose.
    fn advance(&mut self, from: Step, lib: &Library, filter: &SourceFilter) {
        let at = ORDER.iter().position(|s| *s == from).unwrap();
        for &step in &ORDER[at + 1..] {
            if self.enter(step, lib, filter) {
                return;
            }
        }
    }

    /// Set up `step`; returns false when there is nothing to choose.
    fn enter(&mut self, step: Step, lib: &Library, filter: &SourceFilter) -> bool {
        let st = &mut self.state;
        let view = match step {
            Step::Species => {
                let mut items = entity_items(lib, Kind::Race, filter, |i| format!("race:{}|{}", lib.entity(i).name, lib.entity(i).source));
                // Lineages/ancestries are chosen in the next step.
                items.retain(|it| resolve_id(lib, &it.value).is_none_or(|e| lib.version_base(e).is_none()));
                View::Pick(Picker::new("Choose a species", items))
            }
            Step::Subspecies => {
                let Some(sp) = st.species else { return false };
                let race = lib.entity(sp);
                let mut items: Vec<PickItem> = (0..lib.entities().len())
                    .filter(|&i| {
                        let e = lib.entity(i);
                        e.kind == Kind::Subrace
                            && filter.allows(e)
                            && e.data.get("raceName").and_then(Json::as_str) == Some(&race.name)
                            && e.data.get("raceSource").and_then(Json::as_str) == Some(&race.source)
                    })
                    .map(|i| PickItem::new(lib.entity(i).name.clone(), lib.entity(i).source.clone(), format!("race:{}|{}", lib.entity(i).name, lib.entity(i).source)))
                    .collect();
                // Versions: 2024 lineages, legacies and ancestries.
                items.extend(lib.versions_of(sp).iter().map(|&v| {
                    let e = lib.entity(v);
                    PickItem::new(e.name.clone(), e.source.clone(), format!("version:race:{}|{}", e.name, e.source))
                }));
                if items.is_empty() {
                    return false;
                }
                items.sort_by(|a, b| a.label.cmp(&b.label));
                let what = if lib.versions_of(sp).is_empty() { "subrace" } else { "lineage" };
                View::Pick(Picker::new(format!("{}: choose a {what}", race.name), items))
            }
            Step::Background => View::Pick(Picker::new(
                "Choose a background",
                entity_items(lib, Kind::Background, filter, |i| format!("background:{}|{}", lib.entity(i).name, lib.entity(i).source)),
            )),
            Step::Method => View::Pick(Picker::new(
                "How do you want to set ability scores?",
                vec![
                    PickItem::new("Standard array", "15 14 13 12 10 8", "array"),
                    PickItem::new("Point buy", "27 points, 8–15", "buy"),
                    PickItem::new("Manual entry", "rolled at the table, or anything else", "manual"),
                ],
            )),
            Step::Abilities => return true, // set up by the method choice
            Step::BonusAlternative => {
                // Gather ability grants from species/subspecies/background.
                if st.alternatives.is_empty() && st.bonus_tasks.is_empty() {
                    for (prefix, id) in [("Species", st.species), ("Species", st.subspecies), ("Background", st.background)] {
                        let Some(id) = id else { continue };
                        let grants = origin::ability_grants(lib.entity(id).data.get("ability"));
                        if !grants.is_empty() {
                            st.alternatives.push((format!("{prefix} ({})", lib.entity(id).name), grants));
                        }
                    }
                }
                let Some((origin, alts)) = st.alternatives.first().cloned() else { return false };
                if alts.len() == 1 {
                    st.alternatives.remove(0);
                    queue_alternative(st, &origin, &alts[0]);
                    return self.enter(Step::Bonus, lib, filter);
                }
                let items = alts
                    .iter()
                    .enumerate()
                    .map(|(i, a)| PickItem::new(describe_alternative(a), "", i.to_string()))
                    .collect();
                View::Pick(Picker::new(format!("{origin}: ability increases"), items))
            }
            Step::Bonus => {
                let Some(task) = st.bonus_tasks.first() else {
                    // More origins with alternatives may remain.
                    if !st.alternatives.is_empty() {
                        return self.enter(Step::BonusAlternative, lib, filter);
                    }
                    return false;
                };
                let amount = task.amounts[0];
                let taken = st.bonuses.get(&task.origin).cloned().unwrap_or_default();
                let items = task
                    .from
                    .iter()
                    .filter(|a| !taken.contains_key(*a))
                    .map(|a| {
                        let score = st.ch.abilities.get(a) + st.bonuses.values().filter_map(|m| m.get(a)).sum::<i64>();
                        PickItem::new(format!("{}  {score} → {}", ability_name(a), score + amount), a.to_uppercase(), a.clone())
                    })
                    .collect();
                View::Pick(Picker::new(format!("{}: {amount:+} to which ability?", task.origin), items))
            }
            Step::Feat => {
                // Fixed origin feats are added automatically; a category choice is asked.
                let mut categories = None;
                for id in [st.species, st.background].into_iter().flatten() {
                    let grant = origin::feat_grant(lib.entity(id).data.get("feats"));
                    for f in grant.fixed {
                        // Store the canonical id (`feat:Magic Initiate|XPHB`).
                        let Some(e) = resolve_id(lib, &f) else { continue };
                        // `feat:magic initiate; cleric|xphb` names a version of the feat.
                        let id = levelup::feat_id(lib, e);
                        if !st.ch.feats.contains(&id) {
                            st.summary.push(("Feat".into(), name_of(lib, Some(e))));
                            st.ch.feats.push(id);
                        }
                    }
                    if grant.choose.is_some() {
                        categories = grant.choose.map(|(c, _)| c);
                    }
                }
                let Some(categories) = categories else { return false };
                st.feat_choice = Some(categories.clone());
                let items = (0..lib.entities().len())
                    .filter(|&i| {
                        let e = lib.entity(i);
                        e.kind == Kind::Feat
                            && filter.allows(e)
                            && e.data.get("category").and_then(Json::as_str).is_some_and(|c| categories.iter().any(|k| k == c))
                            && !st.ch.feats.contains(&levelup::feat_id(lib, i))
                    })
                    .map(|i| PickItem::new(lib.entity(i).name.clone(), lib.entity(i).source.clone(), levelup::feat_id(lib, i)))
                    .collect();
                View::Pick(Picker::new("Choose a feat", items))
            }
            Step::Class => View::Pick(Picker::new(
                "Choose your starting class",
                entity_items(lib, Kind::Class, filter, |i| levelup::class_id(lib, i)),
            )),
            Step::Subclass => {
                let Some(class) = st.class else { return false };
                if levelup::subclass_level(&lib.entity(class).data) != Some(1) {
                    return false;
                }
                let items: Vec<PickItem> = levelup::subclass_options(lib, class, filter)
                    .into_iter()
                    .map(|s| PickItem::new(lib.entity(s).name.clone(), lib.entity(s).source.clone(), levelup::subclass_id(lib, s)))
                    .collect();
                if items.is_empty() {
                    return false;
                }
                View::Pick(Picker::new("Choose a subclass", items))
            }
            Step::ClassSkills => {
                // Fixed skills from species and background first.
                if st.origin_skill_choices.is_empty() {
                    for id in [st.species, st.subspecies, st.background].into_iter().flatten() {
                        let g = origin::skill_grant(lib.entity(id).data.get("skillProficiencies"));
                        for s in &g.fixed {
                            if !st.ch.proficiencies.skills.contains(s) {
                                st.ch.proficiencies.skills.push(s.clone());
                            }
                        }
                        if g.count > 0 {
                            st.origin_skill_choices.push((lib.entity(id).name.clone(), g));
                        }
                    }
                }
                let Some(class) = st.class else { return false };
                let g = origin::skill_grant(lib.entity(class).data.get("startingProficiencies").and_then(|p| p.get("skills")));
                if g.count == 0 {
                    return false;
                }
                skill_picker(&st.ch, &format!("{}: choose {} skill(s)", lib.entity(class).name, g.count), &g)
            }
            Step::OriginSkills => {
                let Some((from, g)) = st.origin_skill_choices.first().cloned() else { return false };
                skill_picker(&st.ch, &format!("{from}: choose {} skill(s)", g.count), &g)
            }
            Step::FeatureChoice => {
                if st.feature_choices.is_none() {
                    let Some(class) = st.class else { return false };
                    let subclass = st.ch.subclasses.values().next().and_then(|s| resolve_id(lib, s));
                    let mut before = st.ch.clone();
                    before.levels.clear();
                    let skills = st.ch.proficiencies.skills.clone();
                    let found = choices::level_choices(lib, &before, class, subclass, 1, filter, &skills);
                    st.feature_choices = Some(found.into_iter().filter(|c| !c.options.is_empty()).collect());
                }
                let Some(choice) = st.feature_choices.as_ref().and_then(|q| q.first()) else { return false };
                View::Pick(choice_picker(choice, choice.count))
            }
            Step::Armor => {
                let mut items = vec![PickItem::new("No armor", "", "none")];
                let mut armors: Vec<EntityId> = (0..lib.entities().len())
                    .filter(|&i| {
                        let e = lib.entity(i);
                        e.kind == Kind::BaseItem
                            && filter.allows(e)
                            && e.data
                                .get("type")
                                .and_then(Json::as_str)
                                .is_some_and(|t| matches!(t.split('|').next(), Some("LA" | "MA" | "HA")))
                    })
                    .collect();
                armors.sort_by_key(|&i| (lib.entity(i).data.get("ac").and_then(Json::as_i64).unwrap_or(0), lib.entity(i).name.clone()));
                items.extend(armors.into_iter().map(|i| {
                    let e = lib.entity(i);
                    let ac = e.data.get("ac").and_then(Json::as_i64).unwrap_or(0);
                    let cat = match e.data.get("type").and_then(Json::as_str).and_then(|t| t.split('|').next()) {
                        Some("LA") => "light",
                        Some("MA") => "medium",
                        _ => "heavy",
                    };
                    PickItem::new(e.name.clone(), format!("AC {ac} · {cat} · {}", e.source), format!("item:{}|{}", e.name, e.source))
                }));
                View::Pick(Picker::new("Armor (from your starting equipment)", items))
            }
            Step::Shield => View::Pick(Picker::new(
                "Carry a shield?",
                vec![PickItem::new("No shield", "", "no"), PickItem::new("Shield", "+2 AC", "yes")],
            )),
            Step::Review => View::Review,
        };
        st.step = step;
        st.view = view;
        true
    }

    fn picked(&mut self, picked: Picked, label: String, lib: &Library, filter: &SourceFilter) {
        let step = self.state.step;
        let st = &mut self.state;
        let one = match &picked {
            Picked::One(v) => v.clone(),
            Picked::Many(_) => String::new(),
        };
        match step {
            Step::Species => {
                st.species = resolve_id(lib, &one);
                st.ch.species = Some(one);
                st.summary.push(("Species".into(), label));
            }
            Step::Subspecies => {
                if let Some(id) = one.strip_prefix("version:") {
                    // A version is a complete species: it replaces the base.
                    st.species = resolve_id(lib, id);
                    st.ch.species = Some(id.to_string());
                    st.summary.push(("Lineage".into(), label));
                } else {
                    st.subspecies = resolve_id(lib, &one);
                    st.ch.subspecies = Some(one);
                    st.summary.push(("Subrace".into(), label));
                }
            }
            Step::Background => {
                st.background = resolve_id(lib, &one);
                st.ch.background = Some(one);
                st.summary.push(("Background".into(), label));
            }
            Step::Method => {
                let method = match one.as_str() {
                    "array" => Method::StandardArray,
                    "buy" => Method::PointBuy,
                    _ => Method::Manual,
                };
                st.step = Step::Abilities;
                st.view = View::Abilities(AbilityEditor::new(method));
                return;
            }
            Step::BonusAlternative => {
                let (origin, alts) = st.alternatives.remove(0);
                let i: usize = one.parse().unwrap_or(0);
                queue_alternative(st, &origin, &alts[i]);
                if self.enter(Step::Bonus, lib, filter) {
                    return;
                }
            }
            Step::Bonus => {
                let task = &mut st.bonus_tasks[0];
                let amount = task.amounts.remove(0);
                let origin = task.origin.clone();
                *st.bonuses.entry(origin.clone()).or_default().entry(one.clone()).or_default() += amount;
                if st.bonus_tasks[0].amounts.is_empty() {
                    st.bonus_tasks.remove(0);
                }
                push_merged(&mut st.summary, &origin, format!("{} {amount:+}", ability_name(&one)));
                // Stay on this step while choices remain.
                if self.enter(Step::Bonus, lib, filter) {
                    return;
                }
                self.flush_bonuses();
            }
            Step::Feat => {
                st.summary.push(("Feat".into(), label));
                st.ch.feats.push(one);
            }
            Step::Class => {
                st.class = resolve_id(lib, &one);
                st.ch.levels = vec![Level {
                    class: one,
                    hp: Default::default(),
                    choices: Default::default(),
                }];
                st.summary.push(("Class".into(), format!("{label} 1")));
            }
            Step::Subclass => {
                if let Some(class) = st.ch.levels.first().map(|l| l.class.clone()) {
                    st.ch.subclasses.insert(class, one);
                }
                st.summary.push(("Subclass".into(), label));
            }
            Step::ClassSkills | Step::OriginSkills => {
                let Picked::Many(skills) = picked else { return };
                let names = skills.iter().map(|s| skill_name(s)).collect::<Vec<_>>().join(", ");
                st.ch.proficiencies.skills.extend(skills);
                let from = if step == Step::OriginSkills {
                    st.origin_skill_choices.remove(0).0
                } else {
                    "Class".into()
                };
                st.summary.push((format!("Skills ({from})"), names));
                if step == Step::OriginSkills && self.enter(Step::OriginSkills, lib, filter) {
                    return;
                }
            }
            Step::FeatureChoice => {
                let values = match picked {
                    Picked::One(v) => vec![v],
                    Picked::Many(vs) => vs,
                };
                let choice = st.feature_choices.as_mut().map(|q| q.remove(0)).expect("choice in progress");
                let names: Vec<String> = values
                    .iter()
                    .filter_map(|v| choice.options.iter().find(|o| &o.value == v).map(|o| o.label.clone()))
                    .collect();
                st.summary.push((choice.key.clone(), names.join(", ")));
                if let Some(level) = st.ch.levels.first_mut() {
                    level.choices.picks.entry(choice.key).or_default().extend(values);
                }
                if self.enter(Step::FeatureChoice, lib, filter) {
                    return;
                }
            }
            Step::Armor => {
                st.ch.equipped.armor = (one != "none").then_some(one);
                st.summary.push(("Armor".into(), label));
            }
            Step::Shield => {
                st.ch.equipped.shield = one == "yes";
                st.summary.push(("Shield".into(), label));
            }
            Step::Abilities | Step::Review => {}
        }
        if step == Step::BonusAlternative {
            self.flush_bonuses();
            self.advance(Step::Bonus, lib, filter);
        } else {
            self.advance(step, lib, filter);
        }
    }

    /// Record chosen ability increases on the character, one entry per origin.
    fn flush_bonuses(&mut self) {
        let st = &mut self.state;
        st.ch.ability_bonus = st
            .bonuses
            .iter()
            .map(|(origin, scores)| AbilityBonus {
                origin: origin.clone(),
                scores: scores.clone(),
            })
            .collect();
    }

    /// The character as it would be created now (for the review preview).
    pub fn preview_character(&self) -> Character {
        let mut ch = self.state.ch.clone();
        if let View::Abilities(ed) = &self.state.view {
            for (i, (a, _)) in ABILITIES.iter().enumerate() {
                set_score(&mut ch, a, ed.scores[i]);
            }
        }
        ch.ability_bonus = self
            .state
            .bonuses
            .iter()
            .map(|(origin, scores)| AbilityBonus {
                origin: origin.clone(),
                scores: scores.clone(),
            })
            .collect();
        ch
    }

    fn finish(&mut self) -> Character {
        self.flush_bonuses();
        let mut ch = self.state.ch.clone();
        ch.proficiencies.skills.sort();
        ch.proficiencies.skills.dedup();
        ch
    }
}

fn queue_alternative(st: &mut State, origin: &str, parts: &[AbilityGrant]) {
    for p in parts {
        match p {
            AbilityGrant::Fixed(m) => {
                let entry = st.bonuses.entry(origin.to_string()).or_default();
                for (a, n) in m {
                    *entry.entry(a.clone()).or_default() += n;
                }
                let text = m.iter().map(|(a, n)| format!("{} {n:+}", ability_name(a))).collect::<Vec<_>>().join(", ");
                push_merged(&mut st.summary, origin, text);
            }
            AbilityGrant::Choose { from, count, amount } => st.bonus_tasks.push(BonusTask {
                origin: origin.to_string(),
                amounts: vec![*amount; *count],
                from: from.clone(),
            }),
            AbilityGrant::Weighted { from, weights } => st.bonus_tasks.push(BonusTask {
                origin: origin.to_string(),
                amounts: weights.clone(),
                from: from.clone(),
            }),
        }
    }
    // Fixed-only grants are recorded right away.
    let bonuses = st.bonuses.clone();
    st.ch.ability_bonus = bonuses
        .into_iter()
        .map(|(origin, scores)| AbilityBonus { origin, scores })
        .collect();
}

/// Append to the summary, joining with an existing line of the same key.
fn push_merged(summary: &mut Vec<(String, String)>, key: &str, text: String) {
    match summary.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => {
            v.push_str(", ");
            v.push_str(&text);
        }
        None => summary.push((key.to_string(), text)),
    }
}

fn skill_picker(ch: &Character, title: &str, g: &origin::SkillGrant) -> View {
    let items = g
        .options()
        .into_iter()
        .filter(|s| !ch.proficiencies.skills.contains(s))
        .map(|s| {
            let ab = SKILLS.iter().find(|(k, _, _)| *k == s).map_or("", |(_, _, a)| a).to_uppercase();
            PickItem::new(skill_name(&s), ab, s)
        })
        .collect();
    View::Pick(Picker::multi(title, items, g.count))
}

fn set_score(ch: &mut Character, a: &str, v: i64) {
    let ab = &mut ch.abilities;
    match a {
        "str" => ab.str = v,
        "dex" => ab.dex = v,
        "con" => ab.con = v,
        "int" => ab.int = v,
        "wis" => ab.wis = v,
        _ => ab.cha = v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_buy_respects_budget() {
        let mut ed = AbilityEditor::new(Method::PointBuy);
        for _ in 0..7 {
            ed.adjust(true);
        }
        assert_eq!(ed.scores[0], 15, "capped at 15");
        assert_eq!(ed.points_left(), 18);
        ed.cursor = 1;
        for _ in 0..7 {
            ed.adjust(true);
        }
        ed.cursor = 2;
        for _ in 0..7 {
            ed.adjust(true);
        }
        assert_eq!(ed.points_left(), 0);
        assert_eq!(ed.scores[2], 15);
        ed.cursor = 3;
        ed.adjust(true);
        assert_eq!(ed.scores[3], 8, "no points left");
    }

    #[test]
    fn standard_array_swaps_values() {
        let mut ed = AbilityEditor::new(Method::StandardArray);
        ed.cursor = 5; // Charisma 8
        ed.adjust(true);
        assert_eq!(ed.scores, [15, 14, 13, 12, 8, 10]);
        let mut sorted = ed.scores;
        sorted.sort();
        assert_eq!(sorted, [8, 10, 12, 13, 14, 15], "always a permutation");
    }
}
