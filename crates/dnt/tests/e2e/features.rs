use crate::harness::*;
use crate::seed::Home;
use crate::steps::{character_file, row, see, squash};

/// Open Ranger's sheet from the roster, on the Features tab.
fn ranger_features(mut t: Tui) -> Tui {
    t.send("2").wait("Ranger");
    t.send("j").send("j").wait("Skyborn · Drifter · Warden 1");
    t.send(ENTER);
    see(&mut t, "Skyborn · Drifter · Warden 1 · Level 1");
    t.send(TAB);
    see(&mut t, "╭ Features ─");
    t
}

/// The Features list (the left pane), one squashed row per entry.
fn list(t: &Tui) -> Vec<String> {
    t.screen()
        .lines()
        .filter(|l| l.trim_start().starts_with('│'))
        .filter_map(|l| {
            let pane: String = l.chars().take(42).collect();
            let text = squash(pane.trim_matches(|c: char| c == '│' || c == ' ')).trim().to_string();
            (!text.is_empty() && !text.contains('─')).then_some(text)
        })
        .collect()
}

#[test]
fn features_are_grouped_by_where_they_come_from() {
    let mut t = ranger_features(Tui::scenario("features.read", Home::party()));
    let rows = list(&t);
    let at = |name: &str| rows.iter().position(|r| r == name).unwrap_or_else(|| panic!("no {name:?} in {rows:#?}"));
    // Class levels first, then species, background, feats, masteries.
    assert!(at("Warden 1") < at("1 Spellcasting"));
    assert!(at("1 Favored Ground") < at("1 Forest Ground"));
    assert!(at("1 Forest Ground") < at("Species"));
    assert!(at("Species") < at("Skyborn") && at("Skyborn") < at("Background"));
    assert!(at("Background") < at("Drifter") && at("Drifter") < at("Feats"));
    assert!(at("Feats") < at("Lucky Charm") && at("Lucky Charm") < at("Weapon Mastery"));
    assert!(at("Weapon Mastery") < at("Ashstaff") && at("Ashstaff") < at("Hand Hook"));
    // The text of the one under the cursor.
    see(&mut t, "╭ Spellcasting ─");
    see(&mut t, "You cast warden spells using Wisdom.");
    t.send("j");
    see(&mut t, "╭ Weapon Mastery ─");
    see(&mut t, "You master two kinds of weapons.");
    t.send("j").send("j");
    see(&mut t, "You move through undergrowth without slowing.");
    t.send("j");
    see(&mut t, "You have a flying speed of 30 feet.");
    t.send("k");
    see(&mut t, "╭ Forest Ground ─");
}

/// The value row of the Overview's stat boxes (Armor Class, …, Speed).
fn stat_boxes(t: &Tui) -> String {
    let s = t.screen();
    let lines: Vec<&str> = s.lines().collect();
    let i = lines.iter().position(|l| l.contains("Armor Class")).unwrap_or_else(|| panic!("no stat boxes:\n{s}"));
    squash(lines[i - 2])
}

#[test]
fn a_mistake_in_the_rules_is_named_at_start() {
    let home = Home::party();
    let dir = home.store().overlays_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mine.toml"), "[[feature]]\nid = \"feat:Nonesuch|TST\"\n").unwrap();
    // Rules for imported homebrew the campaign does not turn on are fine.
    std::fs::write(dir.join("brew.toml"), "[[feature]]\nid = \"feat:Brewed Luck|TSTB\"\n").unwrap();
    let mut t = Tui::scenario("features.rules-problems", home);
    t.wait("Rules problem: mine.toml: not in the library: feat:Nonesuch|TST");
    assert!(!t.shows("and 1 more"), "{}", t.screen());
}

#[test]
fn rules_written_for_a_feature_change_the_sheet() {
    let mut t = ranger_features(Tui::scenario("features.rules", Home::party()));
    t.send("j").send("j").send("j");
    see(&mut t, "╭ Forest Ground ─");
    t.send("O");
    see(&mut t, "╭ Rules for Forest Ground ─");
    see(&mut t, "Nothing automates this feature yet");
    // Ten more feet of speed.
    t.send("G").send("o");
    see(&mut t, "INSERT");
    for line in ["[[feature.effect]]", "target = \"speed\"", "add = \"10\""] {
        t.typed(line);
        t.send(ENTER);
    }
    see(&mut t, "✓ valid");
    see(&mut t, "speed: 30 → 40");
    t.send(ESC);
    see(&mut t, "NORMAL");
    t.send(ESC);
    see(&mut t, "Saved your rules");
    assert!(row(&t, "1 Forest Ground").contains("auto"), "{}", t.screen());
    t.send(BACKTAB);
    see(&mut t, "╭ Abilities ─");
    assert!(stat_boxes(&t).contains("│ 40 │"), "{}", t.screen());
    // The rules stay: the next start reads them.
    t.quit();
    let overlays = t.home.path().join("data/library/overlays");
    let saved: Vec<_> = std::fs::read_dir(&overlays).unwrap().map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap()).collect();
    assert!(saved.iter().any(|s| s.contains("target = \"speed\"")), "{saved:?}");
    let mut t = Tui::scenario("features.rules", std::mem::replace(&mut t.home, Home::empty()));
    t.send("2").wait("Ranger");
    t.send("j").send("j");
    see(&mut t, "Skyborn · Drifter · Warden 1 · Level 1");
    t.send(ENTER);
    see(&mut t, "╭ Abilities ─");
    see(&mut t, "Skyborn · Drifter · Warden 1 · Level 1");
    assert!(stat_boxes(&t).contains("│ 40 │"), "{}", t.screen());
}

#[test]
fn a_weapon_mastery_is_swapped() {
    let mut t = ranger_features(Tui::scenario("features.mastery-swap", Home::party()));
    // To Ashstaff, the first mastery.
    for _ in 0..7 {
        t.send("j");
    }
    see(&mut t, "╭ Ashstaff ─");
    t.send("r");
    see(&mut t, "Swap the mastery of Ashstaff for…");
    // Not the ones already mastered.
    t.wait("Longfang").wait("Twin Sickle");
    assert!(!row(&t, "Swap the mastery").contains("Hand Hook"));
    t.typed("Longfang");
    t.send(ENTER);
    see(&mut t, "Weapon Mastery: Ashstaff → Longfang");
    let rows = list(&t);
    let mastery = rows.iter().position(|r| r == "Weapon Mastery").expect("the masteries");
    assert_eq!(rows[mastery + 1..], ["Longfang".to_string(), "Hand Hook".to_string()], "{rows:#?}");
}

/// A level 2 Warden written by hand: the choices of its levels not made.
const WANDERER: &str = r#"name = "Wanderer"
species = "race:Kindred|TST"
background = "background:Drifter|TST"

[abilities]
str = 15
dex = 14
con = 13
int = 10
wis = 12
cha = 8

[[levels]]
class = "class:Warden|TST"

[[levels]]
class = "class:Warden|TST"
"#;

#[test]
fn choices_left_open_are_made() {
    let home = Home::party();
    character_file(&home, "wanderer", WANDERER);
    let mut t = Tui::scenario("choices.pending", home);
    t.send("2").wait("Wanderer");
    t.send("G").send(ENTER);
    see(&mut t, "Kindred · Drifter · Warden 2 · Level 2");
    see(&mut t, "4 choice(s) to make press c");
    t.send("c");
    see(&mut t, "4 choice(s) left");
    see(&mut t, "Mountain Ground");
    t.typed("Mountain");
    t.send(ENTER);
    see(&mut t, "Favored Ground chosen");
    see(&mut t, "3 choice(s) left");
    for weapon in ["Longfang", "Thunderbow"] {
        t.typed(weapon);
        t.send(" ").send(&ctrl('u'));
    }
    see(&mut t, "2/2 chosen");
    t.send(ENTER);
    see(&mut t, "Weapon Mastery chosen");
    // The fighting style.
    t.wait("Guardian Style");
    t.typed("Guardian");
    t.send(ENTER);
    see(&mut t, "Fighting Style chosen");
    see(&mut t, "1 choice(s) left");
    // Two tricks; one needs level 5.
    see(&mut t, "Keen Eye ⚠ needs level 5");
    for trick in ["Iron Grip", "Thorn Shot"] {
        t.typed(trick);
        t.send(" ").send(&ctrl('u'));
    }
    see(&mut t, "2/2 chosen");
    t.send(ENTER);
    see(&mut t, "All choices made");
    assert!(!t.shows("choice(s) to make"), "{}", t.screen());
    t.send("c");
    see(&mut t, "No choices pending");
    t.send(TAB);
    see(&mut t, "╭ Features ─");
    let rows = list(&t);
    for name in ["1 Mountain Ground", "2 Fighting Style", "Guardian Style", "Iron Grip", "Thorn Shot", "Longfang", "Thunderbow"] {
        assert!(rows.iter().any(|r| r == name), "no {name:?} in {rows:#?}");
    }
}
