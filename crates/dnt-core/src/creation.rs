//! Creating a character as questions and answers: species → subspecies
//! → background → ability scores → ability increases → feats → class →
//! subclass → skills → feature choices → equipment → review. Steps with
//! nothing to choose are skipped; going back undoes the last answer.

use std::collections::BTreeMap;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use dnt_rules::character::{AbilityBonus, Character, Level};
use dnt_rules::choices;
use dnt_rules::equipment;
use dnt_rules::levelup;
use dnt_rules::origin::{self, AbilityGrant};
use dnt_rules::overlay::resolve_id;
use dnt_rules::standard::{ABILITIES, SKILLS};
use serde_json::Value as Json;

use crate::flow::{choice_question, pick, AbilityScores, Answer, Method, Question};
use crate::options::Choice;

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
    Equipment,
    AnyItem,
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
    Step::Equipment,
    Step::AnyItem,
    Step::Review,
];

/// Everything that changes as the wizard advances (snapshotted for Esc).
#[derive(Debug, Clone)]
struct State {
    step: Step,
    question: Question,
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
    feature_choices: Option<Vec<choices::Choice>>,
    /// Starting-equipment groups still to grant; `None` until computed.
    equipment: Option<Vec<(String, Vec<equipment::Alternative>)>>,
    /// The group currently being chosen from.
    equipment_choice: Option<(String, Vec<equipment::Alternative>)>,
    /// "Any martial weapon"-style picks still to make: (kind, quantity).
    any_items: Vec<(String, i64)>,
}

pub struct NewCharacter {
    state: State,
    history: Vec<State>,
}

fn name_of(lib: &Library, id: Option<EntityId>) -> String {
    id.map(|e| lib.entity(e).name.clone()).unwrap_or_default()
}

fn entity_items(lib: &Library, kind: Kind, filter: &SourceFilter, id: impl Fn(EntityId) -> String) -> Vec<Choice> {
    let mut ids: Vec<EntityId> =
        (0..lib.entities().len()).filter(|&i| lib.entity(i).kind == kind && filter.allows(lib.entity(i))).collect();
    ids.sort_by(|&a, &b| lib.entity(a).name.cmp(&lib.entity(b).name));
    ids.into_iter().map(|i| Choice::new(lib.entity(i).name.clone(), lib.entity(i).source.clone(), id(i))).collect()
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
            AbilityGrant::Choose { from, count, amount } => {
                format!("{amount:+} to {count} of {}", from.iter().map(|a| a.to_uppercase()).collect::<Vec<_>>().join("/"))
            }
            AbilityGrant::Weighted { from, weights } => format!(
                "{} among {}",
                weights.iter().map(|w| format!("{w:+}")).collect::<Vec<_>>().join(" / "),
                from.iter().map(|a| ability_name(a)).collect::<Vec<_>>().join(", ")
            ),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

impl NewCharacter {
    pub fn new(name: String, lib: &Library, filter: &SourceFilter) -> NewCharacter {
        let state = State {
            step: Step::Species,
            question: Question::Confirm,
            ch: Character { name, ..Character::default() },
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
            equipment: None,
            equipment_choice: None,
            any_items: Vec::new(),
        };
        let mut w = NewCharacter { state, history: Vec::new() };
        w.enter(Step::Species, lib, filter);
        w
    }

    pub fn name(&self) -> &str {
        &self.state.ch.name
    }

    pub fn question(&self) -> &Question {
        &self.state.question
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
            ("Equipment", Step::Equipment),
            ("Review", Step::Review),
        ]
        .into_iter()
        .map(|(label, step)| {
            let at = ORDER.iter().position(|s| *s == step).unwrap();
            let next = ORDER
                .iter()
                .skip(at + 1)
                .position(|s| {
                    matches!(
                        s,
                        Step::Background
                            | Step::Method
                            | Step::BonusAlternative
                            | Step::Feat
                            | Step::Class
                            | Step::ClassSkills
                            | Step::Equipment
                            | Step::Review
                    )
                })
                .map_or(ORDER.len(), |p| at + 1 + p);
            (label, cur >= next, (at..next).contains(&cur))
        })
        .collect()
    }

    /// Contents of a starting-equipment package (by its choice value), if
    /// choosing one.
    pub fn equipment_preview(&self, lib: &Library, value: &str) -> Option<(String, Vec<String>)> {
        if self.state.step != Step::Equipment {
            return None;
        }
        let (origin, group) = self.state.equipment_choice.as_ref()?;
        let i: usize = value.parse().ok()?;
        let alt = group.get(i)?;
        let lines = equipment::describe(lib, alt).split(", ").map(str::to_string).collect();
        Some((format!("{origin} · option {}", alt.key), lines))
    }

    /// The entity a choice value stands for, to preview it.
    pub fn preview(&self, lib: &Library, value: &str) -> Option<EntityId> {
        resolve_id(lib, value.strip_prefix("version:").unwrap_or(value))
    }

    /// Undo the last answer; false when there is nothing to undo.
    pub fn back(&mut self) -> bool {
        match self.history.pop() {
            Some(prev) => {
                self.state = prev;
                true
            }
            None => false,
        }
    }

    /// Answer the current question and move on.
    pub fn answer(&mut self, lib: &Library, filter: &SourceFilter, answer: Answer) -> Result<(), String> {
        match (&self.state.question, answer) {
            (Question::Abilities { scores: ed }, Answer::Scores(scores)) => {
                ed.check(&scores)?;
                let method = ed.method;
                self.snapshot();
                let st = &mut self.state;
                for (i, (a, _)) in ABILITIES.iter().enumerate() {
                    set_score(&mut st.ch, a, scores[i]);
                }
                let text: Vec<String> =
                    ABILITIES.iter().enumerate().map(|(i, (a, _))| format!("{} {}", a.to_uppercase(), scores[i])).collect();
                st.summary.push(("Abilities".into(), format!("{} ({})", text.join(" "), method.label())));
                self.advance(Step::Abilities, lib, filter);
                Ok(())
            }
            (Question::Pick { choices, count, .. }, Answer::Pick(values)) => {
                if values.is_empty() || values.len() > *count {
                    return Err(format!("Choose {count}"));
                }
                let mut labels = Vec::new();
                for v in &values {
                    labels.push(
                        choices.iter().find(|c| &c.value == v).ok_or_else(|| format!("{v} is not one of the choices"))?.label.clone(),
                    );
                }
                let multi = *count > 1;
                self.snapshot();
                self.picked(values, multi, labels.join(", "), lib, filter);
                Ok(())
            }
            _ => Err("That does not answer the question".into()),
        }
    }

    /// The finished character (once everything is answered).
    pub fn finish(&mut self) -> Result<Character, String> {
        if self.state.step != Step::Review {
            return Err("Not everything is chosen yet".into());
        }
        self.flush_bonuses();
        let mut ch = self.state.ch.clone();
        ch.proficiencies.skills.sort();
        ch.proficiencies.skills.dedup();
        Ok(ch)
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
        let question = match step {
            Step::Species => {
                let mut items = entity_items(lib, Kind::Race, filter, |i| format!("race:{}|{}", lib.entity(i).name, lib.entity(i).source));
                // Lineages/ancestries are chosen in the next step.
                items.retain(|it| resolve_id(lib, &it.value).is_none_or(|e| lib.version_base(e).is_none()));
                pick("Choose a species", items)
            }
            Step::Subspecies => {
                let Some(sp) = st.species else { return false };
                let race = lib.entity(sp);
                let mut items: Vec<Choice> = (0..lib.entities().len())
                    .filter(|&i| {
                        let e = lib.entity(i);
                        e.kind == Kind::Subrace
                            && filter.allows(e)
                            && e.data.get("raceName").and_then(Json::as_str) == Some(&race.name)
                            && e.data.get("raceSource").and_then(Json::as_str) == Some(&race.source)
                    })
                    .map(|i| {
                        Choice::new(
                            lib.entity(i).name.clone(),
                            lib.entity(i).source.clone(),
                            format!("race:{}|{}", lib.entity(i).name, lib.entity(i).source),
                        )
                    })
                    .collect();
                // Versions: 2024 lineages, legacies and ancestries.
                items.extend(lib.versions_of(sp).iter().map(|&v| {
                    let e = lib.entity(v);
                    Choice::new(e.name.clone(), e.source.clone(), format!("version:race:{}|{}", e.name, e.source))
                }));
                if items.is_empty() {
                    return false;
                }
                items.sort_by(|a, b| a.label.cmp(&b.label));
                let what = if lib.versions_of(sp).is_empty() { "subrace" } else { "lineage" };
                pick(format!("{}: choose a {what}", race.name), items)
            }
            Step::Background => pick(
                "Choose a background",
                entity_items(lib, Kind::Background, filter, |i| format!("background:{}|{}", lib.entity(i).name, lib.entity(i).source)),
            ),
            Step::Method => pick(
                "How do you want to set ability scores?",
                vec![
                    Choice::new("Standard array", "15 14 13 12 10 8", "array"),
                    Choice::new("Point buy", "27 points, 8–15", "buy"),
                    Choice::new("Manual entry", "rolled at the table, or anything else", "manual"),
                ],
            ),
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
                let items = alts.iter().enumerate().map(|(i, a)| Choice::new(describe_alternative(a), "", i.to_string())).collect();
                pick(format!("{origin}: ability increases"), items)
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
                        Choice::new(format!("{}  {score} → {}", ability_name(a), score + amount), a.to_uppercase(), a.clone())
                    })
                    .collect();
                pick(format!("{}: {amount:+} to which ability?", task.origin), items)
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
                    .map(|i| Choice::new(lib.entity(i).name.clone(), lib.entity(i).source.clone(), levelup::feat_id(lib, i)))
                    .collect();
                pick("Choose a feat", items)
            }
            Step::Class => pick("Choose your starting class", entity_items(lib, Kind::Class, filter, |i| levelup::class_id(lib, i))),
            Step::Subclass => {
                let Some(class) = st.class else { return false };
                if levelup::subclass_level(&lib.entity(class).data) != Some(1) {
                    return false;
                }
                let items: Vec<Choice> = levelup::subclass_options(lib, class, filter)
                    .into_iter()
                    .map(|s| Choice::new(lib.entity(s).name.clone(), lib.entity(s).source.clone(), levelup::subclass_id(lib, s)))
                    .collect();
                if items.is_empty() {
                    return false;
                }
                pick("Choose a subclass", items)
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
                choice_question(choice, choice.count)
            }
            Step::Equipment => {
                if st.equipment.is_none() {
                    let mut groups = Vec::new();
                    for (origin, id) in [("Class", st.class), ("Background", st.background)] {
                        let Some(id) = id else { continue };
                        let e = lib.entity(id);
                        for g in equipment::starting_equipment(e.data.get("startingEquipment")) {
                            groups.push((format!("{origin} ({})", e.name), g));
                        }
                    }
                    st.equipment = Some(groups);
                }
                // Grant groups without a real choice; stop at the first choice.
                loop {
                    let Some((origin, group)) = st.equipment.as_mut().and_then(|q| (!q.is_empty()).then(|| q.remove(0))) else {
                        return self.enter(Step::AnyItem, lib, filter);
                    };
                    if group.len() == 1 {
                        grant_alternative(st, lib, &origin, &group[0]);
                        continue;
                    }
                    let items = group
                        .iter()
                        .enumerate()
                        .map(|(i, alt)| Choice::new(format!("{}: {}", alt.key, equipment::describe(lib, alt)), "", i.to_string()))
                        .collect();
                    st.equipment_choice = Some((origin.clone(), group));
                    break pick(format!("{origin}: starting equipment"), items);
                }
            }
            Step::AnyItem => {
                let Some((kind, quantity)) = st.any_items.first().cloned() else { return false };
                let items: Vec<Choice> = equipment::items_of_kind(lib, &kind, filter)
                    .into_iter()
                    .map(|i| {
                        let e = lib.entity(i);
                        Choice::new(e.name.clone(), e.source.clone(), format!("item:{}|{}", e.name, e.source))
                    })
                    .collect();
                Question::Pick { title: format!("Choose {}", equipment::any_label(&kind)), choices: items, count: quantity.max(1) as usize }
            }
            Step::Review => {
                equipment::auto_equip(lib, &mut st.ch);
                Question::Confirm
            }
        };
        st.step = step;
        st.question = question;
        true
    }

    fn picked(&mut self, values: Vec<String>, multi: bool, label: String, lib: &Library, filter: &SourceFilter) {
        let step = self.state.step;
        let st = &mut self.state;
        let one = if multi { String::new() } else { values[0].clone() };
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
                st.question = Question::Abilities { scores: AbilityScores::new(method) };
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
                st.ch.levels = vec![Level { class: one, hp: Default::default(), choices: Default::default() }];
                st.summary.push(("Class".into(), format!("{label} 1")));
            }
            Step::Subclass => {
                if let Some(class) = st.ch.levels.first().map(|l| l.class.clone()) {
                    st.ch.subclasses.insert(class, one);
                }
                st.summary.push(("Subclass".into(), label));
            }
            Step::ClassSkills | Step::OriginSkills => {
                let skills = values;
                let names = skills.iter().map(|s| skill_name(s)).collect::<Vec<_>>().join(", ");
                st.ch.proficiencies.skills.extend(skills);
                let from = if step == Step::OriginSkills { st.origin_skill_choices.remove(0).0 } else { "Class".into() };
                st.summary.push((format!("Skills ({from})"), names));
                if step == Step::OriginSkills && self.enter(Step::OriginSkills, lib, filter) {
                    return;
                }
            }
            Step::FeatureChoice => {
                let choice = st.feature_choices.as_mut().map(|q| q.remove(0)).expect("choice in progress");
                let names: Vec<String> =
                    values.iter().filter_map(|v| choice.options.iter().find(|o| &o.value == v).map(|o| o.label.clone())).collect();
                st.summary.push((choice.key.clone(), names.join(", ")));
                if let Some(level) = st.ch.levels.first_mut() {
                    level.choices.picks.entry(choice.key).or_default().extend(values);
                }
                if self.enter(Step::FeatureChoice, lib, filter) {
                    return;
                }
            }
            Step::Equipment => {
                let (origin, group) = st.equipment_choice.take().expect("equipment choice in progress");
                let i: usize = one.parse().unwrap_or(0);
                grant_alternative(st, lib, &origin, &group[i]);
                if self.enter(Step::AnyItem, lib, filter) || self.enter(Step::Equipment, lib, filter) {
                    return;
                }
            }
            Step::AnyItem => {
                let (kind, _) = st.any_items.remove(0);
                for v in &values {
                    equipment::add_item(&mut st.ch, v, 1, None);
                }
                let names: Vec<String> = values.iter().filter_map(|v| resolve_id(lib, v)).map(|e| lib.entity(e).name.clone()).collect();
                push_merged(&mut st.summary, "Equipment", format!("{} ({})", names.join(", "), equipment::any_label(&kind)));
                if self.enter(Step::AnyItem, lib, filter) || self.enter(Step::Equipment, lib, filter) {
                    return;
                }
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
        st.ch.ability_bonus =
            st.bonuses.iter().map(|(origin, scores)| AbilityBonus { origin: origin.clone(), scores: scores.clone() }).collect();
    }

    /// The character as it would be created now (for the review preview).
    /// The character as it would be created now, with `scores` while they
    /// are being set (for a preview).
    pub fn preview_character(&self, scores: Option<&[i64; 6]>) -> Character {
        let mut ch = self.state.ch.clone();
        if let (Question::Abilities { .. }, Some(scores)) = (&self.state.question, scores) {
            for (i, (a, _)) in ABILITIES.iter().enumerate() {
                set_score(&mut ch, a, scores[i]);
            }
        }
        ch.ability_bonus =
            self.state.bonuses.iter().map(|(origin, scores)| AbilityBonus { origin: origin.clone(), scores: scores.clone() }).collect();
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
            AbilityGrant::Choose { from, count, amount } => {
                st.bonus_tasks.push(BonusTask { origin: origin.to_string(), amounts: vec![*amount; *count], from: from.clone() })
            }
            AbilityGrant::Weighted { from, weights } => {
                st.bonus_tasks.push(BonusTask { origin: origin.to_string(), amounts: weights.clone(), from: from.clone() })
            }
        }
    }
    // Fixed-only grants are recorded right away.
    let bonuses = st.bonuses.clone();
    st.ch.ability_bonus = bonuses.into_iter().map(|(origin, scores)| AbilityBonus { origin, scores }).collect();
}

/// Grant one starting-equipment alternative and queue its "any …" picks.
fn grant_alternative(st: &mut State, lib: &Library, origin: &str, alt: &equipment::Alternative) {
    equipment::grant(&mut st.ch, alt);
    for e in &alt.entries {
        if let equipment::Entry::AnyOf { kind, quantity } = e {
            st.any_items.push((kind.clone(), *quantity));
        }
    }
    let text = equipment::describe(lib, alt);
    if !text.is_empty() {
        st.summary.push((origin.to_string(), text));
    }
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

fn skill_picker(ch: &Character, title: &str, g: &origin::SkillGrant) -> Question {
    let items = g
        .options()
        .into_iter()
        .filter(|s| !ch.proficiencies.skills.contains(s))
        .map(|s| {
            let ab = SKILLS.iter().find(|(k, _, _)| *k == s).map_or("", |(_, _, a)| a).to_uppercase();
            Choice::new(skill_name(&s), ab, s)
        })
        .collect();
    Question::Pick { title: title.to_string(), choices: items, count: g.count }
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
    use crate::flow::STANDARD_ARRAY;
    use serde_json::json;

    /// Answer every question with its first choices until the review.
    fn walk(w: &mut NewCharacter, lib: &Library, filter: &SourceFilter) -> Vec<String> {
        let mut titles = Vec::new();
        for _ in 0..30 {
            let answer = match w.question() {
                Question::Pick { title, choices, count } => {
                    titles.push(title.clone());
                    Answer::Pick(choices.iter().take(*count).map(|c| c.value.clone()).collect())
                }
                Question::Abilities { .. } => Answer::Scores(STANDARD_ARRAY),
                Question::Confirm => return titles,
                Question::Number { .. } => panic!("no numbers when creating"),
            };
            w.answer(lib, filter, answer).unwrap();
        }
        panic!("never reached the review: {titles:?}");
    }

    #[test]
    fn a_character_is_created_by_answering() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "race": [{"name": "Human", "source": "T", "ability": [{"str": 1}]}],
            "background": [{"name": "Acolyte", "source": "T", "skillProficiencies": [{"insight": true}]}],
            "class": [{"name": "Fighter", "source": "T", "hd": {"number": 1, "faces": 10},
                       "startingProficiencies": {"skills": [{"choose": {"from": ["athletics", "acrobatics"], "count": 1}}]}}]
        }));
        let filter = SourceFilter::default();
        let mut w = NewCharacter::new("Ada".into(), &lib, &filter);
        assert!(w.finish().is_err(), "not done yet");
        assert!(w.answer(&lib, &filter, Answer::Scores(STANDARD_ARRAY)).is_err(), "scores do not answer a species question");

        // Wrong scores are refused at the ability step; back undoes an answer.
        w.answer(&lib, &filter, Answer::Pick(vec!["race:Human|T".into()])).unwrap();
        w.answer(&lib, &filter, Answer::Pick(vec!["background:Acolyte|T".into()])).unwrap();
        w.answer(&lib, &filter, Answer::Pick(vec!["array".into()])).unwrap();
        assert!(matches!(w.question(), Question::Abilities { .. }));
        assert!(w.answer(&lib, &filter, Answer::Scores([18; 6])).is_err());
        assert!(w.back());
        assert!(matches!(w.question(), Question::Pick { .. }), "back at the method");

        let titles = walk(&mut w, &lib, &filter);
        assert!(titles.iter().any(|t| t.contains("skill")), "{titles:?}");
        let ch = w.finish().unwrap();
        assert_eq!((ch.name.as_str(), ch.abilities.str), ("Ada", 15));
        assert_eq!(ch.ability_bonus[0].scores.get("str"), Some(&1), "Human +1 Strength");
        assert_eq!(ch.levels.len(), 1);
        assert_eq!(ch.proficiencies.skills, ["athletics", "insight"]);
    }
}
