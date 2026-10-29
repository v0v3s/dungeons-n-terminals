//! Creating a character as questions and answers: species → subspecies
//! → background → ability scores → ability increases → feats → class →
//! subclass → skills → feature choices → equipment → review. Steps with
//! nothing to choose are skipped; going back undoes the last answer, and
//! a finished step can be reopened (undoing what came after it).

use std::collections::BTreeMap;

use dnt_data::{EntityId, Kind, Library, SourceFilter};
use dnt_rules::character::{AbilityBonus, Character, Level};
use dnt_rules::choices;
use dnt_rules::equipment;
use dnt_rules::levelup;
use dnt_rules::origin::{self, AbilityGrant};
use dnt_rules::overlay::resolve_id;
use dnt_rules::standard::{ABILITIES, SKILLS};
use serde::Serialize;
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

const ORDER: [Step; 15] = [
    Step::Species,
    Step::Subspecies,
    Step::Background,
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
    /// What was chosen, each with the step it was chosen at.
    summary: Vec<(Step, String, String)>,
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

/// The steps as a checklist shows them: each from its first step to the
/// next one's.
const GROUPS: [(&str, Step); 9] = [
    ("Species", Step::Species),
    ("Background", Step::Background),
    ("Ability scores", Step::Abilities),
    ("Ability increases", Step::BonusAlternative),
    ("Feats", Step::Feat),
    ("Class", Step::Class),
    ("Skills & features", Step::ClassSkills),
    ("Equipment", Step::Equipment),
    ("Review", Step::Review),
];

fn position(step: Step) -> usize {
    ORDER.iter().position(|s| *s == step).unwrap()
}

/// The checklist group a step is in.
fn group_of(step: Step) -> usize {
    GROUPS.iter().rposition(|(_, s)| position(*s) <= position(step)).unwrap_or(0)
}

/// A step of the checklist.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepRow {
    pub label: &'static str,
    pub done: bool,
    pub current: bool,
    /// What was chosen in it.
    pub chosen: Vec<(String, String)>,
    /// Whether it can be reopened (it asked something, before the current
    /// question).
    pub reopens: bool,
}

fn name_of(lib: &Library, id: Option<EntityId>) -> String {
    id.map(|e| lib.entity(e).name.clone()).unwrap_or_default()
}

fn entity_items(lib: &Library, kind: Kind, filter: &SourceFilter, id: impl Fn(EntityId) -> String) -> Vec<Choice> {
    let mut ids: Vec<EntityId> =
        (0..lib.entities().len()).filter(|&i| lib.entity(i).kind == kind && filter.allows(lib.entity(i))).collect();
    ids.sort_by(|&a, &b| lib.entity(a).name.cmp(&lib.entity(b).name));
    ids.into_iter().map(|i| Choice::new(lib.entity(i).name.clone(), hint(lib, i), id(i))).collect()
}

fn short(a: &str) -> String {
    a.to_uppercase()
}

/// An ability grant in a few words: `INT +2`, `+2/+1 INT/WIS/CHA`.
fn grant_short(parts: &[AbilityGrant]) -> String {
    let list = |from: &[String]| from.iter().map(|a| short(a)).collect::<Vec<_>>().join("/");
    parts
        .iter()
        .map(|p| match p {
            AbilityGrant::Fixed(m) => m.iter().map(|(a, n)| format!("{} {n:+}", short(a))).collect::<Vec<_>>().join(", "),
            AbilityGrant::Choose { from, count, amount } => format!("{amount:+} to {count} of {}", list(from)),
            AbilityGrant::Weighted { from, weights } => {
                format!("{} {}", weights.iter().map(|w| format!("{w:+}")).collect::<Vec<_>>().join("/"), list(from))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Skills granted in a few words: `History, Insight`, `2 of Athletics,
/// Stealth`, `any 1 skill`.
fn skills_short(g: &origin::SkillGrant) -> String {
    let mut parts: Vec<String> = g.fixed.iter().map(|s| skill_name(s).to_string()).collect();
    if g.count > 0 {
        parts.push(if g.from.is_empty() {
            format!("any {} skill{}", g.count, if g.count > 1 { "s" } else { "" })
        } else {
            format!("{} of {}", g.count, g.from.iter().map(|s| skill_name(s)).collect::<Vec<_>>().join(", "))
        });
    }
    parts.join(", ")
}

/// What a species, background, class or feat gives, in one line for its
/// row in a list (then its source): increases, skills, speed, hit die.
fn hint(lib: &Library, id: EntityId) -> String {
    let e = lib.entity(id);
    let d = &e.data;
    let mut parts: Vec<String> = Vec::new();
    match e.kind {
        Kind::Race | Kind::Subrace => {
            parts.extend(origin::species_ability_grants(d).first().map(|g| grant_short(g)));
            let speed = match d.get("speed") {
                Some(Json::Object(o)) => o.get("walk").and_then(Json::as_i64),
                Some(s) => s.as_i64(),
                None => None,
            };
            parts.extend(speed.map(|s| format!("speed {s} ft")));
            parts.extend(d.get("darkvision").and_then(Json::as_i64).map(|v| format!("darkvision {v} ft")));
            parts.push(skills_short(&origin::skill_grant(d.get("skillProficiencies"))));
        }
        Kind::Background => {
            parts.push(skills_short(&origin::skill_grant(d.get("skillProficiencies"))));
            parts.extend(origin::ability_grants(d.get("ability")).first().map(|g| grant_short(g)));
            let feats = origin::feat_grant(d.get("feats")).fixed;
            parts.extend(feats.iter().filter_map(|f| resolve_id(lib, f)).map(|f| lib.entity(f).name.clone()));
        }
        Kind::Class => {
            parts.extend(d.get("hd").and_then(|h| h.get("faces")).and_then(Json::as_i64).map(|f| format!("d{f}")));
            let primary: Vec<String> = d
                .get("primaryAbility")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
                .filter_map(Json::as_object)
                .map(|alt| alt.keys().map(|a| short(a)).collect::<Vec<_>>().join(" & "))
                .collect();
            if !primary.is_empty() {
                parts.push(primary.join(" or "));
            }
            let saves: Vec<String> =
                d.get("proficiency").and_then(Json::as_array).into_iter().flatten().filter_map(Json::as_str).map(short).collect();
            if !saves.is_empty() {
                parts.push(format!("saves {}", saves.join(", ")));
            }
        }
        Kind::Feat => {
            let alt = d.get("prerequisite").and_then(Json::as_array).and_then(|a| a.first());
            let mut needs = Vec::new();
            for (k, v) in alt.and_then(Json::as_object).into_iter().flatten() {
                match (k.as_str(), v) {
                    ("level", Json::Number(n)) => needs.push(format!("level {n}")),
                    ("level", Json::Object(l)) => needs.extend(l.get("level").and_then(Json::as_i64).map(|n| format!("level {n}"))),
                    ("other", Json::String(o)) => needs.push(o.clone()),
                    ("spellcasting" | "spellcasting2020", _) => needs.push("spellcasting".into()),
                    ("ability", Json::Array(a)) => needs.extend(
                        a.iter()
                            .filter_map(Json::as_object)
                            .flat_map(|m| m.iter().filter_map(|(a, n)| Some(format!("{} {}", short(a), n.as_i64()?)))),
                    ),
                    _ => {}
                }
            }
            if !needs.is_empty() {
                parts.push(format!("needs {}", needs.join(", ")));
            }
        }
        _ => {}
    }
    parts.retain(|p| !p.is_empty());
    parts.push(e.source.clone());
    parts.join(" · ")
}

fn ability_name(a: &str) -> &'static str {
    ABILITIES.iter().find(|(k, _)| *k == a).map_or("?", |(_, n)| n)
}

fn skill_name(k: &str) -> &'static str {
    SKILLS.iter().find(|(s, _, _)| *s == k).map_or("?", |(_, n, _)| n)
}

fn describe_alternative(origin: &str, parts: &[AbilityGrant]) -> String {
    if parts.is_empty() {
        return if origin.starts_with("Background") {
            "Skip: no increases from the background".into()
        } else {
            "None: the background gives the increases (2024 rules)".into()
        };
    }
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

    /// What was chosen so far, in order.
    pub fn summary(&self) -> Vec<(String, String)> {
        self.state.summary.iter().map(|(_, k, v)| (k.clone(), v.clone())).collect()
    }

    /// Step titles for the sidebar, with whether each is done/current.
    pub fn progress(&self) -> Vec<(&'static str, bool, bool)> {
        self.steps().into_iter().map(|r| (r.label, r.done, r.current)).collect()
    }

    /// The steps as a checklist: each with what was chosen in it, and
    /// whether it can be reopened.
    pub fn steps(&self) -> Vec<StepRow> {
        let cur = group_of(self.state.step);
        GROUPS
            .iter()
            .enumerate()
            .map(|(g, (label, _))| StepRow {
                label,
                done: g < cur,
                current: g == cur,
                chosen: self.state.summary.iter().filter(|(s, _, _)| group_of(*s) == g).map(|(_, k, v)| (k.clone(), v.clone())).collect(),
                reopens: self.history.iter().any(|h| group_of(h.step) == g),
            })
            .collect()
    }

    /// Go back to the first question of checklist step `group`, undoing
    /// everything answered since; false when it asked nothing yet.
    pub fn reopen(&mut self, group: usize) -> bool {
        let Some(i) = self.history.iter().position(|h| group_of(h.step) == group) else { return false };
        self.history.truncate(i + 1);
        self.state = self.history.pop().expect("a step to reopen");
        true
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

    /// Whether the choices asked for have a text to read (a species, a
    /// class, an equipment package), judged by the first.
    pub fn details(&self, lib: &Library) -> bool {
        let Question::Pick { choices, .. } = &self.state.question else { return false };
        choices.first().is_some_and(|c| self.preview(lib, &c.value).is_some() || self.equipment_preview(lib, &c.value).is_some())
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
            (Question::Abilities { .. }, Answer::Method(method)) => {
                self.state.question = Question::Abilities { scores: AbilityScores::new(method) };
                Ok(())
            }
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
                st.summary.push((Step::Abilities, "Abilities".into(), format!("{} ({})", text.join(" "), method.label())));
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
                            hint(lib, i),
                            format!("race:{}|{}", lib.entity(i).name, lib.entity(i).source),
                        )
                    })
                    .collect();
                // Versions: 2024 lineages, legacies and ancestries.
                items.extend(lib.versions_of(sp).iter().map(|&v| {
                    let e = lib.entity(v);
                    Choice::new(e.name.clone(), hint(lib, v), format!("version:race:{}|{}", e.name, e.source))
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
            // The standard array to start with; the method can change.
            Step::Abilities => Question::Abilities { scores: AbilityScores::new(Method::StandardArray) },
            Step::BonusAlternative => {
                // Gather ability grants from species/subspecies/background.
                if st.alternatives.is_empty() && st.bonus_tasks.is_empty() {
                    // A 2024 background gives the increases; a species may too
                    // (older ones, or those for customizing your origin): then
                    // its increases can be left out.
                    let background_gives =
                        st.background.is_some_and(|b| !origin::ability_grants(lib.entity(b).data.get("ability")).is_empty());
                    for (prefix, id) in [("Species", st.species), ("Species", st.subspecies), ("Background", st.background)] {
                        let Some(id) = id else { continue };
                        let data = &lib.entity(id).data;
                        let mut grants = if prefix == "Species" {
                            origin::species_ability_grants(data)
                        } else {
                            origin::ability_grants(data.get("ability"))
                        };
                        if grants.is_empty() {
                            continue;
                        }
                        // Increases can be left out: a species' when the
                        // background gives them, a background's always
                        // (a table's own rules, or a species that has them).
                        if prefix == "Background" || background_gives {
                            grants.push(Vec::new());
                        }
                        st.alternatives.push((format!("{prefix} ({})", lib.entity(id).name), grants));
                    }
                }
                let Some((origin, alts)) = st.alternatives.first().cloned() else { return false };
                if alts.len() == 1 {
                    st.alternatives.remove(0);
                    queue_alternative(st, &origin, &alts[0]);
                    return self.enter(Step::Bonus, lib, filter);
                }
                let items =
                    alts.iter().enumerate().map(|(i, a)| Choice::new(describe_alternative(&origin, a), "", i.to_string())).collect();
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
                            st.summary.push((Step::Feat, "Feat".into(), name_of(lib, Some(e))));
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
                    .map(|i| Choice::new(lib.entity(i).name.clone(), hint(lib, i), levelup::feat_id(lib, i)))
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
                // Fixed skills from species, background and origin feats
                // (Skilled) first.
                if st.origin_skill_choices.is_empty() {
                    let feats: Vec<EntityId> = st.ch.feats.iter().filter_map(|f| dnt_rules::overlay::resolve_id(lib, f)).collect();
                    for id in [st.species, st.subspecies, st.background].into_iter().flatten().chain(feats) {
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
                st.summary.push((step, "Species".into(), label));
            }
            Step::Subspecies => {
                if let Some(id) = one.strip_prefix("version:") {
                    // A version is a complete species: it replaces the base.
                    st.species = resolve_id(lib, id);
                    st.ch.species = Some(id.to_string());
                    st.summary.push((step, "Lineage".into(), label));
                } else {
                    st.subspecies = resolve_id(lib, &one);
                    st.ch.subspecies = Some(one);
                    st.summary.push((step, "Subrace".into(), label));
                }
            }
            Step::Background => {
                st.background = resolve_id(lib, &one);
                st.ch.background = Some(one);
                st.summary.push((step, "Background".into(), label));
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
                push_merged(&mut st.summary, step, &origin, format!("{} {amount:+}", ability_name(&one)));
                // Stay on this step while choices remain.
                if self.enter(Step::Bonus, lib, filter) {
                    return;
                }
                self.flush_bonuses();
            }
            Step::Feat => {
                st.summary.push((step, "Feat".into(), label));
                st.ch.feats.push(one);
            }
            Step::Class => {
                st.class = resolve_id(lib, &one);
                st.ch.levels = vec![Level { class: one, hp: Default::default(), choices: Default::default() }];
                st.summary.push((step, "Class".into(), format!("{label} 1")));
            }
            Step::Subclass => {
                if let Some(class) = st.ch.levels.first().map(|l| l.class.clone()) {
                    st.ch.subclasses.insert(class, one);
                }
                st.summary.push((step, "Subclass".into(), label));
            }
            Step::ClassSkills | Step::OriginSkills => {
                let skills = values;
                let names = skills.iter().map(|s| skill_name(s)).collect::<Vec<_>>().join(", ");
                st.ch.proficiencies.skills.extend(skills);
                let from = if step == Step::OriginSkills { st.origin_skill_choices.remove(0).0 } else { "Class".into() };
                st.summary.push((step, format!("Skills ({from})"), names));
                if step == Step::OriginSkills && self.enter(Step::OriginSkills, lib, filter) {
                    return;
                }
            }
            Step::FeatureChoice => {
                let choice = st.feature_choices.as_mut().map(|q| q.remove(0)).expect("choice in progress");
                let names: Vec<String> =
                    values.iter().filter_map(|v| choice.options.iter().find(|o| &o.value == v).map(|o| o.label.clone())).collect();
                st.summary.push((step, choice.key.clone(), names.join(", ")));
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
                push_merged(&mut st.summary, step, "Equipment", format!("{} ({})", names.join(", "), equipment::any_label(&kind)));
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
                push_merged(&mut st.summary, Step::BonusAlternative, origin, text);
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
        st.summary.push((Step::Equipment, origin.to_string(), text));
    }
}

/// Append to the summary, joining with an existing line of the same key
/// and step.
fn push_merged(summary: &mut Vec<(Step, String, String)>, step: Step, key: &str, text: String) {
    match summary.iter_mut().find(|(s, k, _)| group_of(*s) == group_of(step) && k == key) {
        Some((_, _, v)) => {
            v.push_str(", ");
            v.push_str(&text);
        }
        None => summary.push((step, key.to_string(), text)),
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

        // A row says what it gives.
        let Question::Pick { choices, .. } = w.question() else { panic!("a species to pick") };
        assert_eq!(choices[0].hint, "STR +1 · T");

        // Wrong scores are refused at the ability step; back undoes an answer.
        w.answer(&lib, &filter, Answer::Pick(vec!["race:Human|T".into()])).unwrap();
        w.answer(&lib, &filter, Answer::Pick(vec!["background:Acolyte|T".into()])).unwrap();
        assert!(matches!(w.question(), Question::Abilities { scores } if scores.scores == STANDARD_ARRAY));
        assert!(w.answer(&lib, &filter, Answer::Scores([18; 6])).is_err());
        // The method changes on the same question.
        w.answer(&lib, &filter, Answer::Method(Method::PointBuy)).unwrap();
        assert!(matches!(w.question(), Question::Abilities { scores } if scores.scores == [8; 6]));
        assert!(w.answer(&lib, &filter, Answer::Scores([15, 15, 15, 15, 8, 8])).is_err(), "over 27 points");
        assert!(w.back());
        assert!(matches!(w.question(), Question::Pick { title, .. } if title == "Choose a background"));

        let titles = walk(&mut w, &lib, &filter);
        assert!(titles.iter().any(|t| t.contains("skill")), "{titles:?}");
        let ch = w.finish().unwrap();
        assert_eq!((ch.name.as_str(), ch.abilities.str), ("Ada", 15));
        assert_eq!(ch.ability_bonus[0].scores.get("str"), Some(&1), "Human +1 Strength");
        assert_eq!(ch.levels.len(), 1);
        assert_eq!(ch.proficiencies.skills, ["athletics", "insight"]);
    }

    #[test]
    fn a_finished_step_is_reopened_with_its_choices() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "race": [{"name": "Human", "source": "T"}, {"name": "Dwarf", "source": "T", "speed": 25, "darkvision": 60}],
            "background": [{"name": "Acolyte", "source": "T", "skillProficiencies": [{"insight": true, "religion": true}],
                            "ability": [{"choose": {"weighted": {"from": ["int", "wis"], "weights": [2, 1]}}}]}],
            "class": [{"name": "Fighter", "source": "T", "hd": {"number": 1, "faces": 10},
                       "primaryAbility": [{"str": true}, {"dex": true}], "proficiency": ["str", "con"]}]
        }));
        let filter = SourceFilter::default();
        let mut w = NewCharacter::new("Ada".into(), &lib, &filter);
        let hints = |w: &NewCharacter| match w.question() {
            Question::Pick { choices, .. } => choices.iter().map(|c| c.hint.clone()).collect::<Vec<_>>(),
            _ => panic!("a pick"),
        };
        assert_eq!(hints(&w), ["speed 25 ft · darkvision 60 ft · T", "T"]);
        w.answer(&lib, &filter, Answer::Pick(vec!["race:Human|T".into()])).unwrap();
        assert_eq!(hints(&w), ["Insight, Religion · +2/+1 INT/WIS · T"]);
        w.answer(&lib, &filter, Answer::Pick(vec!["background:Acolyte|T".into()])).unwrap();
        w.answer(&lib, &filter, Answer::Scores(STANDARD_ARRAY)).unwrap();
        while !matches!(w.question(), Question::Pick { title, .. } if title.contains("class")) {
            let Question::Pick { choices, .. } = w.question() else { panic!("{:?}", w.question()) };
            let v = choices[0].value.clone();
            w.answer(&lib, &filter, Answer::Pick(vec![v])).unwrap();
        }
        assert_eq!(hints(&w), ["d10 · STR or DEX · saves STR, CON · T"]);

        let steps = w.steps();
        let species = &steps[0];
        assert!(species.done && species.reopens);
        assert_eq!(species.chosen, [("Species".to_string(), "Human".to_string())]);
        assert!(steps[2].chosen[0].1.starts_with("STR 15"), "{:?}", steps[2]);
        assert_eq!(steps[3].chosen, [("Background (Acolyte)".to_string(), "Intelligence +2, Wisdom +1".to_string())]);
        assert!(steps[5].current && !steps[5].reopens, "the class is being chosen");
        assert!(!steps[4].reopens, "no feats were asked");

        // Reopening the background undoes everything after it.
        assert!(w.reopen(1));
        assert!(matches!(w.question(), Question::Pick { title, .. } if title == "Choose a background"));
        assert_eq!(w.summary(), [("Species".to_string(), "Human".to_string())]);
        assert!(!w.reopen(4), "nothing was asked there");
    }
}
