use crate::harness::*;
use crate::seed::Home;
use crate::steps::{character_file, row, see, shows};

/// Open Tester's sheet from the roster.
fn tester(mut t: Tui) -> Tui {
    t.send("2").send("G").wait("Kindred · Archivist · Mage 1");
    t.send(ENTER);
    see(&mut t, "Kindred · Archivist · Mage 1 · Level 1");
    t
}

/// Wait for a level-up question, type a filter and choose the first match.
#[track_caller]
fn answer(t: &mut Tui, question: &str, filter: &str) {
    see(t, question);
    t.typed(filter);
    t.send(ENTER);
}

/// Take a Mage level with the average hit points.
#[track_caller]
fn mage_level(t: &mut Tui, to: u32) {
    t.send("u");
    answer(t, "Level up: choose a class", &format!("Mage {} → {to}", to - 1));
    answer(t, "Hit points for this level", "Average");
}

#[track_caller]
fn confirm(t: &mut Tui, level: u32) {
    see(t, "Confirm level up");
    t.send(ENTER);
    see(t, &format!("Now level {level}"));
}

#[test]
fn a_level_with_the_average_hit_points() {
    let mut t = tester(Tui::scenario("levelup.average", Home::party()));
    t.wait("7/7");
    t.send("u");
    see(&mut t, "Level up: choose a class");
    see(&mut t, "Mage 1 → 2");
    t.send(ENTER);
    see(&mut t, "Hit points for this level");
    see(&mut t, "Average 4 d6");
    t.send(ENTER);
    see(&mut t, "✓ Class: Mage 1 → 2");
    see(&mut t, "✓ Hit points: Average 4");
    confirm(&mut t, 2);
    see(&mut t, "Kindred · Archivist · Mage 2 · Level 2");
    // 7 + 4 + 1 (Constitution).
    t.wait("12/12");
    see(&mut t, "d6 ●● 2/2");
    t.send("q").wait("Tester");
    assert!(row(&t, "◆ Tester").contains("Lv 2"), "{}", t.screen());
}

#[test]
fn hit_points_rolled_at_the_table() {
    let mut t = tester(Tui::scenario("levelup.hp", Home::party()));
    t.send("u");
    answer(&mut t, "Level up: choose a class", "Mage");
    answer(&mut t, "Hit points for this level", "Roll it at the table");
    t.typed("5");
    see(&mut t, "❯ 5");
    t.send(ENTER);
    see(&mut t, "✓ Hit points: rolled 5");
    confirm(&mut t, 2);
    // 7 + 5 + 1.
    t.wait("13/13");
}

#[test]
fn hit_points_at_the_maximum() {
    let mut t = tester(Tui::scenario("levelup.hp", Home::party()));
    t.send("u");
    answer(&mut t, "Level up: choose a class", "Mage");
    see(&mut t, "Maximum 6 house rule");
    answer(&mut t, "Hit points for this level", "Maximum");
    see(&mut t, "Confirm level up");
    confirm(&mut t, 2);
    // 7 + 6 + 1.
    t.wait("14/14");
}

/// A level 1 Mage with the scores to take up the Hexer.
const DABBLER: &str = r#"name = "Dabbler"
species = "race:Kindred|TST"
background = "background:Archivist|TST"

[abilities]
str = 8
dex = 12
con = 14
int = 15
wis = 10
cha = 13

[[levels]]
class = "class:Mage|TST"
"#;

#[test]
fn a_level_in_another_class() {
    let home = Home::party();
    character_file(&home, "dabbler", DABBLER);
    let mut t = Tui::scenario("levelup.multiclass", home);
    t.send("2").wait("Dabbler");
    t.send("j");
    see(&mut t, "Kindred · Archivist · Mage 1");
    t.send(ENTER);
    see(&mut t, "Kindred · Archivist · Mage 1 · Level 1");
    t.send("u");
    see(&mut t, "Level up: choose a class");
    // The scores allow the Hexer, not the Monk.
    assert!(!row(&t, "New class: Hexer").contains('⚠'), "{}", t.screen());
    assert!(row(&t, "New class: Monk").contains("⚠ needs Dexterity 13 and Wisdom 13"), "{}", t.screen());
    answer(&mut t, "Level up: choose a class", "Hexer");
    see(&mut t, "✓ Class: New class: Hexer");
    // The Hexer's subclass comes at its first level.
    answer(&mut t, "Choose a subclass", "Tide");
    answer(&mut t, "Hit points for this level", "Average");
    see(&mut t, "✓ Subclass: The Tide");
    see(&mut t, "This becomes character level 1 in this class.");
    confirm(&mut t, 2);
    see(&mut t, "Kindred · Archivist · Mage 1 / Hexer 1 · Level 2");
    assert!(!t.shows("warning"), "{}", t.screen());
    see(&mut t, "Hexer DC 11");
    see(&mut t, "d8 ● 1/1");
}

#[test]
fn the_subclass_at_its_level() {
    let mut t = tester(Tui::scenario("levelup.subclass", Home::party()));
    mage_level(&mut t, 2);
    confirm(&mut t, 2);
    t.send("u");
    answer(&mut t, "Level up: choose a class", "Mage 2 → 3");
    see(&mut t, "Choose a subclass");
    t.wait("Order of Sparks").wait("Order of Wards");
    answer(&mut t, "Choose a subclass", "Wards");
    answer(&mut t, "Hit points for this level", "Average");
    see(&mut t, "✓ Subclass: Order of Wards");
    confirm(&mut t, 3);
    see(&mut t, "Mage 3 · Level 3");
    t.send(TAB).wait("Order of Wards");
}

/// Tester taken to level 3, then at the level-up to 4 after the hit points.
fn at_the_improvement(t: Tui) -> Tui {
    let mut t = tester(t);
    mage_level(&mut t, 2);
    confirm(&mut t, 2);
    t.send("u");
    answer(&mut t, "Level up: choose a class", "Mage 2 → 3");
    answer(&mut t, "Choose a subclass", "Sparks");
    answer(&mut t, "Hit points for this level", "Average");
    confirm(&mut t, 3);
    mage_level(&mut t, 4);
    see(&mut t, "Ability Score Improvement");
    t
}

#[test]
fn an_improvement_raises_a_score() {
    let mut t = at_the_improvement(Tui::scenario("levelup.asi", Home::party()));
    see(&mut t, "+2 to one ability score");
    see(&mut t, "Take a feat instead…");
    answer(&mut t, "Ability Score Improvement", "+2 to one");
    see(&mut t, "Intelligence 14 → 16");
    t.typed("Intelligence");
    t.send(ENTER);
    see(&mut t, "✓ +2 Intelligence 14 → 16");
    confirm(&mut t, 4);
    see(&mut t, "Mage 4 · Level 4");
    assert!(row(&t, "│ Intelligence").contains("Intelligence 16 +3"), "{}", t.screen());
}

#[test]
fn an_improvement_taken_as_a_feat() {
    let mut t = at_the_improvement(Tui::scenario("levelup.asi", Home::party()));
    answer(&mut t, "Ability Score Improvement", "Take a feat");
    t.wait("Tough as Roots");
    t.typed("Tough");
    t.send(ENTER);
    see(&mut t, "Tough as Roots");
    confirm(&mut t, 4);
    see(&mut t, "Mage 4 · Level 4");
    t.send(TAB).wait("Tough as Roots");
}

/// A character written by hand, without a class yet.
const BLANK: &str = r#"name = "Blank"
species = "race:Kindred|TST"
background = "background:Archivist|TST"

[abilities]
str = 10
dex = 14
con = 14
int = 15
wis = 12
cha = 13
"#;

#[test]
fn the_first_level_of_a_character_without_one() {
    let home = Home::party();
    character_file(&home, "blank", BLANK);
    let mut t = Tui::scenario("levelup.first-level", home);
    t.send("2").wait("Blank");
    assert!(row(&t, "Blank").contains("Lv 0"), "{}", t.screen());
    t.send(ENTER);
    see(&mut t, "No class yet — press u to take your first level");
    t.send("u");
    see(&mut t, "First level: choose a class");
    answer(&mut t, "First level: choose a class", "Hexer");
    answer(&mut t, "Choose a subclass", "Lantern");
    // No hit points question: the first level takes the maximum.
    see(&mut t, "✓ Hit points: 8 (maximum at 1st level)");
    confirm(&mut t, 1);
    see(&mut t, "Kindred · Archivist · Hexer 1 · Level 1");
    assert!(!shows(&t, "No class yet"), "{}", t.screen());
    // 8 + 2 (Constitution).
    t.wait("10/10");
}

#[test]
fn the_last_level_is_undone_after_confirming() {
    let mut t = tester(Tui::scenario("levelup.undo", Home::party()));
    mage_level(&mut t, 2);
    confirm(&mut t, 2);
    see(&mut t, "Mage 2 · Level 2");
    t.send("U");
    see(&mut t, "Remove the latest level (Mage)?");
    t.send("n");
    t.wait_gone("Remove the latest level");
    see(&mut t, "Mage 2 · Level 2");
    t.send("U");
    see(&mut t, "Remove the latest level (Mage)?");
    t.send("y");
    see(&mut t, "Removed a Mage level");
    see(&mut t, "Kindred · Archivist · Mage 1 · Level 1");
    t.wait("7/7");
}
