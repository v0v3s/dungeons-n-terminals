//! The scenario rule, checked: every scenario of `e2e/scenarios.toml` has a
//! test on every target it lists, every frontend is decided (a target, or
//! a reason it does not have the feature), and every test names a scenario
//! of the catalog. See `e2e/README.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const FRONTENDS: [&str; 2] = ["tui", "app"];
const TARGETS: [&str; 3] = ["tui", "app", "android"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files(&path, ext, out);
        } else if path.to_string_lossy().ends_with(ext) {
            out.push(path);
        }
    }
}

/// A scenario id: `area.what`, lowercase, digits and dashes.
fn is_id(s: &str) -> bool {
    s.contains('.') && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

/// The string literals right after any of `calls` (`Tui::scenario("x"`).
fn ids_after(text: &str, calls: &[&str]) -> Vec<String> {
    let mut ids = Vec::new();
    for call in calls {
        let mut rest = text;
        while let Some(i) = rest.find(call) {
            rest = &rest[i + call.len()..];
            let arg = rest.trim_start();
            if let Some(arg) = arg.strip_prefix('"') {
                if let Some(end) = arg.find('"') {
                    ids.push(arg[..end].to_string());
                }
            }
        }
    }
    ids
}

/// The terminal tests' scenarios: `Tui::scenario("id"`, `Tui::with_args("id"`,
/// `Tui::with_editor("id"`, `run("id"`.
fn tui_tests() -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut paths = Vec::new();
    files(&root().join("crates/dnt/tests/e2e"), ".rs", &mut paths);
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        for id in ids_after(&text, &["Tui::scenario(", "Tui::with_args(", "Tui::with_editor(", "run("]) {
            found.entry(id).or_default().push(path.display().to_string());
        }
    }
    found
}

/// The app tests' scenarios: tags `"@id"` in `app/e2e/*.spec.ts`.
fn app_tests() -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut paths = Vec::new();
    files(&root().join("app/e2e"), ".spec.ts", &mut paths);
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap();
        for part in text.split("\"@").skip(1) {
            let id: String = part.chars().take_while(|c| *c != '"').collect();
            if id.contains('.') {
                found.entry(id).or_default().push(path.display().to_string());
            }
        }
    }
    found
}

#[test]
fn every_scenario_is_tested_on_its_targets() {
    let text = std::fs::read_to_string(root().join("e2e/scenarios.toml")).expect("e2e/scenarios.toml");
    let catalog: toml::Table = toml::from_str(&text).expect("e2e/scenarios.toml is TOML");
    let scenarios = catalog["scenario"].as_array().expect("[[scenario]] entries");

    let tui = tui_tests();
    let app = app_tests();
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    let (mut on_tui, mut on_app) = (0, 0);

    for s in scenarios {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        if !is_id(id) {
            problems.push(format!("{id:?}: an id is area.what, lowercase"));
        }
        if !ids.insert(id.to_string()) {
            problems.push(format!("{id}: listed twice"));
        }
        if s.get("does").and_then(|v| v.as_str()).is_none_or(str::is_empty) {
            problems.push(format!("{id}: say what the user does (does = \"…\")"));
        }
        let targets: Vec<&str> =
            s.get("targets").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|t| t.as_str()).collect()).unwrap_or_default();
        let not = s.get("not").and_then(|v| v.as_table());
        for t in &targets {
            if !TARGETS.contains(t) {
                problems.push(format!("{id}: unknown target {t:?} (one of {TARGETS:?})"));
            }
        }
        for f in FRONTENDS {
            let listed = targets.contains(&f) || (f == "app" && targets.contains(&"android"));
            let excused = not.and_then(|n| n.get(f)).and_then(|v| v.as_str()).is_some_and(|r| !r.trim().is_empty());
            match (listed, excused) {
                (false, false) => {
                    problems.push(format!("{id}: decide the {f}: add it to targets, or say why not (not = {{ {f} = \"…\" }})"))
                }
                (true, true) => problems.push(format!("{id}: the {f} is a target and excused at once")),
                _ => {}
            }
        }
        if targets.contains(&"tui") {
            if tui.contains_key(id) {
                on_tui += 1;
            } else {
                problems.push(format!("{id}: no terminal test (crates/dnt/tests/e2e: Tui::scenario(\"{id}\", …))"));
            }
        }
        if targets.contains(&"app") || targets.contains(&"android") {
            if app.contains_key(id) {
                on_app += 1;
            } else {
                problems.push(format!("{id}: no app test (app/e2e/*.spec.ts: test(\"…\", {{ tag: \"@{id}\" }}, …))"));
            }
        }
    }
    for (id, at) in tui.iter().chain(app.iter()) {
        if !ids.contains(id) {
            problems.push(format!("{id}: tested in {} but not in e2e/scenarios.toml", at[0]));
        }
    }

    println!("{} scenarios; tested in the terminal {on_tui}, in the app {on_app}", ids.len());
    assert!(problems.is_empty(), "The scenario rule (e2e/README.md) is broken in {} places:\n  {}", problems.len(), problems.join("\n  "));
}
