//! Validate overlays against a library (over the built-in SRD): the files
//! given, or the built-in ones without files, reporting formula errors
//! and ids that match nothing. Exits with 1 on any problem.
//!
//! cargo run -p dnt-rules --example check_overlays -- /path/to/data [extra.toml …]
//!
//! With `--parse-only` first, ids are not looked up (rules for homebrew
//! the data does not have).

use dnt_data::Library;
use dnt_rules::builtin;
use dnt_rules::overlay::Overlay;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let parse_only = args.next_if_eq("--parse-only").is_some();
    let lib = Library::load_dir_over_srd(args.next().expect("usage: check_overlays <data dir> [files]").as_ref()).expect("load");
    let mut files: Vec<(String, String)> = args.map(|p| (p.clone(), std::fs::read_to_string(&p).expect("read"))).collect();
    if files.is_empty() {
        files = builtin::FILES.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect();
    }
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
                let unresolved = if parse_only { Vec::new() } else { overlay.unresolved(&lib) };
                for id in &unresolved {
                    println!("{name}: no such entity: {id}");
                }
                problems += diags.len() + unresolved.len();
                println!("{name}: {} features", overlay.features.len());
            }
        }
    }
    println!("{problems} problem(s)");
    if problems > 0 {
        std::process::exit(1);
    }
}
