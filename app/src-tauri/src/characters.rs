//! Characters: the roster, the open sheet, commands and rolls, and the
//! level-up and new-character flows.

use dnt_core::creation::NewCharacter;
use dnt_core::flow::{choice_question, Answer, Question};
use dnt_core::levelup::LevelUp;
use dnt_core::sheet::{self, Breakdown, Condition, FeatureGroup, Header, Inventory, Overview, SpellRow};
use dnt_core::{options, rolls, spellbook, CharacterSession, Choice, Command, Outcome};
use dnt_data::EntityId;
use dnt_rules::standard::SKILLS;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{App, Inner, Res};

#[derive(Serialize)]
pub struct RosterRow {
    slug: String,
    name: String,
    identity: String,
}

#[tauri::command]
pub async fn roster(app: State<'_, App>) -> Res<Vec<RosterRow>> {
    let inner = app.lock();
    let ws = &inner.ws;
    let Some(lib) = ws.lib.as_ref() else { return Ok(Vec::new()) };
    let filter = ws.filter();
    Ok(ws
        .characters()
        .into_iter()
        .map(|f| {
            let slug = f.slug.clone();
            let s = CharacterSession::new(f, lib, &ws.overlays, &filter);
            RosterRow { slug, name: s.character().name.clone(), identity: sheet::identity(lib, &s) }
        })
        .collect())
}

/// Everything the sheet shows.
#[derive(Serialize)]
pub struct Sheet {
    slug: String,
    header: Header,
    overview: Overview,
    features: Vec<FeatureGroup>,
    spells: Vec<SpellRow>,
    inventory: Inventory,
}

pub fn sheet_of(inner: &Inner) -> Res<Sheet> {
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    Ok(Sheet {
        slug: s.file.slug.clone(),
        header: sheet::header(lib, s),
        overview: sheet::overview(lib, s),
        features: sheet::features(lib, &inner.ws.overlays, s),
        spells: sheet::spells(lib, s),
        inventory: sheet::inventory(lib, s),
    })
}

pub fn open_slug(inner: &mut Inner, slug: &str) -> Res<()> {
    let file = inner.ws.characters().into_iter().find(|f| f.slug == slug).ok_or_else(|| format!("No character {slug}"))?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    inner.sheet = Some(CharacterSession::new(file, lib, &inner.ws.overlays, &inner.ws.filter()));
    inner.levelup = None;
    Ok(())
}

#[tauri::command]
pub async fn open_character(app: State<'_, App>, slug: String) -> Res<Sheet> {
    let mut inner = app.lock();
    open_slug(&mut inner, &slug)?;
    sheet_of(&inner)
}

#[tauri::command]
pub async fn close_character(app: State<'_, App>) -> Res<()> {
    let mut inner = app.lock();
    inner.sheet = None;
    inner.levelup = None;
    Ok(())
}

/// Delete the open character's file (their notes stay) and close it.
#[tauri::command]
pub async fn delete_character(app: State<'_, App>) -> Res<String> {
    let mut inner = app.lock();
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    let name = s.character().name.clone();
    inner.ws.store.delete_character(&s.file).map_err(|e| format!("Could not delete {name}: {e}"))?;
    inner.sheet = None;
    inner.levelup = None;
    Ok(format!("Deleted {name}"))
}

#[tauri::command]
pub async fn sheet(app: State<'_, App>) -> Res<Sheet> {
    sheet_of(&app.lock())
}

/// Save the open character after a change.
fn save(inner: &Inner) -> Res<()> {
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    inner.ws.store.save_character(&s.file).map_err(|e| format!("Could not save: {e}"))
}

#[derive(Serialize)]
pub struct Done {
    outcome: Outcome,
    sheet: Sheet,
}

/// Apply a command to the open character (and save it if it changed).
#[tauri::command]
pub async fn command(app: State<'_, App>, command: Command) -> Res<Done> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_mut().ok_or("No character open")?;
    let outcome = s.apply(lib, &inner.ws.overlays, command)?;
    if outcome.changed {
        save(inner)?;
    }
    Ok(Done { outcome, sheet: sheet_of(inner)? })
}

/// What to roll.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Roll {
    Ability {
        key: String,
    },
    Save {
        key: String,
    },
    Skill {
        key: String,
    },
    Initiative,
    Attack {
        slug: String,
    },
    /// A spell, with the casting class's slug for its attack and DC.
    Spell {
        id: String,
        #[serde(default)]
        caster: Option<String>,
    },
}

#[derive(Serialize)]
pub struct Rolled {
    line: String,
    rolled: rolls::Rolled,
    sheet: Sheet,
}

#[tauri::command]
pub async fn roll(app: State<'_, App>, what: Roll) -> Res<Rolled> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_mut().ok_or("No character open")?;
    let r = match what {
        Roll::Ability { key } => rolls::check(s, &key, None),
        Roll::Save { key } => rolls::save(s, &key),
        Roll::Skill { key } => {
            let (_, name, ab) = SKILLS.iter().find(|(k, _, _)| *k == key).ok_or_else(|| format!("No skill {key}"))?;
            rolls::check(s, ab, Some((&key, name)))
        }
        Roll::Initiative => rolls::initiative(s),
        Roll::Attack { slug } => {
            let a = s.build.attacks.iter().find(|a| a.slug == slug).cloned().ok_or("No such attack")?;
            rolls::attack(s, &a)
        }
        Roll::Spell { id, caster } => {
            let spell = dnt_rules::overlay::resolve_id(lib, &id).ok_or("No such spell")?;
            if !rolls::spell_rolls(lib, spell) {
                return Err(format!("{} has nothing to roll", lib.entity(spell).name));
            }
            let (atk, dc) = rolls::spell_numbers(s, caster.as_deref());
            let level = dnt_rules::spells::spell_level(lib, spell);
            rolls::spell(s, lib, spell, level, atk, dc)
        }
    };
    Ok(Rolled { line: r.line(), rolled: r, sheet: sheet_of(inner)? })
}

/// Normal → advantage → disadvantage for the next roll.
#[tauri::command]
pub async fn cycle_advantage(app: State<'_, App>) -> Res<Sheet> {
    let mut inner = app.lock();
    inner.sheet.as_mut().ok_or("No character open")?.roller.cycle();
    sheet_of(&inner)
}

#[tauri::command]
pub async fn breakdown(app: State<'_, App>, key: String) -> Res<Option<Breakdown>> {
    let inner = app.lock();
    Ok(inner.sheet.as_ref().and_then(|s| sheet::breakdown(s, &key)))
}

#[tauri::command]
pub async fn conditions(app: State<'_, App>) -> Res<Vec<(Condition, bool)>> {
    let inner = app.lock();
    Ok(inner.sheet.as_ref().map(sheet::conditions).unwrap_or_default())
}

/// Lists to choose from for a command.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Wanted {
    /// Items to add (values: item ids).
    Items,
    /// Where to wear a line (values: slot names).
    Wear { line: usize },
    /// Containers a line can go into (values: line indexes, or `loose`).
    Containers { line: usize },
    /// Spells a class can add.
    ClassSpells { class_id: String },
    /// Spells for a feature's choice.
    GrantSpells { key: String, filter: String, missing: usize },
    /// How to cast a spell (values: `slot:N`, `pact`, `ritual`).
    Cast { id: String },
    /// Weapons to swap a mastery to.
    Mastery,
}

#[derive(Serialize)]
pub struct Options {
    title: String,
    choices: Vec<Choice>,
    /// How many to pick.
    pick: usize,
    /// The likely choice (index).
    default: usize,
    /// Where picked spells go.
    target: Option<spellbook::Target>,
}

#[tauri::command]
pub async fn options(app: State<'_, App>, wanted: Wanted) -> Res<Options> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    let filter = inner.ws.filter();
    let ch = s.character();
    let plain = |title: &str, choices: Vec<Choice>| Options { title: title.into(), choices, pick: 1, default: 0, target: None };
    Ok(match wanted {
        Wanted::Items => plain("Add an item", options::item_catalog(lib, &filter)),
        Wanted::Wear { line } => {
            if line >= ch.inventory.len() {
                return Err("No such line".into());
            }
            let (mut choices, default) = options::wear_slots(lib, ch, line);
            // Commands take slot names.
            for (c, slot) in choices.iter_mut().zip(dnt_rules::character::Slot::ALL) {
                c.value = serde_json::to_value(slot).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
            }
            Options { default, ..plain("Wear it where?", choices) }
        }
        Wanted::Containers { line } => {
            if line >= ch.inventory.len() {
                return Err("No such line".into());
            }
            plain("Put it in…", options::containers_for(lib, ch, line))
        }
        Wanted::ClassSpells { class_id } => {
            let casters = dnt_rules::spells::casters(lib, &s.build, &s.ev);
            let c = casters.iter().find(|c| c.id == class_id).ok_or("No such class")?;
            let sc = spellbook::class_choices(lib, ch, c, &filter);
            Options { title: sc.title, choices: sc.choices, pick: sc.pick, default: 0, target: Some(sc.target) }
        }
        Wanted::GrantSpells { key, filter: text, missing } => {
            let sc = spellbook::grant_choices(lib, ch, &key, &text, missing, &filter);
            Options { title: sc.title, choices: sc.choices, pick: sc.pick, default: 0, target: Some(sc.target) }
        }
        Wanted::Cast { id } => {
            let spell = dnt_rules::overlay::resolve_id(lib, &id).ok_or("No such spell")?;
            let (title, choices) = spellbook::cast_choices(lib, s, spell).ok_or("No spell slots")?;
            plain(&title, choices)
        }
        Wanted::Mastery => {
            let choices = dnt_rules::choices::mastery_swap_options(lib, ch, &filter)
                .into_iter()
                .map(|o| Choice { label: o.label, hint: o.hint, warning: o.warning, value: o.value })
                .collect();
            plain("Swap the mastery for…", choices)
        }
    })
}

/// The next class choice the character is missing, as a question.
#[derive(Serialize)]
pub struct PendingChoice {
    level: usize,
    key: String,
    /// `Fighter 2`.
    from: String,
    left: usize,
    question: Question,
}

#[tauri::command]
pub async fn pending(app: State<'_, App>) -> Res<Option<PendingChoice>> {
    let inner = app.lock();
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    Ok(s.pending.first().map(|p| PendingChoice {
        level: p.level_index,
        key: p.choice.key.clone(),
        from: format!("{} {}", p.class_name, p.class_level),
        left: s.pending.len(),
        question: choice_question(&p.choice, p.missing),
    }))
}

#[derive(Serialize)]
pub struct Step {
    question: Question,
    summary: Vec<String>,
}

fn levelup_step(flow: &LevelUp) -> Step {
    Step { question: flow.question.clone(), summary: flow.summary.clone() }
}

#[derive(Serialize)]
pub struct RulesDraft {
    /// The overlay id, e.g. `feat:Tough|XPHB`.
    id: String,
    name: String,
    /// `user`, `builtin` (dnt's) or `new` (a template).
    origin: &'static str,
    text: String,
    checked: Checked,
}

#[derive(Serialize)]
pub struct Checked {
    errors: Vec<String>,
    changes: Vec<String>,
}

fn checked(inner: &Inner, entity: EntityId, text: &str) -> Res<Checked> {
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_ref().ok_or("No character is open")?;
    let c = dnt_core::rules::check(lib, entity, text, &s.file.character, &inner.ws.overlays, &s.ev);
    Ok(Checked { errors: c.errors, changes: c.changes })
}

/// A feature's rules to edit: the user's, dnt's built-in ones or a
/// template, checked against the open character.
#[tauri::command]
pub async fn rules_open(app: State<'_, App>, entity: EntityId) -> Res<RulesDraft> {
    use dnt_core::rules::{self, Origin};
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let id = rules::overlay_id(lib, entity);
    let (origin, text) = rules::start(lib, entity, inner.ws.store.feature_overlay(&id));
    let checked = checked(&inner, entity, &text)?;
    Ok(RulesDraft {
        id,
        name: lib.entity(entity).name.clone(),
        origin: match origin {
            Origin::User => "user",
            Origin::Builtin => "builtin",
            Origin::New => "new",
        },
        text,
        checked,
    })
}

#[tauri::command]
pub async fn rules_check(app: State<'_, App>, entity: EntityId, text: String) -> Res<Checked> {
    checked(&app.lock(), entity, &text)
}

/// Save a feature's rules (empty: remove yours, back to dnt's), and apply
/// them.
#[tauri::command]
pub async fn rules_save(app: State<'_, App>, entity: EntityId, text: String) -> Res<Sheet> {
    let mut inner = app.lock();
    let c = checked(&inner, entity, &text)?;
    if let Some(e) = c.errors.first() {
        return Err(format!("Not saved: {e}"));
    }
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let id = dnt_core::rules::overlay_id(lib, entity);
    inner.ws.store.save_feature_overlay(&id, &dnt_core::rules::to_save(&text)).map_err(|e| format!("Could not save the rules: {e}"))?;
    inner.ws.reload_rules();
    let slug = inner.sheet.as_ref().map(|s| s.file.slug.clone()).ok_or("No character is open")?;
    open_slug(&mut inner, &slug)?;
    sheet_of(&inner)
}

/// Whether `amount` damage would kill the open character outright.
#[tauri::command]
pub async fn kills_outright(app: State<'_, App>, amount: i64) -> Res<bool> {
    let inner = app.lock();
    let s = inner.sheet.as_ref().ok_or("No character is open")?;
    Ok(dnt_rules::play::kills_outright(&s.file.character, &s.ev, amount))
}

/// The open character's file (TOML), to edit as text.
#[tauri::command]
pub async fn character_text(app: State<'_, App>) -> Res<String> {
    let inner = app.lock();
    let s = inner.sheet.as_ref().ok_or("No character is open")?;
    inner.ws.store.fs().read_to_string(&s.file.path).map_err(|e| format!("Could not read the character: {e}"))
}

/// Save the open character's file from edited text (checked first), and
/// open it again.
#[tauri::command]
pub async fn character_set_text(app: State<'_, App>, text: String) -> Res<Sheet> {
    let mut inner = app.lock();
    let s = inner.sheet.as_ref().ok_or("No character is open")?;
    dnt_rules::character::Character::from_toml(&text).map_err(|e| format!("Not saved: {e}"))?;
    let (path, slug) = (s.file.path.clone(), s.file.slug.clone());
    inner.ws.store.write_text(&path, &text).map_err(|e| format!("Could not save the character: {e}"))?;
    open_slug(&mut inner, &slug)?;
    sheet_of(&inner)
}

/// Export the open character: `html` (a page to print, or save as PDF
/// from a browser) or `markdown`, to `path` (a path, or a `content://`
/// URI on Android) from the save dialog.
#[tauri::command]
pub async fn export_sheet(handle: tauri::AppHandle, app: State<'_, App>, format: String, path: String) -> Res<String> {
    use std::io::Write;
    use tauri_plugin_fs::{FsExt, OpenOptions};
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_ref().ok_or("No character is open")?;
    let doc = dnt_core::export::character(lib, s);
    let text = match format.as_str() {
        "html" => dnt_core::export::to_html(&doc),
        "markdown" => dnt_core::export::to_markdown(&doc),
        other => return Err(format!("No export as {other}")),
    };
    if let Some(file) = crate::e2e::save_path(&handle, &path) {
        std::fs::write(file, text).map_err(|e| format!("Could not write the file: {e}"))?;
        return Ok(format!("Exported {}", s.file.character.name));
    }
    let file: tauri_plugin_fs::FilePath = path.parse().map_err(|e| format!("{e}"))?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut out = handle.fs().open(file, opts).map_err(|e| format!("Could not write the file: {e}"))?;
    out.write_all(text.as_bytes()).map_err(|e| format!("Could not write the file: {e}"))?;
    Ok(format!("Exported {}", s.file.character.name))
}

#[tauri::command]
pub async fn levelup_start(app: State<'_, App>) -> Res<Step> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    let flow = LevelUp::start(lib, s.character(), &s.ev, &inner.ws.filter());
    let step = levelup_step(&flow);
    inner.levelup = Some(flow);
    Ok(step)
}

#[tauri::command]
pub async fn levelup_answer(app: State<'_, App>, answer: Answer) -> Res<Step> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_ref().ok_or("No character open")?;
    let filter = inner.ws.filter();
    let flow = inner.levelup.as_mut().ok_or("No level-up in progress")?;
    flow.answer(lib, s.character(), &s.ev, &filter, answer)?;
    Ok(levelup_step(flow))
}

#[tauri::command]
pub async fn levelup_apply(app: State<'_, App>) -> Res<Done> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let flow = inner.levelup.take().ok_or("No level-up in progress")?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let s = inner.sheet.as_mut().ok_or("No character open")?;
    let message = flow.apply(&mut s.file.character);
    s.rebuild(lib, &inner.ws.overlays);
    save(inner)?;
    Ok(Done { outcome: Outcome { message, changed: true, rolled: None }, sheet: sheet_of(inner)? })
}

#[derive(Serialize)]
pub struct CreationStep {
    question: Question,
    summary: Vec<(String, String)>,
    /// Steps for a progress list: (label, done, current).
    progress: Vec<(&'static str, bool, bool)>,
    name: String,
}

fn creation_step(flow: &NewCharacter) -> CreationStep {
    CreationStep {
        question: flow.question().clone(),
        summary: flow.summary().to_vec(),
        progress: flow.progress(),
        name: flow.name().into(),
    }
}

#[tauri::command]
pub async fn creation_start(app: State<'_, App>, name: String) -> Res<CreationStep> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    if inner.ws.active_campaign().is_none() {
        return Err("Create a campaign first".into());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("Your character needs a name".into());
    }
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let flow = NewCharacter::new(name.to_string(), lib, &inner.ws.filter());
    let step = creation_step(&flow);
    inner.creation = Some(flow);
    Ok(step)
}

/// The new character in progress, if any (it lasts until it is created
/// or cancelled, whatever else is opened meanwhile).
#[tauri::command]
pub async fn creation_current(app: State<'_, App>) -> Res<Option<CreationStep>> {
    Ok(app.lock().creation.as_ref().map(creation_step))
}

/// Give up the new character in progress.
#[tauri::command]
pub async fn creation_cancel(app: State<'_, App>) -> Res<()> {
    app.lock().creation = None;
    Ok(())
}

#[tauri::command]
pub async fn creation_answer(app: State<'_, App>, answer: Answer) -> Res<CreationStep> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let filter = inner.ws.filter();
    let flow = inner.creation.as_mut().ok_or("No new character in progress")?;
    flow.answer(lib, &filter, answer)?;
    Ok(creation_step(flow))
}

#[tauri::command]
pub async fn creation_back(app: State<'_, App>) -> Res<Option<CreationStep>> {
    let mut inner = app.lock();
    let flow = inner.creation.as_mut().ok_or("No new character in progress")?;
    if flow.back() {
        Ok(Some(creation_step(flow)))
    } else {
        inner.creation = None;
        Ok(None)
    }
}

/// The character as it would be: what a choice (an entity id) is, and
/// the numbers so far (with `scores` while they are being set).
#[derive(Serialize)]
pub struct Preview {
    entity: Option<crate::library::Entry>,
    equipment: Option<(String, Vec<String>)>,
    identity: String,
    stats: Vec<(&'static str, String)>,
    abilities: Vec<(&'static str, String, String)>,
    warnings: Vec<String>,
}

#[tauri::command]
pub async fn creation_preview(app: State<'_, App>, value: Option<String>, scores: Option<[i64; 6]>) -> Res<Preview> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let flow = inner.creation.as_ref().ok_or("No new character in progress")?;
    let entity = value.as_deref().and_then(|v| flow.preview(lib, v)).map(|e| crate::library::entry_of(&inner, e)).transpose()?;
    let equipment = value.as_deref().and_then(|v| flow.equipment_preview(lib, v));
    let ch = flow.preview_character(scores.as_ref());
    let file = dnt_store::CharacterFile { slug: String::new(), path: Default::default(), character: ch };
    let s = CharacterSession::new(file, lib, &inner.ws.overlays, &inner.ws.filter());
    let o = sheet::overview(lib, &s);
    Ok(Preview {
        entity,
        equipment,
        identity: sheet::identity(lib, &s),
        stats: vec![
            ("Hit Points", o.hp.max.to_string()),
            ("Armor Class", o.tiles[0].value.clone()),
            ("Initiative", o.tiles[1].value.clone()),
            ("Speed", o.tiles[2].value.clone()),
        ],
        abilities: o.abilities.into_iter().map(|a| (a.name, a.score, a.modifier)).collect(),
        warnings: s.build.warnings.clone(),
    })
}

/// Create the character and open it.
#[tauri::command]
pub async fn creation_finish(app: State<'_, App>) -> Res<Sheet> {
    let mut guard = app.lock();
    let inner = &mut *guard;
    let flow = inner.creation.as_mut().ok_or("No new character in progress")?;
    let character = flow.finish()?;
    let campaign = inner.ws.active_campaign().ok_or("No active campaign")?;
    let file = inner.ws.store.create_character(campaign, character).map_err(|e| e.to_string())?;
    inner.creation = None;
    open_slug(inner, &file.slug)?;
    sheet_of(inner)
}
