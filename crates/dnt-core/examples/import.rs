//! Import a zip into a store and compare the result with loading a data
//! folder: `cargo run --release -p dnt-core --example import -- data.zip /tmp/store [data dir]`.

use std::time::Instant;

use dnt_core::import;
use dnt_data::Library;
use dnt_store::Store;

fn main() {
    let mut args = std::env::args().skip(1);
    let (zip, dir) = (args.next().expect("usage: import <zip> <store dir> [data dir]"), args.next().expect("store dir"));
    let store = Store::open(format!("{dir}/data"), format!("{dir}/config.toml"));
    let t = Instant::now();
    let report = import::import_zip(&store, std::fs::read(&zip).expect("read zip")).expect("import");
    println!("{} in {:?}", report.summary(), t.elapsed());
    for (file, why) in report.skipped.iter().take(5) {
        println!("  skipped {file}: {why}");
    }
    let t = Instant::now();
    let lib = import::load(&store).expect("load");
    println!("loaded {} entities in {:?}", lib.entities().len(), t.elapsed());
    if let Some(data) = args.next() {
        let folder = Library::load_dir(data.as_ref()).expect("load folder");
        println!("folder has {} entities", folder.entities().len());
        let differ: Vec<String> = folder
            .entities()
            .iter()
            .filter(|e| lib.get(&e.key).is_none_or(|i| lib.entity(i).data != e.data))
            .map(|e| format!("{:?} {} ({})", e.kind, e.name, e.source))
            .collect();
        println!("{} entities differ{}", differ.len(), differ.first().map(|d| format!(", e.g. {d}")).unwrap_or_default());
        let (a, b) = (by_kind(&lib), by_kind(&folder));
        for (k, n) in &b {
            if a.get(k) != Some(n) {
                println!("  {k}: imported {:?}, folder {n}", a.get(k));
            }
        }
    }
}

#[allow(dead_code)]
fn by_kind(lib: &Library) -> std::collections::BTreeMap<String, usize> {
    let mut m = std::collections::BTreeMap::new();
    for e in lib.entities() {
        *m.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
    }
    m
}
