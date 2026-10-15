use std::fmt;

/// The kinds of entities `dnt` knows how to ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Class,
    Subclass,
    ClassFeature,
    SubclassFeature,
    Spell,
    Race,
    Subrace,
    Background,
    Feat,
    OptionalFeature,
    Item,
    BaseItem,
    ItemGroup,
    MagicVariant,
    ItemMastery,
    Condition,
    Disease,
    Status,
    Language,
    Skill,
    Sense,
    Action,
    VariantRule,
}

impl Kind {
    pub const ALL: &'static [Kind] = &[
        Kind::Class,
        Kind::Subclass,
        Kind::ClassFeature,
        Kind::SubclassFeature,
        Kind::Spell,
        Kind::Race,
        Kind::Subrace,
        Kind::Background,
        Kind::Feat,
        Kind::OptionalFeature,
        Kind::Item,
        Kind::BaseItem,
        Kind::ItemGroup,
        Kind::MagicVariant,
        Kind::ItemMastery,
        Kind::Condition,
        Kind::Disease,
        Kind::Status,
        Kind::Language,
        Kind::Skill,
        Kind::Sense,
        Kind::Action,
        Kind::VariantRule,
    ];

    /// The top-level JSON key holding entities of this kind.
    pub fn json_key(self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Subclass => "subclass",
            Kind::ClassFeature => "classFeature",
            Kind::SubclassFeature => "subclassFeature",
            Kind::Spell => "spell",
            Kind::Race => "race",
            Kind::Subrace => "subrace",
            Kind::Background => "background",
            Kind::Feat => "feat",
            Kind::OptionalFeature => "optionalfeature",
            Kind::Item => "item",
            Kind::BaseItem => "baseitem",
            Kind::ItemGroup => "itemGroup",
            Kind::MagicVariant => "magicvariant",
            Kind::ItemMastery => "itemMastery",
            Kind::Condition => "condition",
            Kind::Disease => "disease",
            Kind::Status => "status",
            Kind::Language => "language",
            Kind::Skill => "skill",
            Kind::Sense => "sense",
            Kind::Action => "action",
            Kind::VariantRule => "variantrule",
        }
    }

    pub fn from_json_key(key: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.json_key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Class => "Class",
            Kind::Subclass => "Subclass",
            Kind::ClassFeature => "Class Feature",
            Kind::SubclassFeature => "Subclass Feature",
            Kind::Spell => "Spell",
            Kind::Race => "Species",
            Kind::Subrace => "Subspecies",
            Kind::Background => "Background",
            Kind::Feat => "Feat",
            Kind::OptionalFeature => "Option",
            Kind::Item => "Item",
            Kind::BaseItem => "Item",
            Kind::ItemGroup => "Item",
            Kind::MagicVariant => "Item",
            Kind::ItemMastery => "Mastery",
            Kind::Condition => "Condition",
            Kind::Disease => "Disease",
            Kind::Status => "Status",
            Kind::Language => "Language",
            Kind::Skill => "Skill",
            Kind::Sense => "Sense",
            Kind::Action => "Action",
            Kind::VariantRule => "Rule",
        }
    }

    /// Entity kinds an inline `{@tag ...}` can point to.
    pub fn for_tag(tag: &str) -> &'static [Kind] {
        match tag {
            "class" => &[Kind::Class],
            "subclass" => &[Kind::Subclass],
            "classFeature" => &[Kind::ClassFeature],
            "subclassFeature" => &[Kind::SubclassFeature],
            "spell" => &[Kind::Spell],
            "race" => &[Kind::Race, Kind::Subrace],
            "background" => &[Kind::Background],
            "feat" => &[Kind::Feat],
            "optfeature" => &[Kind::OptionalFeature],
            "item" => &[Kind::Item, Kind::BaseItem, Kind::ItemGroup, Kind::MagicVariant],
            "itemMastery" => &[Kind::ItemMastery],
            "condition" => &[Kind::Condition],
            "disease" => &[Kind::Disease],
            "status" => &[Kind::Status],
            "language" => &[Kind::Language],
            "skill" => &[Kind::Skill],
            "sense" => &[Kind::Sense],
            "action" => &[Kind::Action],
            "variantrule" => &[Kind::VariantRule],
            _ => &[],
        }
    }

    /// Source assumed when a tag omits it.
    pub fn default_tag_source(tag: &str) -> &'static str {
        match tag {
            "item" | "disease" | "variantrule" | "table" | "hazard" | "object" | "deck" | "card" => "DMG",
            "itemMastery" => "XPHB",
            "creature" => "MM",
            _ => "PHB",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
