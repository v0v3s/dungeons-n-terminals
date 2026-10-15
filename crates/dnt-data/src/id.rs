//! Entity identity, following the data's pipe-delimited reference conventions.
//!
//! Lookup keys are lowercase and fully qualified (defaults filled in), so
//! that a tag like `{@classFeature Rage|Barbarian||1}` and the feature's own
//! JSON produce the same key.

use serde_json::Value;

use crate::kind::Kind;

/// A fully-qualified, case-insensitive lookup key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    pub kind: Kind,
    pub path: String,
}

impl Key {
    pub fn new(kind: Kind, parts: &[&str]) -> Key {
        Key { kind, path: parts.join("|").to_lowercase() }
    }

    /// Build the key for an entity from its JSON object.
    pub fn of_entity(kind: Kind, obj: &Value) -> Option<Key> {
        let s = |k: &str| obj.get(k).and_then(Value::as_str);
        let num = |k: &str| obj.get(k).and_then(Value::as_u64).map(|n| n.to_string());
        let name = display_name(kind, obj)?;
        let source = entity_source(obj)?;
        Some(match kind {
            Kind::ClassFeature => Key::new(kind, &[&name, s("className")?, s("classSource")?, &num("level")?, source]),
            Kind::Subclass => Key::new(kind, &[s("shortName")?, s("className")?, s("classSource")?, source]),
            Kind::SubclassFeature => Key::new(
                kind,
                &[&name, s("className")?, s("classSource")?, s("subclassShortName")?, s("subclassSource")?, &num("level")?, source],
            ),
            _ => Key::new(kind, &[&name, source]),
        })
    }

    /// Build a key from the pipe-separated parts of a reference (an inline
    /// tag's arguments, or a `classFeatures` entry), filling data defaults.
    /// Returns the key and the index of the part holding display text, if any.
    pub fn from_ref(kind: Kind, tag: &str, parts: &[&str]) -> Option<(Key, Option<usize>)> {
        let part = |i: usize| parts.get(i).copied().map(str::trim).filter(|p| !p.is_empty());
        let name = part(0)?;
        let default_src = Kind::default_tag_source(tag);
        Some(match kind {
            Kind::ClassFeature => {
                // name|className|classSource|level|source|display
                let class_source = part(2).unwrap_or("PHB");
                let source = part(4).unwrap_or(class_source);
                (Key::new(kind, &[name, part(1)?, class_source, part(3)?, source]), Some(5))
            }
            Kind::Subclass => {
                // shortName|className|classSource|source|display
                let class_source = part(2).unwrap_or("PHB");
                let source = part(3).unwrap_or("PHB");
                (Key::new(kind, &[name, part(1)?, class_source, source]), Some(4))
            }
            Kind::SubclassFeature => {
                // name|className|classSource|subclassShortName|subclassSource|level|source|display
                let class_source = part(2).unwrap_or("PHB");
                let sub_source = part(4).unwrap_or("PHB");
                let source = part(6).unwrap_or(sub_source);
                (Key::new(kind, &[name, part(1)?, class_source, part(3)?, sub_source, part(5)?, source]), Some(7))
            }
            _ => (Key::new(kind, &[name, part(1).unwrap_or(default_src)]), Some(2)),
        })
    }
}

/// An entity's source. Magic variants keep theirs under `inherits`.
pub fn entity_source(obj: &Value) -> Option<&str> {
    obj.get("source").or_else(|| obj.get("inherits").and_then(|i| i.get("source"))).and_then(Value::as_str)
}

/// The human-facing name of an entity. Subraces are shown as
/// `Race (Subrace)`, matching how data tags refer to them.
pub fn display_name(kind: Kind, obj: &Value) -> Option<String> {
    let name = obj.get("name").and_then(Value::as_str)?;
    if kind == Kind::Subrace {
        let race = obj.get("raceName").and_then(Value::as_str)?;
        return Some(format!("{race} ({name})"));
    }
    Some(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn feature_json_and_ref_agree() {
        let feature = json!({"name": "Rage", "className": "Barbarian", "classSource": "PHB", "level": 1, "source": "PHB"});
        let from_json = Key::of_entity(Kind::ClassFeature, &feature).unwrap();
        let (from_ref, _) = Key::from_ref(Kind::ClassFeature, "classFeature", &["Rage", "Barbarian", "", "1"]).unwrap();
        assert_eq!(from_json, from_ref);
    }

    #[test]
    fn subclass_feature_defaults() {
        let (key, display) =
            Key::from_ref(Kind::SubclassFeature, "subclassFeature", &["School of Abjuration", "Wizard", "", "Abjuration", "", "2"])
                .unwrap();
        assert_eq!(key.path, "school of abjuration|wizard|phb|abjuration|phb|2|phb");
        assert_eq!(display, Some(7));
    }

    #[test]
    fn simple_ref_uses_tag_default_source() {
        let (spell, _) = Key::from_ref(Kind::Spell, "spell", &["Fireball"]).unwrap();
        assert_eq!(spell.path, "fireball|phb");
        let (item, _) = Key::from_ref(Kind::Item, "item", &["Bag of Holding"]).unwrap();
        assert_eq!(item.path, "bag of holding|dmg");
    }
}
