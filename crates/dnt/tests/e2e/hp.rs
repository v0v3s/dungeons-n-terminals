use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;

/// Type an amount into the damage (`d`), heal (`h`) or temporary hit
/// points (`t`) prompt.
fn amount(t: &mut Tui, key: &str, n: i64) {
    t.send(key);
    t.send(&n.to_string());
    t.send(ENTER);
}

#[test]
fn damage_takes_temporary_hit_points_first_and_leaves_the_character_dying_at_0() {
    let mut t = Tui::scenario("hp.damage", Home::party());
    open(&mut t, "Tester");
    assert_eq!(hit_points(&t), "8/8");
    amount(&mut t, "t", 5);
    t.wait("+5 temp");
    amount(&mut t, "d", 3);
    wait_status(&mut t, "Damage 3");
    t.wait("+2 temp");
    assert_eq!(hit_points(&t), "8/8  +2 temp");
    amount(&mut t, "d", 4);
    wait_status(&mut t, "Damage 4");
    t.wait_gone("temp");
    assert_eq!(hit_points(&t), "6/8");
    amount(&mut t, "d", 6);
    wait_status(&mut t, "Damage 6");
    t.wait("Death saves");
    assert_eq!(hit_points(&t), "0/8");
    assert!(t.line("Death saves").contains("D roll"), "{}", t.screen());
}

#[test]
fn damage_that_kills_outright_can_leave_the_character_dead() {
    let mut t = Tui::scenario("hp.damage-lethal", Home::party());
    open(&mut t, "Tester");
    // 8 over 0 is as much as the maximum of 8.
    amount(&mut t, "d", 16);
    t.wait("Killed outright?").wait("as much over 0 as the hit point maximum");
    t.send("y");
    t.wait("✝ Dead");
    wait_status(&mut t, "Dead");
    assert_eq!(hit_points(&t), "0/8");
}

#[test]
fn damage_that_kills_outright_can_leave_the_character_dying() {
    let mut t = Tui::scenario("hp.damage-lethal", Home::party());
    open(&mut t, "Tester");
    // Esc takes no damage at all.
    amount(&mut t, "d", 20);
    t.wait("Killed outright?");
    t.send(ESC).wait_gone("Killed outright?");
    assert_eq!(hit_points(&t), "8/8");
    amount(&mut t, "d", 20);
    t.wait("Killed outright?");
    t.send("n");
    wait_status(&mut t, "Damage 20: at 0 hit points, dying");
    t.wait("Death saves");
    assert!(!t.shows("✝ Dead"), "{}", t.screen());
    assert_eq!(hit_points(&t), "0/8");
}

#[test]
fn healing_stops_at_the_maximum() {
    let mut t = Tui::scenario("hp.heal", Home::party());
    open(&mut t, "Tester");
    amount(&mut t, "d", 5);
    wait_status(&mut t, "Damage 5");
    assert_eq!(hit_points(&t), "3/8");
    amount(&mut t, "h", 3);
    wait_status(&mut t, "Heal 3");
    assert_eq!(hit_points(&t), "6/8");
    amount(&mut t, "h", 10);
    wait_status(&mut t, "Heal 10");
    assert_eq!(hit_points(&t), "8/8");
}

#[test]
fn healing_brings_a_dying_character_back_up() {
    let mut t = Tui::scenario("hp.heal", Home::party());
    open(&mut t, "Tester");
    amount(&mut t, "d", 8);
    t.wait("Death saves");
    amount(&mut t, "h", 2);
    wait_status(&mut t, "Heal 2");
    t.wait_gone("Death saves");
    assert_eq!(hit_points(&t), "2/8");
}

#[test]
fn temporary_hit_points_keep_the_higher_amount() {
    let mut t = Tui::scenario("hp.temp", Home::party());
    open(&mut t, "Tester");
    amount(&mut t, "t", 5);
    wait_status(&mut t, "Temporary hit points 5");
    assert_eq!(hit_points(&t), "8/8  +5 temp");
    // Temporary hit points do not add up: the higher amount stays.
    amount(&mut t, "t", 3);
    wait_status(&mut t, "Temporary hit points 3");
    assert_eq!(hit_points(&t), "8/8  +5 temp");
    amount(&mut t, "t", 8);
    wait_status(&mut t, "Temporary hit points 8");
    assert_eq!(hit_points(&t), "8/8  +8 temp");
}

#[test]
fn death_saves_are_rolled_while_dying_and_the_dead_can_be_marked_not_dead() {
    let mut t = Tui::scenario("hp.death-save", Home::party());
    open(&mut t, "Tester");
    t.send("D");
    wait_status(&mut t, "Death saves are made at 0 hit points");
    amount(&mut t, "d", 8);
    t.wait("Death saves");
    assert!(t.line("Death saves").contains("○○○  ○○○"), "{}", t.screen());
    t.send("D");
    wait_status(&mut t, "Death save: a ");
    assert!(t.line("Death saves").contains('●'), "a save is marked: {}", t.screen());
    // Roll on until three failures (the dice are repeatable: no natural 20).
    for _ in 0..5 {
        if t.shows("✝ Dead") {
            break;
        }
        let marks = |s: &str| s.lines().find(|l| l.contains("Death saves")).map_or(0, |l| l.matches('●').count());
        let before = marks(&t.screen());
        t.send("D");
        t.wait_until("another death save", |s| s.contains("✝ Dead") || marks(s) > before);
    }
    t.wait("✝ Dead");
    wait_status(&mut t, "a third failure: dead");
    t.send("D");
    wait_status(&mut t, "Dead: no more death saves");
    t.send("h");
    wait_status(&mut t, "No longer dead");
    t.wait_gone("✝ Dead");
    t.wait("Death saves");
}
