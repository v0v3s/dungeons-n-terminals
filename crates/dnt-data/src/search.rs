//! Fuzzy name search over library entities.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::library::{EntityId, Library};

pub struct Searcher {
    matcher: Matcher,
    buf: Vec<char>,
}

impl Default for Searcher {
    fn default() -> Self {
        Searcher {
            matcher: Matcher::new(Config::DEFAULT),
            buf: Vec::new(),
        }
    }
}

impl Searcher {
    /// Rank `candidates` by how well their names match `query`. An empty
    /// query returns all candidates alphabetically.
    pub fn search(&mut self, lib: &Library, query: &str, candidates: &[EntityId]) -> Vec<EntityId> {
        let query = query.trim();
        if query.is_empty() {
            let mut all = candidates.to_vec();
            all.sort_by_cached_key(|&id| sort_key(lib, id));
            return all;
        }
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut scored: Vec<(u32, EntityId)> = candidates
            .iter()
            .filter_map(|&id| {
                let name = &lib.entity(id).name;
                let score = pattern.score(Utf32Str::new(name, &mut self.buf), &mut self.matcher)?;
                Some((score, id))
            })
            .collect();
        scored.sort_by(|(sa, a), (sb, b)| {
            sb.cmp(sa)
                .then_with(|| lib.entity(*a).name.len().cmp(&lib.entity(*b).name.len()))
                .then_with(|| sort_key(lib, *a).cmp(&sort_key(lib, *b)))
        });
        scored.into_iter().map(|(_, id)| id).collect()
    }
}

fn sort_key(lib: &Library, id: EntityId) -> (String, crate::Kind, String) {
    let e = lib.entity(id);
    (e.name.to_lowercase(), e.kind, e.source.clone())
}
