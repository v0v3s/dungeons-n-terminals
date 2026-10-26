use crate::harness::*;
use crate::seed::{self, Home};

/// The packed test data, as a file in the home.
fn zip_in(home: &Home) -> String {
    let path = home.path().join("test-data.zip");
    std::fs::write(&path, seed::data_zip()).unwrap();
    path.display().to_string()
}

#[test]
fn the_first_start_imports_the_zip_and_asks_for_a_campaign() {
    let home = Home::empty();
    let zip = zip_in(&home);
    let mut t = Tui::scenario("setup.import-first-run", home);
    t.wait("Getting started").wait("1 · Import the data");
    t.typed(&zip).send(ENTER);
    t.wait("✓ 1 · Import the data").wait("▸ 2 · Create a campaign").wait("and name it");
    t.wait_gone("Getting started");
    t.send("2").send("1").wait("Browse");
}

#[test]
fn a_wrong_path_at_getting_started_says_so() {
    let home = Home::empty();
    let missing = home.path().join("nothing-here.zip").display().to_string();
    let mut t = Tui::scenario("setup.import-first-run", home);
    t.wait("Getting started");
    t.typed(&missing).send(ENTER).wait("nothing-here.zip:");
    t.wait("Getting started");
    t.send(&ctrl('u'));
    assert!(!t.line("❯").contains("nothing-here"), "ctrl+u clears the path: {}", t.screen());
}

#[test]
fn the_first_campaign_becomes_the_active_one() {
    let mut t = Tui::scenario("setup.first-campaign", Home::data());
    t.wait("2 · Create a campaign");
    t.send("n").wait("New campaign");
    t.typed("Westmarch").send(ENTER);
    t.wait("Created campaign");
    assert!(t.screen().lines().any(|l| l.starts_with(" │ ◈ Westmarch") && l.contains("active")), "{}", t.screen());
    t.wait("Books · Westmarch");
    t.wait_gone("2 · Create a campaign");
    // The header names the active campaign everywhere.
    t.send("2");
    assert!(t.line(" dnt ").contains("◈ Westmarch"), "{}", t.screen());
    t.send("3").wait("Notes · Westmarch");
}

#[test]
fn a_data_folder_at_getting_started_is_read_in_place() {
    let folder = seed::e2e_dir().join("data").canonicalize().unwrap();
    let home = Home::empty();
    let mut t = Tui::scenario("setup.import-folder", home);
    t.wait("Getting started");
    t.typed(&folder.display().to_string()).send(ENTER);
    t.wait("2 · Create a campaign");
    t.send("n").typed("Folder").send(ENTER).wait("The Proving Grounds").wait("Proving Grounds: Outskirts");
    // Read in place: nothing was imported, the folder is remembered.
    let out = run("setup.import-folder", &t.home, &["imported"]);
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(!listed.contains("TST"), "nothing is imported: {listed}");
    let config = std::fs::read_to_string(t.home.path().join("config.toml")).unwrap();
    assert!(config.contains("e2e"), "the folder is remembered: {config}");
}

#[test]
fn a_data_folder_given_with_data_skips_getting_started() {
    let folder = seed::e2e_dir().join("data").canonicalize().unwrap();
    let folder = folder.display().to_string();
    let mut t = Tui::with_args("setup.import-folder", Home::empty(), &["--data", &folder]);
    t.wait("2 · Create a campaign");
    assert!(!t.shows("Getting started"), "{}", t.screen());
    t.send("n").typed("Folder").send(ENTER).wait("The Proving Grounds");
    t.send("1").send("/").typed("mirefolk").wait("Mirefolk");
}
