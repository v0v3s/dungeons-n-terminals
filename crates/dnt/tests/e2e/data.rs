use std::io::Cursor;

use crate::harness::*;
use crate::seed::{self, Home};

fn stdout(out: &std::process::Output) -> String {
    assert!(out.status.success(), "dnt failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A zip of the test data alone, or of the homebrew alone, in the home.
fn part_zip(home: &Home, data: bool) -> String {
    let dir = seed::e2e_dir();
    let mut out = Cursor::new(Vec::new());
    let (d, h) = if data { (Some(dir.join("data")), None) } else { (None, Some(dir.join("homebrew"))) };
    dnt_core::pack::pack(d.as_deref(), h.as_deref(), &mut out).unwrap();
    let path = home.path().join(if data { "data-only.zip" } else { "brew-only.zip" });
    std::fs::write(&path, out.into_inner()).unwrap();
    path.display().to_string()
}

fn all_zip(home: &Home) -> String {
    let path = home.path().join("test-data.zip");
    std::fs::write(&path, seed::data_zip()).unwrap();
    path.display().to_string()
}

/// The entry count `dnt imported` gives a source.
fn count(listed: &str, source: &str) -> Option<String> {
    listed.lines().find(|l| l.split_whitespace().next() == Some(source)).map(|l| l.split_whitespace().nth(1).unwrap().to_string())
}

#[test]
fn homebrew_imported_later_joins_the_data() {
    let home = Home::empty();
    let data = part_zip(&home, true);
    let brew = part_zip(&home, false);
    stdout(&run("data.import-later", &home, &["import", &data]));
    let listed = stdout(&run("data.import-later", &home, &["imported"]));
    assert!(count(&listed, "TST").is_some() && count(&listed, "TSTB").is_none(), "{listed}");
    let said = stdout(&run("data.import-later", &home, &["import", &brew]));
    assert!(said.contains("Homebrew"), "{said}");
    let listed = stdout(&run("data.import-later", &home, &["imported"]));
    assert!(count(&listed, "TSTB").is_some() && count(&listed, "TST").is_some(), "{listed}");

    let mut t = Tui::scenario("data.import-later", home);
    t.send("n").typed("Later").send(ENTER).wait("Books · Later");
    t.send(TAB).send(TAB).wait("Homebrew · Later").wait("Proving Grounds Brew");
}

#[test]
fn importing_again_updates_instead_of_doubling() {
    let home = Home::campaign();
    let before = stdout(&run("data.import-later", &home, &["imported"]));
    let zip = all_zip(&home);
    stdout(&run("data.import-later", &home, &["import", &zip]));
    let after = stdout(&run("data.import-later", &home, &["imported"]));
    assert_eq!(before, after);

    let mut t = Tui::scenario("data.import-later", home);
    t.wait("Browse").wait("All 203");
    t.send("/").typed("mirefolk").wait("1 results");
}

#[test]
fn imported_lists_sources_with_counts_and_the_homebrew() {
    let home = Home::data();
    let listed = stdout(&run("data.list", &home, &["imported"]));
    assert_eq!(count(&listed, "TST").as_deref(), Some("184"), "{listed}");
    assert_eq!(count(&listed, "TSX").as_deref(), Some("5"), "{listed}");
    assert!(listed.contains("Homebrew (enable per campaign)"), "{listed}");
    let brew = listed.lines().find(|l| l.starts_with("TSTB")).unwrap_or_else(|| panic!("{listed}"));
    assert!(brew.contains("3 entries") && brew.contains("Proving Grounds Brew"), "{listed}");
}

#[test]
fn forgetting_a_source_removes_its_entries() {
    let home = Home::campaign();
    let said = stdout(&run("data.forget-source", &home, &["forget", "TSX"]));
    assert!(said.contains("Removed TSX"), "{said}");
    let listed = stdout(&run("data.forget-source", &home, &["imported"]));
    assert!(count(&listed, "TSX").is_none() && count(&listed, "TST").is_some(), "{listed}");

    let mut t = Tui::scenario("data.forget-source", home);
    t.send("4").wait("The Proving Grounds");
    assert!(!t.shows("Outskirts"), "{}", t.screen());
    t.send("1").send("/").typed("mirefolk").wait("0 results");
}

#[test]
fn forgetting_homebrew_keeps_the_data() {
    let home = Home::campaign();
    let said = stdout(&run("data.forget-homebrew", &home, &["forget", "--homebrew"]));
    assert!(said.contains("Removed all imported homebrew"), "{said}");
    let listed = stdout(&run("data.forget-homebrew", &home, &["imported"]));
    assert!(count(&listed, "TST").is_some() && !listed.contains("TSTB"), "{listed}");

    let mut t = Tui::scenario("data.forget-homebrew", home);
    t.wait("All 203");
    t.send("4").send(TAB).send(TAB).wait("Books · E2E");
    assert!(!t.shows("Proving Grounds Brew"), "{}", t.screen());
}

#[test]
fn forgetting_everything_keeps_the_campaigns() {
    let home = Home::party();
    let said = stdout(&run("data.forget-all", &home, &["forget", "--all"]));
    assert!(said.contains("campaigns, characters and notes stay"), "{said}");
    let listed = stdout(&run("data.forget-all", &home, &["imported"]));
    assert!(listed.trim().is_empty(), "{listed}");

    // Without data dnt asks for it again; the campaign and party are still there.
    let zip = all_zip(&home);
    let mut t = Tui::scenario("data.forget-all", home);
    t.wait("Getting started");
    t.typed(&zip).send(ENTER);
    t.wait("◈ E2E");
    t.send("2").wait("Brawler").wait("Tester");
}

#[test]
fn reset_asks_first_and_declining_keeps_everything() {
    let home = Home::party();
    // No answer (end of input) is not "reset".
    let said = stdout(&run("data.reset", &home, &["reset"]));
    assert!(said.contains("Type reset to go on") && said.contains("Nothing was deleted."), "{said}");
    let listed = stdout(&run("data.reset", &home, &["imported"]));
    assert!(count(&listed, "TST").is_some(), "{listed}");
    let mut t = Tui::scenario("data.reset", home);
    t.send("2").wait("Tester");
}

#[test]
fn reset_after_typing_reset_deletes_everything() {
    let home = Home::party();
    let campaign = home.campaign_file("");
    let mut t = Tui::with_args("data.reset", home, &["reset"]);
    t.wait("Type reset to go on");
    t.typed("reset").send(ENTER);
    t.wait("dnt is reset");
    assert!(!campaign.exists(), "the campaign folder is gone: {}", campaign.display());
    let listed = stdout(&run("data.reset", &t.home, &["imported"]));
    assert!(listed.trim().is_empty(), "{listed}");
}

#[test]
fn after_a_reset_dnt_starts_as_new() {
    let home = Home::party();
    let said = stdout(&run("data.reset", &home, &["reset", "--yes"]));
    assert!(said.contains("dnt is reset"), "{said}");
    let mut t = Tui::scenario("data.reset", home);
    t.wait("Getting started").wait("1 · Import the data");
}

#[test]
fn the_help_ends_with_about() {
    let mut t = Tui::scenario("data.about", Home::party());
    t.send("?").wait("Help");
    t.send("G").wait("About");
    t.wait(&format!("dnt {} · MIT license", env!("CARGO_PKG_VERSION")));
    t.wait("github.com/v0v3s/dungeons-n-terminals");
    t.wait("It has the SRD 5.2.1 built in, and reads the books you import.");
    // The attribution the SRD's licence asks for.
    t.wait("This work includes material from the System Reference");
    t.wait("licensed under the Creative Commons Attribution 4.0");
    t.send(ESC).wait_gone("MIT license");
}
