//! Homebrew editing: cloning book entries into the campaign, new entries
//! from templates, and changing an entry field by field (dnt-core's
//! homebrew and entry_fields). The entry being edited is kept here, by
//! key, so it survives the library being rebuilt after each change.

use dnt_core::entry_fields::{self, Widget};
use dnt_core::homebrew::{self, Change, Doc};
use dnt_data::homebrew::{CloneMode, Template};
use dnt_data::{view, EntityId};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::{App, Inner, Res};

/// What can be done with an entry.
#[derive(Serialize)]
pub struct Actions {
    /// It is your own (campaign or global homebrew): it can be edited.
    edit: bool,
    /// It is from a book: it can be cloned into the campaign.
    clone: bool,
    /// A campaign version of a book entry: its original can be shown.
    original: bool,
    /// Why not, when nothing can be done (e.g. no campaign).
    why: String,
}

#[tauri::command]
pub async fn brew_actions(app: State<'_, App>, id: EntityId) -> Res<Actions> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let source = inner.ws.campaign_source();
    let original = lib.original(id).is_some();
    Ok(match homebrew::edit(lib, id, source.as_deref()) {
        Ok(_) => Actions { edit: true, clone: false, original, why: String::new() },
        Err(why) => Actions { edit: false, clone: source.is_some(), original, why },
    })
}

/// The book original of a campaign version, as an entry.
#[tauri::command]
pub async fn brew_original(app: State<'_, App>, id: EntityId) -> Res<crate::library::Entry> {
    let inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let data = lib.original(id).ok_or("This entry has no book original")?;
    let mut entry = crate::library::entry_of(&inner, id)?;
    entry.set_blocks(view::document_of(lib, id, data, &inner.ws.filter()));
    entry.set_source_note("book original");
    Ok(entry)
}

#[derive(Serialize)]
pub struct FieldRow {
    name: String,
    display: String,
    /// `text`, `number`, `bool`, `entries` (Markdown), `json` or `form`.
    widget: &'static str,
    /// What typing it looks like, for form fields.
    hint: String,
    /// Fixed choices, when the field has them.
    choices: Option<Vec<String>>,
    /// Several choices can be ticked (and custom ones added).
    multi: bool,
    /// Its name for people (empty: use `name`).
    label: String,
    /// What it is.
    help: String,
    /// The text to start editing from.
    text: String,
    /// Stored in the homebrew (changed from the book, or new).
    changed: bool,
    /// Not one of the kind's common fields.
    extra: bool,
}

/// The entry being edited, with its fields.
#[derive(Serialize)]
pub struct Editing {
    /// The entity, in the rebuilt library.
    id: EntityId,
    name: String,
    kind: &'static str,
    /// `campaign` or `global`.
    doc: &'static str,
    /// A campaign version of a book entry.
    replaces: bool,
    fields: Vec<FieldRow>,
    /// What the last change did.
    message: String,
}

fn editing_of(inner: &Inner, message: String) -> Res<Editing> {
    let entry = inner.editing.as_ref().ok_or("Nothing is being edited")?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let id = entry.entity(lib).ok_or("The entry is gone")?;
    let doc = inner.ws.homebrew_doc(entry.doc)?;
    let fields = entry_fields::fields(lib, entry, entry.entry(lib, &doc));
    Ok(Editing {
        id,
        name: lib.entity(id).name.clone(),
        kind: entry.kind.label(),
        doc: match entry.doc {
            Doc::Campaign => "campaign",
            Doc::Global => "global",
        },
        replaces: entry.replaces,
        fields: fields
            .into_iter()
            .map(|f| FieldRow {
                widget: match f.widget {
                    Widget::Text => "text",
                    Widget::Number => "number",
                    Widget::Bool => "bool",
                    Widget::Entries => "entries",
                    Widget::Json => "json",
                    Widget::Form(_) => "form",
                },
                hint: f.format().map_or("", dnt_data::forms::hint).to_string(),
                choices: f.choices().map(|c| c.into_iter().map(|(label, _)| label).collect()),
                multi: f.format().is_some_and(dnt_data::forms::multiple),
                label: f.label.clone(),
                help: f.help.clone(),
                text: f.text(),
                display: f.display.clone(),
                changed: f.changed,
                extra: f.extra,
                name: f.name,
            })
            .collect(),
        message,
    })
}

/// Apply a change to the entry being edited, save its document and
/// rebuild the library.
fn apply(inner: &mut Inner, change: Change) -> Res<String> {
    let mut entry = inner.editing.clone().ok_or("Nothing is being edited")?;
    let mut doc = inner.ws.homebrew_doc(entry.doc)?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let deleting = change == Change::Delete;
    let message = homebrew::change(lib, &mut doc, &mut entry, change)?;
    inner.ws.save_homebrew_doc(entry.doc, &doc)?;
    inner.editing = (!deleting).then_some(entry);
    inner.library_changed();
    Ok(message)
}

/// Start editing your own entry.
#[tauri::command]
pub async fn brew_open(app: State<'_, App>, id: EntityId) -> Res<Editing> {
    let mut inner = app.lock();
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let entry = homebrew::edit(lib, id, inner.ws.campaign_source().as_deref())?;
    inner.editing = Some(entry);
    editing_of(&inner, String::new())
}

/// Clone a book entry into the campaign: a version replacing it, or a
/// separate copy named `name`. Then edit it.
#[tauri::command]
pub async fn brew_clone(app: State<'_, App>, id: EntityId, copy: Option<String>) -> Res<Editing> {
    let mut inner = app.lock();
    let source = inner.ws.campaign_source().ok_or("Cloning needs an active campaign")?;
    let mut doc = inner.ws.homebrew_doc(Doc::Campaign)?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let mode = match copy {
        Some(name) if !name.trim().is_empty() => CloneMode::Copy { name: name.trim().to_string() },
        Some(_) => return Err("The copy needs a name".into()),
        None => CloneMode::Replace,
    };
    let (entry, message) = homebrew::clone(lib, &mut doc, id, &source, mode)?;
    inner.ws.save_homebrew_doc(Doc::Campaign, &doc)?;
    inner.editing = Some(entry);
    inner.library_changed();
    editing_of(&inner, message)
}

/// The templates for new entries: (index, label).
#[tauri::command]
pub async fn brew_templates() -> Vec<(usize, &'static str)> {
    Template::ALL.iter().enumerate().map(|(i, t)| (i, t.label())).collect()
}

/// A new campaign entry from a template, to edit.
#[tauri::command]
pub async fn brew_create(app: State<'_, App>, template: usize, name: String) -> Res<Editing> {
    let mut inner = app.lock();
    let source = inner.ws.campaign_source().ok_or("New entries need an active campaign")?;
    let template = *Template::ALL.get(template).ok_or("No such template")?;
    if name.trim().is_empty() {
        return Err("The entry needs a name".into());
    }
    let mut doc = inner.ws.homebrew_doc(Doc::Campaign)?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let (entry, message) = homebrew::create(lib, &mut doc, template, name.trim(), &source)?;
    inner.ws.save_homebrew_doc(Doc::Campaign, &doc)?;
    inner.editing = Some(entry);
    inner.library_changed();
    editing_of(&inner, message)
}

/// What to set a field to.
#[derive(serde::Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum SetTo {
    /// Typed text, read as the field reads it.
    Text(String),
    /// One of the field's choices, by index.
    Choice(usize),
    /// A yes/no field switched.
    Toggle,
    /// A field's value as JSON (a field the entry did not have).
    Json(String),
    /// Back to the book (campaign versions) or removed.
    Clear,
}

#[tauri::command]
pub async fn brew_set(app: State<'_, App>, field: String, to: SetTo) -> Res<Editing> {
    let mut inner = app.lock();
    let entry = inner.editing.clone().ok_or("Nothing is being edited")?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let doc = inner.ws.homebrew_doc(entry.doc)?;
    let fields = entry_fields::fields(lib, &entry, entry.entry(lib, &doc));
    let current = fields.iter().find(|f| f.name == field);
    let value = match (to, current) {
        (SetTo::Clear, _) => None,
        (SetTo::Json(text), _) => Some(entry_fields::parse_json(&text)?),
        (SetTo::Text(text), Some(f)) => f.parse(&text, lib)?,
        (SetTo::Text(text), None) => Some(Value::String(text)),
        (SetTo::Toggle, Some(f)) => Some(f.toggled()),
        (SetTo::Choice(i), Some(f)) => {
            let choices = f.choices().ok_or("This field has no choices")?;
            choices.into_iter().nth(i).ok_or("No such choice")?.1
        }
        (_, None) => return Err(format!("No field {field}")),
    };
    let message = apply(&mut inner, Change::Set { field, value })?;
    editing_of(&inner, message)
}

/// The whole entry as JSON, to edit in one go.
#[tauri::command]
pub async fn brew_json(app: State<'_, App>) -> Res<String> {
    let inner = app.lock();
    let entry = inner.editing.as_ref().ok_or("Nothing is being edited")?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let doc = inner.ws.homebrew_doc(entry.doc)?;
    let stored = entry.entry(lib, &doc).ok_or("The entry is not in the homebrew")?;
    Ok(entry_fields::json_text(Some(&Value::Object(stored.clone()))))
}

#[tauri::command]
pub async fn brew_replace(app: State<'_, App>, json: String) -> Res<Editing> {
    let mut inner = app.lock();
    let Value::Object(new) = entry_fields::parse_json(&json)? else {
        return Err("The entry must be a JSON object".into());
    };
    let message = apply(&mut inner, Change::Replace(new))?;
    editing_of(&inner, message)
}

#[tauri::command]
pub async fn brew_delete(app: State<'_, App>) -> Res<String> {
    let mut inner = app.lock();
    apply(&mut inner, Change::Delete)
}

/// Move a campaign entry to the global homebrew (every campaign), and
/// point the campaign's characters and notes at it.
#[tauri::command]
pub async fn brew_promote(app: State<'_, App>) -> Res<Editing> {
    let mut inner = app.lock();
    let mut entry = inner.editing.clone().ok_or("Nothing is being edited")?;
    let mut campaign = inner.ws.homebrew_doc(Doc::Campaign)?;
    let mut global = inner.ws.homebrew_doc(Doc::Global)?;
    let lib = inner.ws.lib.as_ref().ok_or("No data yet")?;
    let promoted = homebrew::promote(lib, &mut campaign, &mut global, &mut entry)?;
    // The global one first: a failure leaves the entry twice, not lost.
    inner.ws.store.save_global_homebrew(&global).map_err(|e| format!("Could not save the global homebrew: {e}"))?;
    inner.ws.save_homebrew_doc(Doc::Campaign, &campaign)?;
    inner.ws.reload();
    let c = inner.ws.active_campaign().ok_or("Homebrew needs an active campaign")?;
    let message = homebrew::rewrite_references(&inner.ws.store, c, inner.ws.notes().as_ref(), entry.kind, &promoted);
    inner.editing = Some(entry);
    inner.library_changed();
    editing_of(&inner, message)
}

#[tauri::command]
pub async fn brew_close(app: State<'_, App>) -> Res<()> {
    app.lock().editing = None;
    Ok(())
}
