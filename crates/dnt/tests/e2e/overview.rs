use crate::harness::*;
use crate::seed::Home;
use crate::sheet_play::*;
use crate::steps::row;

/// Tester's first resource, after the four attacks (Dagger, the Arcane
/// Focus (Quarterstaff), Quarterstaff, Unarmed Strike) and 18 skills.
const FIRST_RESOURCE: usize = FIRST_ATTACK + 4 + 18;

/// Write rules for the `n`th feature on the Features tab, as a user does
/// with `O`: the lines go at the end of the template, then Esc saves.
fn write_rules(t: &mut Tui, n: usize, rules: &str) {
    t.send(TAB).wait("edit rules");
    t.send(&"k".repeat(20));
    t.send(&"j".repeat(n));
    t.send("O").wait("Rules for").wait("NORMAL");
    t.send("G").send("o").wait("INSERT");
    t.send(&rules.replace('\n', "\r"));
    t.wait(rules.lines().last().unwrap());
    // One Esc at a time: back to normal mode, then save.
    t.send(ESC).wait("NORMAL");
    t.send(ESC);
    wait_status(t, "Saved your rules");
    t.send(BACKTAB).wait("Armor Class");
}

#[test]
fn conditions_and_exhaustion_turn_on_and_off() {
    let mut t = Tui::scenario("conditions.toggle", Home::party());
    open(&mut t, "Tester");
    t.send("C").wait("╭ Conditions");
    assert!(t.line("Blinded").contains("○ Blinded"), "{}", t.screen());
    t.send(" ");
    wait_status(&mut t, "Blinded");
    t.wait("◉ Blinded");
    t.send("j").send(" ");
    wait_status(&mut t, "Charmed");
    t.wait("◉ Charmed");
    t.send("+");
    wait_status(&mut t, "Exhaustion 1");
    t.wait("−2 to d20 tests");
    t.send(ESC).wait_gone("╭ Conditions");
    assert!(t.line("Conditions").contains("Blinded · Charmed · Exhaustion 1"), "{}", t.screen());
    // Exhaustion 1 takes 5 ft. of speed.
    assert_eq!(stat_box(&t, "Speed"), "25");
    t.send("C").wait("╭ Conditions");
    t.send(" ");
    wait_status(&mut t, "No longer Blinded");
    t.wait("○ Blinded");
    t.send("-");
    wait_status(&mut t, "Exhaustion 0");
    t.send(ESC).wait_gone("╭ Conditions");
    assert!(t.line("Conditions").contains("Charmed"), "{}", t.screen());
    assert!(!t.line("Conditions").contains("Blinded"), "{}", t.screen());
    assert_eq!(stat_box(&t, "Speed"), "30");
}

#[test]
fn feature_uses_slots_and_hit_dice_are_spent_and_restored() {
    let mut t = Tui::scenario("resources.spend", Home::party());
    open(&mut t, "Tester");
    // The SRD's features come without rules for their uses: give Arcane
    // Recovery (the Wizard's third feature) one, through its rules.
    write_rules(
        &mut t,
        3,
        "[[feature.resource]]\nkey = \"arcane_recovery\"\nname = \"Arcane Recovery\"\nmax = \"1\"\nrecharge = \"long_rest\"",
    );
    t.wait("Resources");
    assert!(t.line("Arcane Recovery").contains("1/1"), "{}", t.screen());
    to(&mut t, FIRST_RESOURCE);
    t.send(" ");
    wait_status(&mut t, "Used Arcane Recovery");
    assert!(t.line("Arcane Recovery").contains("0/1"), "{}", t.screen());
    t.send(" ");
    wait_status(&mut t, "No Arcane Recovery left");
    t.send("+");
    wait_status(&mut t, "Restored Arcane Recovery");
    assert!(t.line("Arcane Recovery").contains("1/1"), "{}", t.screen());
    // A spell slot.
    t.send("j").send("-");
    wait_status(&mut t, "Used level 1 slot");
    assert!(t.line("│ Level 1 ").contains("1/2"), "{}", t.screen());
    t.send("=");
    wait_status(&mut t, "Restored level 1 slot");
    assert!(t.line("│ Level 1 ").contains("2/2"), "{}", t.screen());
    // A hit die.
    t.send("j").send(" ");
    wait_status(&mut t, "Used d6 hit die");
    assert!(t.line("│ d6 ").contains("0/1"), "{}", t.screen());
    t.send("+");
    wait_status(&mut t, "Restored d6 hit die");
    assert!(t.line("│ d6 ").contains("1/1"), "{}", t.screen());
    // Nothing to restore on a full one.
    to(&mut t, FIRST_RESOURCE);
    t.send("+");
    wait_status(&mut t, "Arcane Recovery is full");
}

#[test]
fn pact_slots_are_spent_and_restored() {
    let mut t = Tui::scenario("resources.spend", Home::party());
    open(&mut t, "Hexa");
    // The pact slots come just before the hit die.
    t.send("G").send("k").send(" ");
    wait_status(&mut t, "Used pact slot");
    assert!(t.line("Pact (level 1)").contains("0/1"), "{}", t.screen());
    t.send(" ");
    wait_status(&mut t, "No pact slot left");
    t.send("+");
    wait_status(&mut t, "Restored pact slot");
    assert!(t.line("Pact (level 1)").contains("1/1"), "{}", t.screen());
}

#[test]
fn a_toggle_flips_and_the_numbers_follow() {
    let mut t = Tui::scenario("toggles.flip", Home::party());
    open(&mut t, "Tester");
    // A toggle comes from a feature's rules: Spellcasting gets one that
    // adds 2 to the Armor Class while it is on.
    write_rules(
        &mut t,
        1,
        "[[feature.toggle]]\nkey = \"warded\"\nname = \"Warded\"\n[[feature.effect]]\ntarget = \"ac\"\nadd = \"2\"\nwhen = \"toggle('warded')\"",
    );
    t.wait("Toggles").wait("○ Warded");
    assert_eq!(stat_box(&t, "Armor Class"), "12");
    // The toggle is the last item.
    t.send("G").send(ENTER);
    wait_status(&mut t, "Warded on");
    t.wait("◉ Warded");
    assert_eq!(stat_box(&t, "Armor Class"), "14");
    t.send(ENTER);
    wait_status(&mut t, "Warded off");
    t.wait("○ Warded");
    assert_eq!(stat_box(&t, "Armor Class"), "12");
    // Space flips it too.
    t.send(" ");
    wait_status(&mut t, "Warded on");
    assert_eq!(stat_box(&t, "Armor Class"), "14");
}

#[test]
fn a_tracker_of_your_own_is_added() {
    let mut t = Tui::scenario("trackers.add", Home::party());
    open(&mut t, "Tester");
    t.send("a").send("Lucky Break, 2, short").send(ENTER);
    wait_status(&mut t, "Tracking Lucky Break");
    let line = t.line("Lucky Break  ");
    assert!(line.contains("●●") && line.contains("2/2") && line.contains("short rest"), "{line}");
    // Uses and recharge can be left out: one use, a long rest.
    t.send("a").send("Inspiration").send(ENTER);
    wait_status(&mut t, "Tracking Inspiration");
    let line = t.line("Inspiration  ");
    assert!(line.contains("1/1") && line.contains("long rest"), "{line}");
    // A tracker is spent like any resource.
    to(&mut t, FIRST_RESOURCE);
    t.send(" ");
    wait_status(&mut t, "Used Lucky Break");
    assert!(t.line("Lucky Break  ").contains("1/2"), "{}", t.screen());
    // What is not understood is said.
    t.send("a").send("Odd, many").send(ENTER);
    wait_status(&mut t, "many: uses must be a number");
    t.send("a").send("Odd, 1, weekly").send(ENTER);
    wait_status(&mut t, "weekly: recharge is long, short, dawn or manual");
    assert!(!t.shows("Odd"), "{}", t.screen());
}

#[test]
fn a_tracker_of_your_own_is_removed_after_confirming() {
    let mut t = Tui::scenario("trackers.remove", Home::party());
    open(&mut t, "Tester");
    t.send("a").send("Lucky Break, 2, short").send(ENTER);
    wait_status(&mut t, "Tracking Lucky Break");
    // Only your own: not a spell slot.
    t.send("G").send("k").send("x");
    wait_status(&mut t, "x removes a tracker of your own");
    to(&mut t, FIRST_RESOURCE);
    t.send("x").wait("Stop tracking Lucky Break?");
    t.send("n").wait_gone("Stop tracking Lucky Break?");
    t.wait("Lucky Break  ");
    t.send("x").wait("Stop tracking Lucky Break?");
    t.send("y");
    wait_status(&mut t, "No longer tracking Lucky Break");
    let s = t.screen();
    assert_eq!(s.matches("Lucky Break").count(), 1, "only the status line names it:\n{s}");
    assert!(!t.shows("Resources"), "{s}");
}

#[test]
fn weapons_are_held_from_the_attacks() {
    let mut t = Tui::scenario("attacks.hold", Home::party());
    open(&mut t, "Tester");
    // Apart from the keys' hint, not run into it.
    assert!(t.line("In hand").contains("nothing  "), "{}", t.screen());
    // A Dagger, the first attack, in the main hand.
    to(&mut t, FIRST_ATTACK);
    t.send(" ");
    wait_status(&mut t, "Dagger: main hand");
    t.wait("main  Dagger");
    // The second Dagger in the off hand.
    t.send("o");
    wait_status(&mut t, "Dagger: one in each hand");
    assert!(t.line("Dagger  ").contains("pair  Dagger"), "{}", t.screen());
    assert!(t.line("In hand").contains("Dagger ×2"), "{}", t.screen());
    // A light weapon is not held in both hands.
    t.send("b");
    wait_status(&mut t, "Dagger is not used with two hands");
    // The Quarterstaff (versatile, after the Arcane Focus one) in both
    // hands puts the daggers away.
    t.send("w");
    wait_status(&mut t, "Dagger: main hand");
    t.send("jj").send("b");
    wait_status(&mut t, "Quarterstaff: both hands (put away Dagger)");
    assert!(t.line(" Quarterstaff  ").contains("both  Quarterstaff"), "{}", t.screen());
    assert!(t.line("In hand").contains("Quarterstaff (two hands)"), "{}", t.screen());
    assert!(!t.line("Dagger  ").contains("main"), "{}", t.screen());
}

#[test]
fn a_weapon_is_held_in_the_off_hand() {
    let mut t = Tui::scenario("attacks.hold", Home::party());
    open(&mut t, "Ranger");
    // The Scimitar (light), the first attack.
    to(&mut t, FIRST_ATTACK);
    t.send("o");
    wait_status(&mut t, "Scimitar: off hand");
    assert!(row(&t, "Scimitar +").contains("│ off Scimitar"), "{}", t.screen());
    assert!(t.line("In hand").contains("Scimitar"), "{}", t.screen());
    // Unarmed strikes need no hands: after the Scimitar, Shortsword,
    // Longbow and Dagger.
    t.send("G");
    to(&mut t, FIRST_ATTACK + 4);
    t.send(" ");
    wait_status(&mut t, "Unarmed strikes need no hands");
}

#[test]
fn the_shield_is_taken_up_and_put_away() {
    let mut t = Tui::scenario("attacks.shield", Home::party());
    open(&mut t, "Tester");
    t.send("S");
    wait_status(&mut t, "No shield in the inventory");
    // A Shield from the Inventory: the plain one, after eight magic items
    // and gear with "Shield" in the name.
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    t.send("a").send("Shield").wait("Spellguard Shield");
    t.send(&DOWN.repeat(8)).send(ENTER);
    wait_status(&mut t, "Added Shield");
    t.send(TAB).send(TAB).wait("Armor Class");
    assert_eq!(stat_box(&t, "Armor Class"), "12");
    t.send("S");
    wait_status(&mut t, "Shield: off hand");
    assert!(t.line("In hand").contains("Shield"), "{}", t.screen());
    assert_eq!(stat_box(&t, "Armor Class"), "14");
    t.send("S");
    wait_status(&mut t, "Put away Shield");
    // Apart from the keys' hint, not run into it.
    assert!(t.line("In hand").contains("nothing  "), "{}", t.screen());
    assert_eq!(stat_box(&t, "Armor Class"), "12");
}
