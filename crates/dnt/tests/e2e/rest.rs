use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;

#[test]
fn a_short_rest_brings_back_short_rest_resources() {
    let mut t = Tui::scenario("rest.short", Home::party());
    open(&mut t, "Hexa");
    // The last items: the pact slots, then the hit die.
    t.send("G").send(" ");
    wait_status(&mut t, "Used d8 hit die");
    t.send("k").send(" ");
    wait_status(&mut t, "Used pact slot");
    assert!(t.line("Pact (level 1)").contains("0/1"), "{}", t.screen());
    // Not without confirming.
    t.send("s").wait("Take a short rest?");
    t.send("n").wait_gone("Take a short rest?");
    assert!(t.line("Pact (level 1)").contains("0/1"), "{}", t.screen());
    t.send("s").wait("Take a short rest?");
    t.send("y");
    wait_status(&mut t, "Short rest: pact slots");
    assert!(t.line("Pact (level 1)").contains("1/1"), "{}", t.screen());
    // Hit dice come back on a long rest only.
    assert!(t.line("│ d8 ").contains("0/1"), "{}", t.screen());
}

#[test]
fn a_short_rest_brings_back_a_short_rest_tracker() {
    let mut t = Tui::scenario("rest.short", Home::party());
    open(&mut t, "Tester");
    t.send("a").send("Lucky Break, 2, short").send(ENTER);
    wait_status(&mut t, "Tracking Lucky Break");
    t.send("a").send("Long Watch, 1, long").send(ENTER);
    wait_status(&mut t, "Tracking Long Watch");
    // The trackers come right after the skills: 12 + 4 attacks + 18 skills.
    to(&mut t, 34);
    t.send(" ");
    wait_status(&mut t, "Used Lucky Break");
    t.send("j").send(" ");
    wait_status(&mut t, "Used Long Watch");
    assert!(t.line("Lucky Break").contains("1/2"), "{}", t.screen());
    t.send("s").wait("Take a short rest?");
    t.send(ENTER);
    wait_status(&mut t, "Short rest: ");
    assert!(t.line("Lucky Break").contains("2/2"), "{}", t.screen());
    assert!(t.line("Long Watch").contains("0/1"), "{}", t.screen());
}

#[test]
fn a_long_rest_brings_back_hit_points_slots_and_hit_dice() {
    let mut t = Tui::scenario("rest.long", Home::party());
    open(&mut t, "Tester");
    t.send("d").send("3").send(ENTER);
    wait_status(&mut t, "Damage 3");
    assert_eq!(hit_points(&t), "5/8");
    // The last items: the level 1 slots, then the hit die.
    t.send("G").send(" ");
    wait_status(&mut t, "Used d6 hit die");
    t.send("k").send(" ");
    wait_status(&mut t, "Used level 1 slot");
    assert!(t.line("│ Level 1 ").contains("1/2"), "{}", t.screen());
    assert!(t.line("│ d6 ").contains("0/1"), "{}", t.screen());
    // Not without confirming.
    t.send("L").wait("Take a long rest?");
    t.send(ESC).wait_gone("Take a long rest?");
    assert_eq!(hit_points(&t), "5/8");
    t.send("L").wait("Take a long rest?");
    t.send("y");
    wait_status(&mut t, "Long rest: ");
    t.wait_until("full hit points", |s| s.lines().any(|l| l.contains("Hit Points") && l.contains("8/8")));
    assert!(t.line("│ Level 1 ").contains("2/2"), "{}", t.screen());
    assert!(t.line("│ d6 ").contains("1/1"), "{}", t.screen());
}
