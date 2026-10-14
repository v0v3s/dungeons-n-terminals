//! Structured search queries and the facet index they run against.
//!
//! Syntax (terms are ANDed; `_` stands for a space inside a term):
//!
//! | Term       | Meaning                                                  |
//! |------------|----------------------------------------------------------|
//! | `word`     | fuzzy match on the name                                  |
//! | `@xphb`    | source                                                   |
//! | `#wizard`  | tag: class, school, item type, feat category, kind, …    |
//! | `*fire`    | damage type                                              |
//! | `^3`, `^1-3` | level (spell level, feature level)                     |
//! | `"text"`   | full-text search in the entry body                       |
//! | `!term`    | negates any facet or text term                           |

use std::collections::BTreeMap;

use serde_json::Value;

use crate::doc;
use crate::kind::Kind;
use crate::library::{EntityId, Library};
use crate::view;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Facet {
    Source,
    Tag,
    Damage,
    Level,
}

impl Facet {
    pub const ALL: [Facet; 4] = [Facet::Source, Facet::Tag, Facet::Damage, Facet::Level];

    pub fn prefix(self) -> char {
        match self {
            Facet::Source => '@',
            Facet::Tag => '#',
            Facet::Damage => '*',
            Facet::Level => '^',
        }
    }

    fn from_prefix(c: char) -> Option<Facet> {
        Facet::ALL.into_iter().find(|f| f.prefix() == c)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermKind {
    Facet(Facet),
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub kind: TermKind,
    /// Lowercase value with `_` replaced by spaces.
    pub value: String,
    pub negate: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// The plain words, matched fuzzily against names.
    pub name: String,
    pub terms: Vec<Term>,
}

impl Query {
    pub fn parse(input: &str) -> Query {
        let mut q = Query::default();
        let mut name_words = Vec::new();
        for token in tokenize(input) {
            let (negate, body) = match token.strip_prefix('!') {
                Some(rest) if !rest.is_empty() => (true, rest),
                _ => (false, token.as_str()),
            };
            let mut chars = body.chars();
            let first = chars.next();
            if body.starts_with('"') {
                let text = body.trim_matches('"').to_lowercase();
                if !text.is_empty() {
                    q.terms.push(Term {
                        kind: TermKind::Text,
                        value: text,
                        negate,
                    });
                }
            } else if let Some(facet) = first.and_then(Facet::from_prefix) {
                let value = chars.as_str().to_lowercase().replace('_', " ");
                // A bare prefix is still being typed; ignore it.
                if !value.is_empty() {
                    q.terms.push(Term {
                        kind: TermKind::Facet(facet),
                        value,
                        negate,
                    });
                }
            } else {
                name_words.push(token);
            }
        }
        q.name = name_words.join(" ");
        q
    }
}

/// Split on whitespace, keeping `"quoted phrases"` (optionally `!"…"`) whole.
fn tokenize(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in input.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.push(c);
            }
            c if c.is_whitespace() && !in_quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Per-entity facet values, computed once per library.
pub struct FacetIndex {
    values: Vec<Vec<(Facet, String)>>,
    text: Option<Vec<String>>,
}

impl FacetIndex {
    pub fn build(lib: &Library) -> FacetIndex {
        FacetIndex {
            values: (0..lib.entities().len()).map(|id| entity_facets(lib, id)).collect(),
            text: None,
        }
    }

    pub fn values(&self, id: EntityId) -> &[(Facet, String)] {
        &self.values[id]
    }

    /// Filter `candidates` by the query's facet and text terms.
    pub fn filter(&mut self, lib: &Library, query: &Query, candidates: &[EntityId]) -> Vec<EntityId> {
        if query.terms.iter().any(|t| t.kind == TermKind::Text) && self.text.is_none() {
            self.text = Some((0..lib.entities().len()).map(|id| plain_text(lib, id)).collect());
        }
        candidates
            .iter()
            .copied()
            .filter(|&id| query.terms.iter().all(|t| self.term_matches(id, t) != t.negate))
            .collect()
    }

    fn term_matches(&self, id: EntityId, term: &Term) -> bool {
        match term.kind {
            TermKind::Text => self.text.as_ref().is_some_and(|t| t[id].contains(&term.value)),
            TermKind::Facet(Facet::Level) => {
                let (lo, hi) = parse_range(&term.value);
                self.values[id]
                    .iter()
                    .filter(|(f, _)| *f == Facet::Level)
                    .filter_map(|(_, v)| v.parse::<u32>().ok())
                    .any(|l| lo <= l && l <= hi)
            }
            TermKind::Facet(facet) => self.values[id]
                .iter()
                .any(|(f, v)| *f == facet && value_matches(v, &term.value)),
        }
    }

    /// Facet values occurring among `candidates`, with counts, best first.
    /// Used for completion suggestions.
    pub fn suggestions(&self, facet: Facet, partial: &str, candidates: &[EntityId]) -> Vec<(String, usize)> {
        let partial = partial.to_lowercase().replace('_', " ");
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for &id in candidates {
            for (f, v) in &self.values[id] {
                if *f == facet && value_matches(v, &partial) {
                    *counts.entry(v).or_default() += 1;
                }
            }
        }
        let mut out: Vec<(String, usize)> = counts.into_iter().map(|(v, n)| (v.to_string(), n)).collect();
        // Values that start with the partial text first, then by frequency.
        out.sort_by(|(a, na), (b, nb)| {
            (!a.starts_with(&partial))
                .cmp(&!b.starts_with(&partial))
                .then(nb.cmp(na))
                .then(a.cmp(b))
        });
        out
    }
}

/// A value matches if it, or any of its words, starts with the term.
fn value_matches(value: &str, term: &str) -> bool {
    value.starts_with(term) || value.split([' ', '-', '(']).any(|w| w.starts_with(term))
}

fn parse_range(s: &str) -> (u32, u32) {
    match s.split_once('-') {
        Some((a, b)) => (a.trim().parse().unwrap_or(0), b.trim().parse().unwrap_or(u32::MAX)),
        None => {
            let n = s.trim().parse().unwrap_or(u32::MAX);
            (n, n)
        }
    }
}

fn plain_text(lib: &Library, id: EntityId) -> String {
    let d = &lib.entity(id).data;
    let mut blocks = Vec::new();
    for field in ["entries", "entriesHigherLevel"] {
        if let Some(v) = d.get(field) {
            blocks.extend(doc::from_entries(v));
        }
    }
    if let Some(v) = d.get("inherits").and_then(|i| i.get("entries")) {
        blocks.extend(doc::from_entries(v));
    }
    doc::plain_text(&blocks).to_lowercase()
}

fn entity_facets(lib: &Library, id: EntityId) -> Vec<(Facet, String)> {
    let e = lib.entity(id);
    let d = &e.data;
    let mut out = vec![
        (Facet::Source, e.source.to_lowercase()),
        (Facet::Tag, e.kind.label().to_lowercase()),
    ];
    let mut tag = |v: &str| out.push((Facet::Tag, v.to_lowercase()));
    let s = |k: &str| d.get(k).and_then(Value::as_str);

    match e.kind {
        Kind::Spell => {
            for c in lib.spell_classes(id) {
                tag(&c.name);
            }
            tag(view::spell_school(s("school").unwrap_or("")));
            let meta = d.get("meta");
            if meta.and_then(|m| m.get("ritual")).and_then(Value::as_bool) == Some(true) {
                tag("ritual");
            }
            let concentration = d
                .get("duration")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|x| x.get("concentration").and_then(Value::as_bool) == Some(true));
            if concentration {
                tag("concentration");
            }
            if d.get("level").and_then(Value::as_u64) == Some(0) {
                tag("cantrip");
            }
        }
        Kind::ClassFeature | Kind::Subclass => {
            if let Some(c) = s("className") {
                tag(c);
            }
        }
        Kind::SubclassFeature => {
            for k in ["className", "subclassShortName"] {
                if let Some(v) = s(k) {
                    tag(v);
                }
            }
        }
        Kind::Class => {
            if let Some(p) = s("casterProgression") {
                tag(match p {
                    "full" => "full caster",
                    "1/2" | "artificer" => "half caster",
                    "1/3" => "third caster",
                    "pact" => "pact caster",
                    other => other,
                });
            }
        }
        Kind::OptionalFeature => {
            for t in d.get("featureType").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                tag(view::feature_type(t));
            }
        }
        Kind::Feat => {
            if let Some(c) = s("category") {
                tag(view::feat_category(c));
            }
        }
        Kind::Item | Kind::BaseItem | Kind::ItemGroup | Kind::MagicVariant => {
            let d = d.get("inherits").unwrap_or(d);
            let s = |k: &str| d.get(k).and_then(Value::as_str);
            if let Some(t) = s("type") {
                tag(view::item_type(t.split('|').next().unwrap_or(t)));
            }
            if let Some(r) = s("rarity").filter(|r| !matches!(*r, "none" | "unknown")) {
                tag(r);
            }
            if d.get("wondrous").and_then(Value::as_bool) == Some(true) {
                tag("wondrous");
            }
            if d.get("reqAttune").is_some_and(|v| v.as_bool() != Some(false)) {
                tag("attunement");
            }
            if let Some(w) = s("weaponCategory") {
                tag(&format!("{w} weapon"));
            }
        }
        Kind::Race | Kind::Subrace => {
            for t in d.get("creatureTypes").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                tag(t);
            }
        }
        _ => {}
    }

    // Damage types.
    for t in d.get("damageInflict").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        out.push((Facet::Damage, t.to_lowercase()));
    }
    let item = d.get("inherits").unwrap_or(d);
    if let Some(code) = item.get("dmgType").and_then(Value::as_str) {
        out.push((Facet::Damage, view::damage_type(code).to_string()));
    }

    // Levels.
    if let Some(l) = d.get("level").and_then(Value::as_u64) {
        out.push((Facet::Level, l.to_string()));
    }

    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_terms() {
        let q = Query::parse(r#"fire #wiz !@phb *cold ^1-3 "save" ball"#);
        assert_eq!(q.name, "fire ball");
        assert_eq!(q.terms.len(), 5);
        assert_eq!(q.terms[1].kind, TermKind::Facet(Facet::Source));
        assert!(q.terms[1].negate);
        assert_eq!(q.terms[4].kind, TermKind::Text);
        assert_eq!(q.terms[4].value, "save");
    }

    #[test]
    fn quoted_phrases_and_underscores() {
        let q = Query::parse(r#""bonus action" #fighting_style"#);
        assert_eq!(q.terms[0].value, "bonus action");
        assert_eq!(q.terms[1].value, "fighting style");
    }

    #[test]
    fn bare_prefix_is_ignored() {
        assert!(Query::parse("#").terms.is_empty());
    }

    #[test]
    fn word_prefix_matching() {
        assert!(value_matches("eldritch invocation", "invo"));
        assert!(value_matches("wizard", "wiz"));
        assert!(!value_matches("wizard", "iz"));
        assert_eq!(parse_range("2-"), (2, u32::MAX));
        assert_eq!(parse_range("3"), (3, 3));
    }
}
