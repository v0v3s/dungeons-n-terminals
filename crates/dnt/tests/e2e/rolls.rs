use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;

/// Tester's first skill, Acrobatics (+4), after the three attacks.
const ACROBATICS: usize = FIRST_ATTACK + 3;

#[test]
fn checks_saves_skills_and_initiative_roll() {
    let mut t = Tui::scenario("roll.checks", Home::party());
    open(&mut t, "Tester");
    // Strength +2, its save +2 (not proficient).
    to(&mut t, STRENGTH);
    t.send("r");
    wait_status(&mut t, "Strength check: ");
    assert!((3..=22).contains(&rolled(&t, "Strength check")), "{}", status(&t));
    t.send("R");
    wait_status(&mut t, "Strength save: ");
    assert!((3..=22).contains(&rolled(&t, "Strength save")), "{}", status(&t));
    to(&mut t, ACROBATICS);
    t.send("r");
    wait_status(&mut t, "Acrobatics: ");
    assert!((5..=24).contains(&rolled(&t, "Acrobatics")), "{}", status(&t));
    to(&mut t, INITIATIVE);
    t.send("r");
    wait_status(&mut t, "Initiative: ");
    assert!((3..=22).contains(&rolled(&t, "Initiative")), "{}", status(&t));
    // The rolls are listed on the sheet too.
    assert!(t.line("Rolled").contains("Initiative"), "{}", t.screen());
    t.wait("Acrobatics  ").wait("Strength save  ");
}

#[test]
fn an_attack_rolls_to_hit_and_its_damage() {
    let mut t = Tui::scenario("roll.attack", Home::party());
    open(&mut t, "Tester");
    // Thornknife: +4 to hit, 1d4 + 2 piercing.
    to(&mut t, FIRST_ATTACK);
    t.send("r");
    wait_status(&mut t, "Thornknife");
    let line = status(&t);
    assert!(line.contains("piercing damage"), "{line}");
    assert!((5..=24).contains(&rolled(&t, "Thornknife")), "{line}");
    t.wait("Rolled");
}

#[test]
fn advantage_and_disadvantage_apply_to_the_next_roll_only() {
    let mut t = Tui::scenario("roll.advantage", Home::party());
    open(&mut t, "Tester");
    to(&mut t, STRENGTH);
    t.send("v");
    wait_status(&mut t, "Next roll: advantage");
    t.send("r");
    wait_status(&mut t, "Strength check: ");
    assert!(status(&t).contains("advantage"), "{}", status(&t));
    // The next one is a plain roll again.
    t.send("R");
    wait_status(&mut t, "Strength save: ");
    assert!(!status(&t).contains("advantage"), "{}", status(&t));
    t.send("v").send("v");
    wait_status(&mut t, "Next roll: disadvantage");
    t.send("r");
    wait_status(&mut t, "Strength check: ");
    assert!(status(&t).contains("disadvantage"), "{}", status(&t));
    // A third press goes back to normal.
    t.send("v").send("v").send("v");
    wait_status(&mut t, "Next roll: normal");
}

#[test]
fn a_hit_die_heals() {
    let mut t = Tui::scenario("roll.hit-die", Home::party());
    open(&mut t, "Tester");
    // The hit die is the last item.
    t.send("G").send("r");
    wait_status(&mut t, "Hit points are full");
    t.send("d").send("6").send(ENTER);
    wait_status(&mut t, "Damage 6");
    assert_eq!(hit_points(&t), "1/7");
    t.send("r");
    wait_status(&mut t, "Spent a d6 hit die");
    // d6 + 1 (Constitution): 2 to 7 hit points.
    wait_status(&mut t, "Hit die d6: +");
    let hp: i64 = hit_points(&t).split('/').next().unwrap().parse().unwrap();
    assert!((3..=7).contains(&hp), "{}", t.screen());
    assert!(t.line("│ d6 ").contains("0/1"), "{}", t.screen());
    if hp < 7 {
        t.send("r");
        wait_status(&mut t, "No d6 hit die left");
    }
}

#[test]
fn rolls_go_into_the_latest_session_note() {
    let mut t = Tui::scenario("roll.to-notes", Home::party());
    t.send("3").wait("Sessions");
    t.send("s").wait("## Recap");
    t.send(ESC);
    open(&mut t, "Tester");
    t.send("N");
    wait_status(&mut t, "Rolls go into the latest session note");
    to(&mut t, STRENGTH);
    t.send("r");
    wait_status(&mut t, "Strength check: ");
    let logged = status(&t).split("Strength check: ").nth(1).unwrap().trim().to_string();
    // Off again: the next roll stays on the sheet.
    t.send("N");
    wait_status(&mut t, "Rolls stay on the sheet");
    t.send("R");
    wait_status(&mut t, "Strength save: ");
    t.send("3").wait("## Rolls").wait(&format!("- Strength check: {logged}"));
    assert!(!t.shows("Strength save"), "{}", t.screen());
    // And in the note's file.
    let dir = t.home.campaign_file("notes/Sessions");
    let note = std::fs::read_dir(&dir).unwrap().next().expect("a session note").unwrap().path();
    let text = std::fs::read_to_string(note).unwrap();
    assert!(text.contains(&format!("- Strength check: {logged}")), "{text}");
    assert!(!text.contains("Strength save"), "{text}");
}

#[test]
fn rolling_without_a_session_note_says_so() {
    let mut t = Tui::scenario("roll.to-notes", Home::party());
    open(&mut t, "Tester");
    t.send("N");
    wait_status(&mut t, "Rolls go into the latest session note");
    t.send("r");
    wait_status(&mut t, "no session note yet");
}

#[test]
fn a_number_explains_how_it_adds_up() {
    let mut t = Tui::scenario("explain.number", Home::party());
    open(&mut t, "Tester");
    to(&mut t, AC);
    t.send(ENTER);
    t.wait("╭ Armor Class ").wait("✓ base");
    assert!(!t.shows("Rolled"), "nothing is rolled: {}", t.screen());
    t.send(ESC).wait_gone("╭ Armor Class ");
    // An attack explains its bonus and damage.
    to(&mut t, FIRST_ATTACK);
    t.send(ENTER);
    t.wait("Attack bonus").wait("Damage").wait("Proficient.");
    t.send(ESC).wait_gone("Attack bonus");
    assert!(!t.shows("Rolled"), "nothing is rolled: {}", t.screen());
}
