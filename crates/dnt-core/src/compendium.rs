//! Searching the compendium: entities narrowed to a category and the
//! campaign's sources, matched against a query with facets (`#kind`,
//! `*dmg`, `^lvl`, `@source`), with completions for the facet being typed.

use dnt_data::query::{Facet, FacetIndex, Query};
use dnt_data::search::Searcher;
use dnt_data::{EntityId, Kind, Library, SourceFilter};

/// Groups of kinds the result list can be narrowed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    All,
    Spells,
    Classes,
    Features,
    Species,
    Backgrounds,
    Feats,
    Items,
    Rules,
}

impl Category {
    pub const ORDER: [Category; 9] = [
        Category::All,
        Category::Spells,
        Category::Classes,
        Category::Features,
        Category::Species,
        Category::Backgrounds,
        Category::Feats,
        Category::Items,
        Category::Rules,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::All => "All",
            Category::Spells => "Spells",
            Category::Classes => "Classes",
            Category::Features => "Features",
            Category::Species => "Species",
            Category::Backgrounds => "Backgrounds",
            Category::Feats => "Feats",
            Category::Items => "Items",
            Category::Rules => "Rules",
        }
    }

    pub fn contains(self, kind: Kind) -> bool {
        use Kind::*;
        match self {
            Category::All => true,
            Category::Spells => kind == Spell,
            Category::Classes => matches!(kind, Class | Subclass),
            Category::Features => matches!(kind, ClassFeature | SubclassFeature | OptionalFeature),
            Category::Species => matches!(kind, Race | Subrace),
            Category::Backgrounds => kind == Background,
            Category::Feats => kind == Feat,
            Category::Items => matches!(kind, Item | BaseItem | ItemGroup | MagicVariant | ItemMastery),
            Category::Rules => matches!(kind, Condition | Disease | Status | Action | VariantRule | Sense | Skill | Language),
        }
    }

    /// What `#`, `*` and `^` mean on this tab, for the hint line.
    pub fn hints(self) -> &'static [(char, &'static str)] {
        match self {
            Category::All => &[('#', "kind"), ('*', "dmg"), ('^', "lvl")],
            Category::Spells => &[('#', "class·school"), ('*', "dmg"), ('^', "lvl")],
            Category::Classes => &[('#', "class·caster")],
            Category::Features => &[('#', "class·subclass"), ('^', "lvl")],
            Category::Species => &[('#', "creature type")],
            Category::Backgrounds => &[],
            Category::Feats => &[('#', "category")],
            Category::Items => &[('#', "type·rarity"), ('*', "dmg")],
            Category::Rules => &[('#', "kind")],
        }
    }

    pub fn cycle(self, delta: isize) -> Category {
        let n = Self::ORDER.len() as isize;
        let i = Self::ORDER.iter().position(|c| *c == self).unwrap() as isize;
        Self::ORDER[((i + delta).rem_euclid(n)) as usize]
    }
}

pub struct Search {
    pub filter: SourceFilter,
    pub category: Category,
    pub query: String,
    pub results: Vec<EntityId>,
    /// Entities per category under the current source filter.
    pub category_counts: [usize; Category::ORDER.len()],
    /// Completions for the facet term being typed, with counts.
    pub suggestions: Vec<(String, usize)>,
    visible: Vec<EntityId>,
    searcher: Searcher,
    facets: FacetIndex,
}

impl Search {
    pub fn new(lib: &Library, filter: SourceFilter) -> Search {
        let mut s = Search {
            filter,
            category: Category::All,
            query: String::new(),
            results: Vec::new(),
            category_counts: [0; Category::ORDER.len()],
            suggestions: Vec::new(),
            visible: Vec::new(),
            searcher: Searcher::default(),
            facets: FacetIndex::build(lib),
        };
        s.recount(lib);
        s.refilter(lib);
        s
    }

    pub fn set_filter(&mut self, lib: &Library, filter: SourceFilter) {
        self.filter = filter;
        self.recount(lib);
        self.refilter(lib);
    }

    pub fn set_category(&mut self, lib: &Library, category: Category) {
        self.category = category;
        self.refilter(lib);
    }

    pub fn set_query(&mut self, lib: &Library, query: impl Into<String>) {
        self.query = query.into();
        self.run(lib);
    }

    fn recount(&mut self, lib: &Library) {
        self.category_counts = [0; Category::ORDER.len()];
        for e in lib.entities().iter().filter(|e| self.filter.allows(e)) {
            for (i, cat) in Category::ORDER.iter().enumerate() {
                if cat.contains(e.kind) {
                    self.category_counts[i] += 1;
                }
            }
        }
    }

    /// Recompute the candidate set after a source or category change.
    fn refilter(&mut self, lib: &Library) {
        self.visible = (0..lib.entities().len())
            .filter(|&id| {
                let e = lib.entity(id);
                self.category.contains(e.kind) && self.filter.allows(e)
            })
            .collect();
        self.run(lib);
    }

    /// Search again with the current query.
    pub fn run(&mut self, lib: &Library) {
        let query = Query::parse(&self.query);
        let candidates = if query.terms.is_empty() { self.visible.clone() } else { self.facets.filter(lib, &query, &self.visible) };
        self.results = self.searcher.search(lib, &query.name, &candidates);
        self.suggestions = match self.partial_facet() {
            Some((facet, partial)) => {
                let mut s = self.facets.suggestions(facet, &partial, &self.visible);
                s.truncate(8);
                s
            }
            None => Vec::new(),
        };
    }

    /// The facet term currently being typed (the last token), if any.
    fn partial_facet(&self) -> Option<(Facet, String)> {
        if self.query.ends_with(char::is_whitespace) {
            return None;
        }
        let last = self.query.split_whitespace().last()?;
        let last = last.strip_prefix('!').unwrap_or(last);
        let mut chars = last.chars();
        let prefix = chars.next()?;
        let facet = Facet::ALL.into_iter().find(|f| f.prefix() == prefix && *f != Facet::Level)?;
        Some((facet, chars.as_str().to_string()))
    }

    /// Replace the facet term being typed with the first suggestion.
    pub fn complete(&mut self, lib: &Library) -> bool {
        let Some((value, _)) = self.suggestions.first() else { return false };
        let value = value.replace(' ', "_");
        let start = self.query.rfind(char::is_whitespace).map_or(0, |i| i + 1);
        let token = &self.query[start..];
        let keep = token.chars().take_while(|c| *c == '!').count() + 1;
        let head = token[..keep].to_string();
        self.query.truncate(start);
        self.query.push_str(&head);
        self.query.push_str(&value);
        self.query.push(' ');
        self.run(lib);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_narrows_and_completes() {
        let mut lib = Library::new();
        lib.load_json(&json!({
            "spell": [{"name": "Fire Bolt", "source": "T", "level": 0, "school": "V"},
                      {"name": "Fireball", "source": "T", "level": 3, "school": "V"},
                      {"name": "Sleep", "source": "T", "level": 1, "school": "E"}],
            "item": [{"name": "Fire Opal", "source": "T"}]
        }));
        let mut s = Search::new(&lib, SourceFilter::default());
        let count = |s: &Search, c: Category| s.category_counts[Category::ORDER.iter().position(|x| *x == c).unwrap()];
        assert_eq!((count(&s, Category::All), count(&s, Category::Spells), count(&s, Category::Items)), (4, 3, 1));

        s.set_query(&lib, "fire");
        assert_eq!(s.results.len(), 3);
        s.set_category(&lib, Category::Spells);
        let names: Vec<&str> = s.results.iter().map(|&e| lib.entity(e).name.as_str()).collect();
        assert_eq!(names.len(), 2, "{names:?}");

        s.set_query(&lib, "^");
        assert!(s.suggestions.is_empty(), "levels are not completed");
        s.set_query(&lib, "#evo");
        assert!(!s.suggestions.is_empty(), "a school to complete");
        assert!(s.complete(&lib));
        assert!(s.query.ends_with(' ') && s.query.starts_with('#'), "{:?}", s.query);
        assert_eq!(s.results.len(), 2, "both fire spells are evocations");
    }
}
