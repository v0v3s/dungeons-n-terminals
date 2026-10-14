//! Validate overlays against a library: the built-in ones plus any files
//! given, reporting formula errors and ids that match nothing.
//!
//! cargo run -p dnt-rules --example check_overlays -- /path/to/dnd-data [extra.toml …]

use dnt_data::Library;
use dnt_rules::builtin;
use dnt_rules::overlay::Overlay;

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = Library::load_data_dir(args.next().expect("usage: check_overlays <data dir> [files]").as_ref()).expect("load");
    let mut files: Vec<(String, String)> = builtin::FILES.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    files.extend(args.map(|p| (p.clone(), std::fs::read_to_string(&p).expect("read"))));
    let mut problems = 0;
    for (name, text) in &files {
        match Overlay::parse(text) {
            Err(e) => {
                println!("{name}: {e}");
                problems += 1;
            }
            Ok((overlay, diags)) => {
                for d in &diags {
                    println!("{name}: {d}");
                }
                for id in overlay.unresolved(&lib) {
                    println!("{name}: no such entity: {id}");
                }
                problems += diags.len() + overlay.unresolved(&lib).len();
                println!("{name}: {} features", overlay.features.len());
            }
        }
    }
    println!("{problems} problem(s)");
}
