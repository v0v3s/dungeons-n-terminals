//! Check a data directory (or single data files) against the schemas in
//! `schemas/`, the files dnt reads and in the shapes it understands.
//!
//! cargo run --release -p dnt-data --example validate -- /path/to/data [more.json …]

use std::path::{Path, PathBuf};

use serde_json::Value;

fn schema(name: &str) -> jsonschema::Validator {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas").join(name);
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("schema")).expect("schema JSON");
    jsonschema::validator_for(&schema).expect("valid schema")
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    assert!(!args.is_empty(), "usage: validate <data dir | file.json> …");
    let data_file = schema("data-file.schema.json");
    let books = schema("books.schema.json");
    let index = schema("index.schema.json");
    let spell_classes = schema("spell-classes.schema.json");

    // (file, schema) pairs: a data directory's files, or files given directly.
    let mut files: Vec<(PathBuf, &jsonschema::Validator)> = Vec::new();
    for arg in &args {
        if arg.is_file() {
            files.push((arg.clone(), &data_file));
            continue;
        }
        let dir = if arg.join("data/books.json").is_file() { arg.join("data") } else { arg.clone() };
        files.push((dir.join("books.json"), &books));
        if dir.join("adventures.json").is_file() {
            files.push((dir.join("adventures.json"), &books));
        }
        for f in [
            "races.json",
            "backgrounds.json",
            "feats.json",
            "items.json",
            "items-base.json",
            "magicvariants.json",
            "optionalfeatures.json",
            "conditionsdiseases.json",
            "languages.json",
            "skills.json",
            "senses.json",
            "actions.json",
            "variantrules.json",
        ] {
            files.push((dir.join(f), &data_file));
        }
        for sub in ["class", "spells"] {
            let idx = dir.join(sub).join("index.json");
            files.push((idx.clone(), &index));
            if let Ok(Value::Object(m)) = std::fs::read_to_string(&idx).map(|t| serde_json::from_str(&t).unwrap_or(Value::Null)) {
                files.extend(m.values().filter_map(Value::as_str).map(|f| (dir.join(sub).join(f), &data_file)));
            }
        }
        if dir.join("spells/sources.json").is_file() {
            files.push((dir.join("spells/sources.json"), &spell_classes));
        }
    }

    let (mut checked, mut bad) = (0, 0);
    for (path, validator) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("missing  {}", path.display());
            continue;
        };
        let doc: Value = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                println!("not JSON {}: {e}", path.display());
                bad += 1;
                continue;
            }
        };
        checked += 1;
        let errors: Vec<String> = validator.iter_errors(&doc).take(5).map(|e| format!("    {} at {}", e, e.instance_path())).collect();
        if !errors.is_empty() {
            bad += 1;
            println!("invalid  {}\n{}", path.display(), errors.join("\n"));
        }
    }
    println!("{checked} files checked, {bad} with problems");
    std::process::exit(if bad > 0 { 1 } else { 0 });
}
