use crate::harness::*;
use crate::seed::Home;

/// The compendium with `query` typed into its search.
fn search(t: &mut Tui, query: &str) {
    t.send("1").wait("Browse").send("/").typed(query);
}

/// The row of the list's selected result.
fn selected(t: &Tui) -> String {
    t.line("▌")
}

#[test]
fn results_update_as_you_type() {
    let mut t = Tui::scenario("compendium.search", Home::campaign());
    search(&mut t, "gl");
    t.wait("Glaive").wait("Globe of Invulnerability");
    t.typed("yph").wait("1 results").wait("You inscribe a glyph that later unleashes a magical effect.");
    assert!(!t.shows("Glaive"), "{}", t.screen());
    t.send(BACKSPACE).send(BACKSPACE).send(BACKSPACE).wait("Glaive");
    t.send(&ctrl('u')).wait("2041 results");
}

#[test]
fn filters_narrow_a_search() {
    let mut t = Tui::scenario("compendium.filters", Home::campaign());
    search(&mut t, "@TSX");
    // The older book's Fire Bolt is reprinted in the SRD: only the reprint shows.
    t.wait("2 results").wait("Mirefolk").wait("Outskirt Gust");
    assert!(!t.shows("Fire Bolt") && !t.shows("Glaive"), "{}", t.screen());

    // Level 1 spells: not the cantrip Acid Splash, not the level 2 Acid Arrow.
    t.send(&ctrl('u')).typed("#spell ^1").wait("58 results").wait("Alarm");
    assert!(!t.shows("Acid Splash") && !t.shows("Acid Arrow"), "{}", t.screen());

    // And not from TSX: Outskirt Gust goes.
    t.typed(" !@TSX").wait("57 results");

    t.send(&ctrl('u')).typed("\"hurl a bolt\"").wait("1 results");
    assert!(selected(&t).contains("Guiding Bolt"), "{}", t.screen());
}

#[test]
fn tab_switches_the_category() {
    let mut t = Tui::scenario("compendium.category", Home::campaign());
    t.send("1").wait("2041 results");
    t.send(TAB).wait("340 results");
    assert!(selected(&t).contains("Acid Arrow"), "{}", t.screen());
    t.send(TAB).wait("27 results").wait("Barbarian");
    t.send(BACKTAB).send(BACKTAB).wait("2041 results");
    // The last category, backwards from All.
    t.send(BACKTAB).wait("195 results").wait("Ability Check");
}

#[test]
fn o_orders_the_results() {
    let mut t = Tui::scenario("compendium.order", Home::campaign());
    t.send("1").wait("2041 results").send(TAB).wait("340 results");
    t.send("o").wait("name A–Z");
    assert!(selected(&t).contains("Acid Arrow"), "{}", t.screen());
    t.send("o").wait("name Z–A");
    assert!(selected(&t).contains("Zone of Truth"), "{}", t.screen());
    t.send("o").wait("· source");
    t.send("o").wait("level · rarity");
    // A cantrip first, a level 9 spell last.
    assert!(selected(&t).contains("Acid Splash"), "{}", t.screen());
    t.send("G");
    assert!(selected(&t).contains("Wish"), "{}", t.screen());
    t.send("o").wait_gone("level · rarity");
}

#[test]
fn an_entry_shows_its_fields_text_tables_and_lists() {
    let mut t = Tui::scenario("compendium.read", Home::campaign());
    search(&mut t, "proving rule");
    t.wait("1 results").send(ENTER);
    t.wait("Rule  ·  The Proving Grounds");
    t.wait("Roll 1d20 and add your modifier.");
    // The table, its caption and rows.
    t.wait("Outcomes");
    assert!(t.line("d6").contains("Result"), "{}", t.screen());
    assert!(t.line("1-3").contains("Fail"), "{}", t.screen());
    assert!(t.line("4-6").contains("Pass"), "{}", t.screen());
    // The list.
    t.wait("• First, look.").wait("• Then, leap.");

    t.send(ESC).send(ESC).send("/").send(&ctrl('u')).typed("guiding bolt").wait("1 results").send(ENTER);
    assert!(t.line("Casting Time").contains("1 action"), "{}", t.screen());
    assert!(t.line("Range").contains("120 feet"), "{}", t.screen());
    t.wait("You hurl a bolt of light toward a creature within range. Make a ranged spell attack against");
    t.wait("4d6 Radiant damage");
    t.wait("Using a Higher-Level Spell Slot");
}

/// The Barbarian's Persistent Rage, opened: links to Barbarian (its class),
/// Unconscious and Incapacitated.
fn persistent_rage(t: &mut Tui) {
    search(t, "persistent rage");
    t.wait("1 results").send(ENTER).wait("Your Rage ends early if you have the");
}

#[test]
fn following_a_link_and_going_back() {
    let mut t = Tui::scenario("compendium.follow-link", Home::campaign());
    persistent_rage(&mut t);
    t.send(TAB).send(TAB).send(ENTER);
    t.wait("While you have the Unconscious condition").wait("← Persistent Rage · 1 back");
    t.send(BACKSPACE).wait("Your Rage ends early");
    t.wait_gone("1 back");
    // Back where we were: on Unconscious. The next link, and back with h.
    t.send(TAB).send(ENTER).wait("While you have the Incapacitated condition");
    t.send("h").wait("Your Rage ends early");
}

#[test]
fn keys_move_between_links() {
    let mut t = Tui::scenario("compendium.link-keys", Home::campaign());
    persistent_rage(&mut t);
    // n moves forward: Barbarian, Unconscious, Incapacitated.
    t.send("n").send("n").send("n").send(ENTER);
    t.wait("While you have the Incapacitated condition").wait("← Persistent Rage");
    t.send(BACKSPACE).wait("Your Rage ends early");
    // N moves back from Incapacitated to Unconscious.
    t.send("N").send(ENTER).wait("While you have the Unconscious condition");
    t.send(&ctrl('o')).wait("Your Rage ends early");
    // Tab past the last link wraps to the first: Barbarian.
    t.send(TAB).send(TAB).send(ENTER).wait("Class  ·  System Reference Document 5.2.1");
    t.wait("← Persistent Rage");
    // Shift+Tab before the first wraps to the last.
    t.send(BACKSPACE).wait("Your Rage ends early").send(BACKTAB).send(ENTER);
    t.wait("While you have the Incapacitated condition");
}

#[test]
fn cloning_replaces_the_original_in_the_campaign() {
    let mut t = Tui::scenario("compendium.clone", Home::campaign());
    search(&mut t, "guiding bolt");
    t.wait("1 results").send(ESC);
    t.send("c").wait("Clone Guiding Bolt into the campaign").wait("Replace the original in this campaign");
    t.send(ENTER).wait("Guiding Bolt now has a campaign version").wait("Editing Guiding Bolt · campaign version");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("guiding bolt").wait("1 results").send(ENTER);
    t.wait("campaign version");
}

#[test]
fn cloning_as_a_copy_asks_for_a_new_name() {
    let mut t = Tui::scenario("compendium.clone", Home::campaign());
    search(&mut t, "guiding bolt");
    t.wait("1 results").send(ESC);
    t.send("c").wait("Separate copy…").send("j").send(ENTER).wait("Guiding Bolt (copy)");
    t.send(&ctrl('u')).typed("Guiding Spark").send(ENTER);
    t.wait("Editing Guiding Spark");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("#spell guiding").wait("2 results");
    t.wait("Guiding Bolt").wait("Guiding Spark");
    t.send(&ctrl('u')).typed("guiding spark").wait("1 results").send(ENTER);
    t.wait("Spell  ·  E2E (campaign)").wait("You hurl a bolt of light toward a creature within range.");
}

#[test]
fn o_switches_to_the_book_original_and_back() {
    let mut t = Tui::scenario("compendium.original", Home::campaign());
    search(&mut t, "guiding bolt");
    t.wait("1 results").send(ESC);
    t.send("c").send(ENTER).wait("Editing Guiding Bolt · campaign version");
    // time is the fourth field.
    t.send("j").send("j").send("j").send(ENTER).wait("❯ 1 action");
    t.send(&ctrl('u')).typed("1 bonus action").send(ENTER).wait("Saved time");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("guiding bolt").wait("1 results").send(ENTER);
    t.wait("campaign version");
    assert!(t.line("Casting Time").contains("1 bonus action"), "{}", t.screen());
    t.send("o").wait("book version");
    assert!(t.line("Casting Time").contains("1 action"), "{}", t.screen());
    assert!(!t.line("Casting Time").contains("bonus"), "{}", t.screen());
    t.send("o").wait("campaign version");
    assert!(t.line("Casting Time").contains("1 bonus action"), "{}", t.screen());
}

#[test]
fn n_creates_a_homebrew_entry_from_a_template() {
    let mut t = Tui::scenario("compendium.new-homebrew", Home::campaign());
    t.send("1").wait("Browse");
    t.send("n").wait("Create a campaign entry").wait("Magic item");
    // Magic item, Weapon, Armor, Adventuring gear, Spell.
    t.send("j").send("j").send("j").send("j").send(ENTER);
    t.typed("Frost Lance").send(ENTER);
    t.wait("Created Frost Lance").wait("Editing Frost Lance · campaign entry");
    assert!(t.line("● level").contains("Level 1"), "{}", t.screen());
    t.send(ESC).wait("Browse");
    t.send("/").typed("frost lance").wait("1 results").send(ENTER);
    t.wait("Spell  ·  E2E (campaign)").wait("Describe what the spell does.");
}
