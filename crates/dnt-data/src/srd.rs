//! The System Reference Document 5.2.1, built in: the data in
//! `data/srd` (made by `scripts/srd`), loaded under any other data.
//!
//! This work includes material from the System Reference Document 5.2.1
//! ("SRD 5.2.1") by Wizards of the Coast LLC, available at
//! https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the
//! Creative Commons Attribution 4.0 International License, available at
//! https://creativecommons.org/licenses/by/4.0/legalcode.

use crate::source::MemorySource;

/// The SRD's source id.
pub const SOURCE: &str = "SRD52";

/// The attribution the SRD's licence asks for, wherever it is shown.
pub const ATTRIBUTION: &str = "This work includes material from the System Reference Document 5.2.1 (\"SRD 5.2.1\") by Wizards of the Coast LLC, available at https://www.dndbeyond.com/srd. The SRD 5.2.1 is licensed under the Creative Commons Attribution 4.0 International License, available at https://creativecommons.org/licenses/by/4.0/legalcode.";

const FILES: &[(&str, &str)] = &[
    ("books.json", include_str!("../../../data/srd/books.json")),
    ("actions.json", include_str!("../../../data/srd/actions.json")),
    ("backgrounds.json", include_str!("../../../data/srd/backgrounds.json")),
    ("conditionsdiseases.json", include_str!("../../../data/srd/conditionsdiseases.json")),
    ("feats.json", include_str!("../../../data/srd/feats.json")),
    ("items-base.json", include_str!("../../../data/srd/items-base.json")),
    ("items.json", include_str!("../../../data/srd/items.json")),
    ("languages.json", include_str!("../../../data/srd/languages.json")),
    ("magicvariants.json", include_str!("../../../data/srd/magicvariants.json")),
    ("optionalfeatures.json", include_str!("../../../data/srd/optionalfeatures.json")),
    ("races.json", include_str!("../../../data/srd/races.json")),
    ("senses.json", include_str!("../../../data/srd/senses.json")),
    ("skills.json", include_str!("../../../data/srd/skills.json")),
    ("variantrules.json", include_str!("../../../data/srd/variantrules.json")),
    ("class/index.json", include_str!("../../../data/srd/class/index.json")),
    ("class/class-barbarian.json", include_str!("../../../data/srd/class/class-barbarian.json")),
    ("class/class-bard.json", include_str!("../../../data/srd/class/class-bard.json")),
    ("class/class-cleric.json", include_str!("../../../data/srd/class/class-cleric.json")),
    ("class/class-druid.json", include_str!("../../../data/srd/class/class-druid.json")),
    ("class/class-fighter.json", include_str!("../../../data/srd/class/class-fighter.json")),
    ("class/class-monk.json", include_str!("../../../data/srd/class/class-monk.json")),
    ("class/class-paladin.json", include_str!("../../../data/srd/class/class-paladin.json")),
    ("class/class-ranger.json", include_str!("../../../data/srd/class/class-ranger.json")),
    ("class/class-rogue.json", include_str!("../../../data/srd/class/class-rogue.json")),
    ("class/class-sorcerer.json", include_str!("../../../data/srd/class/class-sorcerer.json")),
    ("class/class-warlock.json", include_str!("../../../data/srd/class/class-warlock.json")),
    ("class/class-wizard.json", include_str!("../../../data/srd/class/class-wizard.json")),
    ("spells/index.json", include_str!("../../../data/srd/spells/index.json")),
    ("spells/sources.json", include_str!("../../../data/srd/spells/sources.json")),
    ("spells/spells-srd52.json", include_str!("../../../data/srd/spells/spells-srd52.json")),
];

/// The SRD's files.
pub fn files() -> MemorySource {
    let mut src = MemorySource::default();
    for (path, text) in FILES {
        src.insert(*path, text.as_bytes());
    }
    src
}

#[cfg(test)]
mod tests {
    use crate::{Kind, Library};

    #[test]
    fn the_srd_loads_whole() {
        let lib = Library::srd();
        assert!(lib.warnings.is_empty(), "{:?}", lib.warnings);
        let count = |k: Kind| lib.entities().iter().filter(|e| e.kind == k).count();
        assert_eq!((count(Kind::Class), count(Kind::Spell), count(Kind::Background)), (12, 339, 4));
    }
}
