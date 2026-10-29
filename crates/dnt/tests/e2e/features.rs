use crate::harness::*;
use crate::seed::Home;
use crate::steps::{character_file, row, see, squash};

/// Open Ranger's sheet from the roster, on the Features tab.
fn ranger_features(mut t: Tui) -> Tui {
    t.send("2").wait("Ranger");
    t.send("j").send("j").wait("Tiefling; Infernal Legacy · Criminal · Ranger 1");
    t.send(ENTER);
    see(&mut t, "Tiefling; Infernal Legacy · Criminal · Ranger 1 · Level 1");
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
    assert!(at("Ranger 1") < at("1 Spellcasting"));
    assert!(at("1 Spellcasting") < at("1 Favored Enemy") && at("1 Favored Enemy") < at("1 Weapon Mastery"));
    assert!(at("1 Weapon Mastery") < at("Species"));
    assert!(at("Species") < at("Tiefling; Infernal Legacy") && at("Tiefling; Infernal Legacy") < at("Background"));
    assert!(at("Background") < at("Criminal") && at("Criminal") < at("Feats"));
    assert!(at("Feats") < at("Alert") && at("Alert") < at("Weapon Mastery"));
    assert!(at("Weapon Mastery") < at("Longbow") && at("Longbow") < at("Scimitar"));
    // The text of the one under the cursor.
    see(&mut t, "╭ Spellcasting ─");
    see(&mut t, "Wisdom is your spellcasting ability for your Ranger spells.");
    t.send("j");
    see(&mut t, "╭ Favored Enemy ─");
    see(&mut t, "You can cast it twice without expending");
    t.send("j");
    see(&mut t, "╭ Weapon Mastery ─");
    see(&mut t, "Your training with weapons allows you");
    t.send("j");
    see(&mut t, "You have Darkvision with a range of 60 feet.");
    t.send("k");
    see(&mut t, "╭ Weapon Mastery ─");
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
    t.send("j");
    see(&mut t, "╭ Favored Enemy ─");
    t.send("O");
    see(&mut t, "╭ Rules for Favored Enemy ─");
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
    assert!(row(&t, "1 Favored Enemy").contains("auto"), "{}", t.screen());
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
    see(&mut t, "Tiefling; Infernal Legacy · Criminal · Ranger 1 · Level 1");
    t.send(ENTER);
    see(&mut t, "╭ Abilities ─");
    see(&mut t, "Tiefling; Infernal Legacy · Criminal · Ranger 1 · Level 1");
    assert!(stat_boxes(&t).contains("│ 40 │"), "{}", t.screen());
}

#[test]
fn a_weapon_mastery_is_swapped() {
    let mut t = ranger_features(Tui::scenario("features.mastery-swap", Home::party()));
    // To Longbow, the first mastery.
    for _ in 0..6 {
        t.send("j");
    }
    see(&mut t, "╭ Longbow ─");
    t.send("r");
    see(&mut t, "Swap the mastery of Longbow for…");
    // Not the ones already mastered.
    t.wait("Longsword").wait("Shortsword");
    assert!(!row(&t, "Swap the mastery").contains("Scimitar"));
    t.typed("Longsword");
    t.send(ENTER);
    see(&mut t, "Weapon Mastery: Longbow → Longsword");
    let rows = list(&t);
    let mastery = rows.iter().position(|r| r == "Weapon Mastery").expect("the masteries");
    assert_eq!(rows[mastery + 1..], ["Longsword".to_string(), "Scimitar".to_string()], "{rows:#?}");
}

/// A Cleric 1 / Ranger 2 / Warlock 2 written by hand: the choices of its levels not made.
const WANDERER: &str = r#"name = "Wanderer"
species = "race:Dwarf|SRD52"
background = "background:Soldier|SRD52"

[abilities]
str = 15
dex = 14
con = 13
int = 8
wis = 13
cha = 13

[[levels]]
class = "class:Cleric|SRD52"

[[levels]]
class = "class:Ranger|SRD52"

[[levels]]
class = "class:Ranger|SRD52"

[[levels]]
class = "class:Warlock|SRD52"

[[levels]]
class = "class:Warlock|SRD52"
"#;

#[test]
fn choices_left_open_are_made() {
    let home = Home::party();
    character_file(&home, "wanderer", WANDERER);
    let mut t = Tui::scenario("choices.pending", home);
    t.send("2").wait("Wanderer");
    t.send("G").send(ENTER);
    see(&mut t, "Dwarf · Soldier · Cleric 1 / Ranger 2 / Warlock 2 · Level 5");
    see(&mut t, "5 choice(s) to make press c");
    t.send("c");
    see(&mut t, "5 choice(s) left");
    // The Cleric's Divine Order: one option.
    see(&mut t, "Thaumaturge");
    t.typed("Thaumaturge");
    t.send(ENTER);
    see(&mut t, "Divine Order chosen");
    see(&mut t, "4 choice(s) left");
    for weapon in ["Longsword", "Scimitar"] {
        t.typed(weapon);
        t.send(" ").send(&ctrl('u'));
    }
    see(&mut t, "2/2 chosen");
    t.send(ENTER);
    see(&mut t, "Weapon Mastery chosen");
    // The fighting style.
    t.wait("Two-Weapon Fighting");
    t.typed("Defense");
    t.send(ENTER);
    see(&mut t, "Fighting Style chosen");
    see(&mut t, "2 choice(s) left");
    // The first level's invocation; some need level 2.
    see(&mut t, "Eldritch Invocations: choose one (Warlock 1)");
    see(&mut t, "Fiendish Vigor ⚠ needs level 2");
    t.typed("Armor of Shadows");
    t.send(ENTER);
    see(&mut t, "1 choice(s) left");
    // Two more at the second; one needs level 5.
    see(&mut t, "Eldritch Invocations: choose 2 (Warlock 2)");
    see(&mut t, "Ascendant Step ⚠ needs level 5");
    for invocation in ["Fiendish Vigor", "Mask of Many Faces"] {
        t.typed(invocation);
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
    for name in [
        "1 Thaumaturge",
        "2 Fighting Style",
        "Defense",
        "Armor of Shadows",
        "Fiendish Vigor",
        "Mask of Many Faces",
        "Longsword",
        "Scimitar",
    ] {
        assert!(rows.iter().any(|r| r == name), "no {name:?} in {rows:#?}");
    }
    // The option is listed under the class feature it is chosen for.
    let at = |name: &str| rows.iter().position(|r| r == name).unwrap_or_else(|| panic!("no {name:?} in {rows:#?}"));
    assert!(
        at("Cleric 1") < at("1 Divine Order") && at("1 Divine Order") < at("1 Thaumaturge") && at("1 Thaumaturge") < at("Ranger 2"),
        "{rows:#?}"
    );
}
