//! A character in play: the file, its build and computed numbers, the
//! rules it follows, its pending choices and its dice.

use dnt_data::{EntityId, Library, SourceFilter};
use dnt_rules::builder::{build, Build};
use dnt_rules::character::{Character, InventoryItem};
use dnt_rules::choices::{self, Pending};
use dnt_rules::equipment;
use dnt_rules::overlay::OverlaySet;
use dnt_rules::play::Edition;
use dnt_rules::Evaluation;
use dnt_store::CharacterFile;

use crate::rolls::Roller;

pub struct CharacterSession {
    pub file: CharacterFile,
    /// Class feature choices this character is missing.
    pub pending: Vec<Pending>,
    /// The campaign's sources (for choices and pickers).
    pub filter: SourceFilter,
    pub build: Build,
    pub ev: Evaluation,
    pub edition: Edition,
    /// Dice, the next roll's mode and the roll log.
    pub roller: Roller,
}

impl CharacterSession {
    pub fn new(mut file: CharacterFile, lib: &Library, overlays: &OverlaySet, filter: &SourceFilter) -> CharacterSession {
        // Older files: equipped weapons go in hand, worn items get a slot
        // (saved with the next change).
        equipment::normalize(lib, &mut file.character);
        let build = build(&file.character, lib, overlays);
        let ev = build.evaluate();
        let edition = Edition::of(&build, lib);
        CharacterSession {
            pending: choices::pending(lib, &file.character, filter),
            filter: filter.clone(),
            file,
            build,
            ev,
            edition,
            roller: Roller::default(),
        }
    }

    /// Recompute everything after the character changed.
    pub fn rebuild(&mut self, lib: &Library, overlays: &OverlaySet) {
        self.build = build(&self.file.character, lib, overlays);
        self.ev = self.build.evaluate();
        self.pending = choices::pending(lib, &self.file.character, &self.filter);
    }

    pub fn character(&self) -> &Character {
        &self.file.character
    }

    /// Path of this character's note (`notes/Party/<name>.md`).
    pub fn note_path(&self) -> Option<std::path::PathBuf> {
        let campaign = self.file.path.parent()?.parent()?;
        Some(campaign.join("notes/Party").join(format!("{}.md", dnt_store::notes::file_safe(&self.file.character.name))))
    }
}

/// The library entity an inventory line refers to.
pub fn item_entity(lib: &Library, line: &InventoryItem) -> Option<EntityId> {
    dnt_rules::overlay::resolve_id(lib, line.item.as_deref()?)
}

/// What an inventory line is called: its own name, else the item's.
pub fn line_name(lib: &Library, line: &InventoryItem) -> String {
    line.name
        .clone()
        .or_else(|| item_entity(lib, line).map(|e| lib.entity(e).name.clone()))
        .or_else(|| line.item.clone())
        .unwrap_or_else(|| "?".into())
}
