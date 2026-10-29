use crate::harness::*;
use crate::seed::{self, Home};

#[test]
fn the_first_start_has_the_srd_and_asks_for_a_campaign() {
    let mut t = Tui::scenario("setup.srd-built-in", Home::empty());
    t.wait("✓ 1 · The rules: the SRD 5.2.1").wait("Built in. dnt import adds your").wait("▸ 2 · Create a campaign");
    t.send("n").typed("Westmarch").send(ENTER).wait("Created campaign");
    t.wait("System Reference Document 5.2.1");
    t.send("1").send("/").typed("fireball").wait("Fireball");
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
fn a_data_folder_given_with_data_is_read_over_the_srd() {
    let folder = seed::e2e_dir().join("data").canonicalize().unwrap();
    let folder = folder.display().to_string();
    let mut t = Tui::with_args("setup.import-folder", Home::empty(), &["--data", &folder]);
    t.wait("2 · Create a campaign");
    t.send("n").typed("Folder").send(ENTER).wait("The Proving Grounds");
    t.send("1").send("/").typed("mirefolk").wait("Mirefolk");
    // The SRD is there too.
    t.send(&ctrl('u')).typed("eldritch blast").wait("Eldritch Blast");
    // Read in place: nothing was imported, the folder is remembered.
    let out = run("setup.import-folder", &t.home, &["imported"]);
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(!listed.contains("TST"), "nothing is imported: {listed}");
    let config = std::fs::read_to_string(t.home.path().join("config.toml")).unwrap();
    assert!(config.contains("e2e"), "the folder is remembered: {config}");
}
