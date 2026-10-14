//! Load a data data directory and report counts, timing and link health.
//!
//! cargo run --release -p dnt-data --example stats -- /path/to/dnd-data

use std::collections::BTreeMap;
use std::time::Instant;

use dnt_data::{doc, view, Library};

fn main() {
    let path = std::env::args().nth(1).expect("usage: stats <data dir>");
    let start = Instant::now();
    let lib = Library::load_data_dir(path.as_ref()).expect("load failed");
    println!("loaded {} entities in {:?}", lib.entities().len(), start.elapsed());

    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for e in lib.entities() {
        *by_kind.entry(format!("{:?}", e.kind)).or_default() += 1;
    }
    for (k, n) in &by_kind {
        println!("  {k:<16} {n}");
    }
    println!("warnings: {}", lib.warnings.len());
    for w in lib.warnings.iter().take(10) {
        println!("  {w}");
    }

    let start = Instant::now();
    let (mut total, mut broken) = (0, BTreeMap::<String, Vec<String>>::new());
    for e in lib.entities() {
        let blocks = view::document(&lib, lib.get(&e.key).unwrap(), &Default::default());
        for link in doc::links(&blocks) {
            total += 1;
            if lib.resolve(link).is_none() {
                broken.entry(link.tag.clone()).or_default().push(link.args.clone());
            }
        }
    }
    let n_broken: usize = broken.values().map(Vec::len).sum();
    println!("rendered all entries in {:?}; links: {total}, unresolved: {n_broken}", start.elapsed());
    for (tag, args) in &broken {
        println!("  {tag:<16} {:>5}  e.g. {}", args.len(), args.iter().take(3).cloned().collect::<Vec<_>>().join(" ; "));
    }
}
