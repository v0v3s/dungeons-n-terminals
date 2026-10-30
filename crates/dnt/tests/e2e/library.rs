use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::{open, stat_box, wait_status};
use crate::steps::{row, see};

/// The rules library, from Campaigns (the test library: e2e/rules).
fn library(t: &mut Tui) {
    t.send("4").wait("Books · E2E");
    t.send("L");
    see(t, "╭ Rules library ─");
}

/// Your own rules for the Wizard's Ritual Adept: 10 more feet of speed.
fn own_rules(home: &Home) {
    let dir = home.store().overlays_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let mine = "[[feature]]\nid = \"classFeature:Ritual Adept|Wizard|SRD52|1\"\n[[feature.effect]]\ntarget = \"speed\"\nadd = \"10\"\n";
    std::fs::write(dir.join("mine.toml"), mine).unwrap();
}

#[test]
fn the_library_is_a_tree_of_folders_files_and_rules() {
    let mut t = Tui::scenario("rules.library-browse", Home::party());
    library(&mut t);
    // Folded at first: the folders (the SRD first, then the homebrew).
    see(&mut t, "▸ ○ SRD  3 rule(s)");
    see(&mut t, "▸ ○ Homebrew  1 rule(s)");
    let s = t.screen();
    assert!(s.find("SRD").unwrap() < s.find("Homebrew").unwrap(), "{s}");
    assert!(!t.shows("Classes"), "{}", t.screen());
    // Unfolded: its files, folded too; then a file's rules, each saying
    // what it would do.
    t.send("l");
    see(&mut t, "▸ ○ Classes  2 rule(s)");
    assert!(!t.shows("Ritual Adept"), "{}", t.screen());
    t.send("j").send("l");
    see(&mut t, "▾ ○ Classes  2 rule(s)");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("new"), "{}", t.screen());
    assert!(row(&t, "Favored Enemy (Ranger 1)").contains("new · replaces dnt's own"), "{}", t.screen());
    // And folded again, from its folder.
    t.send("g").send("h");
    see(&mut t, "▸ ○ SRD");
    t.wait_gone("Ritual Adept");
    t.send(ESC).wait_gone("Rules library");
    // And from the shell.
    let out = run("rules.library-browse", &t.home, &["rules"]);
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(listed.contains("SRD  (srd)") && listed.contains("Classes  (srd/classes)"), "{listed}");
    assert!(listed.lines().any(|l| l.contains("Favored Enemy (Ranger 1)") && l.contains("replaces dnt's own")), "{listed}");
}

#[test]
fn checked_rules_are_imported_and_the_sheet_uses_them() {
    let mut t = Tui::scenario("rules.library-import", Home::party());
    library(&mut t);
    // A whole file: its rules are checked, its folder partly.
    t.send("l").send("j").send(" ");
    see(&mut t, "▸ ● Classes");
    see(&mut t, "▾ ◐ SRD");
    t.send(ENTER);
    wait_status(&mut t, "Imported 2 rule(s)");
    t.send("l");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("yours already"), "{}", t.screen());
    t.send(ESC).wait_gone("Rules library");
    // Ritual Adept's (invented) five feet.
    open(&mut t, "Tester");
    assert_eq!(stat_box(&t, "Speed"), "35");
    // A single rule, from the shell.
    let out = run("rules.library-import", &t.home, &["rules", "import", "feat:Savage Attacker|SRD52"]);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Imported 1 rule(s)");
    let out = run("rules.library-import", &t.home, &["rules", "import", "nowhere"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("nowhere: not in the rules library"));
}

#[test]
fn rules_that_differ_from_yours_ask_to_overwrite_or_keep() {
    let home = Home::party();
    own_rules(&home);
    let mut t = Tui::scenario("rules.library-conflict", home);
    library(&mut t);
    t.send("l").send("j").send("l");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("differs from yours"), "{}", t.screen());
    // The whole SRD: Ritual Adept asks, kept by default.
    t.send("g").send(" ").send(ENTER);
    see(&mut t, "These differ from your rules for the same features");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("‹keep›"), "{}", t.screen());
    t.send("O");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("‹overwrite›"), "{}", t.screen());
    t.send("K");
    assert!(row(&t, "Ritual Adept (Wizard 1)").contains("‹keep›"), "{}", t.screen());
    t.send("o");
    see(&mut t, "‹overwrite›");
    t.send(ENTER);
    wait_status(&mut t, "Imported 3 rule(s), 1 of yours overwritten");
    t.send(ESC).wait_gone("Rules library");
    open(&mut t, "Tester");
    assert_eq!(stat_box(&t, "Speed"), "35");
    // Replaced where it was: your file is gone with its only rule.
    assert!(!t.home.store().overlays_dir().join("mine.toml").exists());
}

#[test]
fn kept_rules_stay_yours() {
    let home = Home::party();
    own_rules(&home);
    let mut t = Tui::scenario("rules.library-conflict", home);
    // From the shell, without a terminal to ask in: it says so.
    let out = run("rules.library-conflict", &t.home, &["rules", "import", "srd"]);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("1 rule(s) differ from yours: Ritual Adept (Wizard 1); choose with --overwrite-all or --keep-all"), "{said}");
    let out = run("rules.library-conflict", &t.home, &["rules", "import", "srd", "--keep-all"]);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Imported 2 rule(s), 1 of yours kept");
    open(&mut t, "Tester");
    assert_eq!(stat_box(&t, "Speed"), "40");
}
