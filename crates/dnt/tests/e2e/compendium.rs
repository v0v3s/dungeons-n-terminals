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
    t.wait("Glass Shield").wait("Glimmer");
    t.typed("im").wait("1 results").wait("An object sheds dim light.");
    assert!(!t.shows("Glass Shield"), "{}", t.screen());
    t.send(BACKSPACE).send(BACKSPACE).wait("Glass Shield");
    t.send(&ctrl('u')).wait("203 results");
}

#[test]
fn filters_narrow_a_search() {
    let mut t = Tui::scenario("compendium.filters", Home::campaign());
    search(&mut t, "@TSX");
    t.wait("3 results").wait("Mirefolk").wait("Old Lantern").wait("Outskirt Gust");
    assert!(!t.shows("Glimmer"), "{}", t.screen());

    t.send(&ctrl('u')).typed("#spell ^1").wait("7 results").wait("Ember Bolt").wait("Outskirt Gust");
    assert!(!t.shows("Glimmer") && !t.shows("Mist Step"), "{}", t.screen());

    t.typed(" !@TSX").wait("6 results").wait_gone("Outskirt Gust");

    t.send(&ctrl('u')).typed("\"sheds\"").wait("1 results");
    assert!(selected(&t).contains("Glimmer"), "{}", t.screen());
}

#[test]
fn tab_switches_the_category() {
    let mut t = Tui::scenario("compendium.category", Home::campaign());
    t.send("1").wait("203 results");
    t.send(TAB).wait("18 results");
    assert!(selected(&t).contains("Ember Bolt"), "{}", t.screen());
    t.send(TAB).wait("10 results");
    t.send(BACKTAB).send(BACKTAB).wait("203 results");
    // The last category, backwards from All.
    t.send(BACKTAB).wait("29 results").wait("Proving Rule");
}

#[test]
fn o_orders_the_results() {
    let mut t = Tui::scenario("compendium.order", Home::campaign());
    t.send("1").wait("203 results").send(TAB).wait("18 results");
    t.send("o").wait("name A–Z");
    assert!(selected(&t).contains("Ember Bolt"), "{}", t.screen());
    t.send("o").wait("name Z–A");
    assert!(selected(&t).contains("Ward of Thorns"), "{}", t.screen());
    t.send("o").wait("· source");
    t.send("o").wait("level · rarity");
    assert!(selected(&t).contains("Frost Needle"), "{}", t.screen());
    t.send("G");
    assert!(selected(&t).contains("Unmaking Word"), "{}", t.screen());
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

    t.send(ESC).send(ESC).send("/").send(&ctrl('u')).typed("ember bolt").wait("1 results").send(ENTER);
    assert!(t.line("Casting Time").contains("1 action"), "{}", t.screen());
    assert!(t.line("Range").contains("60 feet"), "{}", t.screen());
    t.wait("Make a ranged spell attack. On a hit: 3d6 fire damage.");
    t.wait("Using a Higher-Level Spell Slot");
}

/// The Mage's Spellcasting feature, opened: links to Mage (its class),
/// Grimoire and Proving Rule.
fn spellcasting(t: &mut Tui) {
    search(t, "spellcasting \"grimoire\"");
    t.wait("1 results").send(ENTER).wait("You cast mage spells from your Grimoire. See Proving Rule.");
}

#[test]
fn following_a_link_and_going_back() {
    let mut t = Tui::scenario("compendium.follow-link", Home::campaign());
    spellcasting(&mut t);
    t.send(TAB).send(TAB).send(ENTER);
    t.wait("A mage's spellbook.").wait("← Spellcasting · 1 back");
    t.send(BACKSPACE).wait("You cast mage spells from your Grimoire.");
    t.wait_gone("1 back");
    // Back where we were: on Grimoire. The next link, and back with h.
    t.send(TAB).send(ENTER).wait("Roll 1d20 and add your modifier.");
    t.send("h").wait("You cast mage spells from your Grimoire.");
}

#[test]
fn keys_move_between_links() {
    let mut t = Tui::scenario("compendium.link-keys", Home::campaign());
    spellcasting(&mut t);
    // n moves forward: Mage, Grimoire, Proving Rule.
    t.send("n").send("n").send("n").send(ENTER);
    t.wait("Roll 1d20 and add your modifier.").wait("← Spellcasting");
    t.send(BACKSPACE).wait("You cast mage spells");
    // N moves back from Proving Rule to Grimoire.
    t.send("N").send(ENTER).wait("A mage's spellbook.");
    t.send(&ctrl('o')).wait("You cast mage spells");
    // Tab past the last link wraps to the first: Mage.
    t.send(TAB).send(TAB).send(ENTER).wait("Class  ·  The Proving Grounds");
    t.wait("← Spellcasting");
    // Shift+Tab before the first wraps to the last.
    t.send(BACKSPACE).wait("You cast mage spells").send(BACKTAB).send(ENTER);
    t.wait("Roll 1d20 and add your modifier.");
}

#[test]
fn cloning_replaces_the_original_in_the_campaign() {
    let mut t = Tui::scenario("compendium.clone", Home::campaign());
    search(&mut t, "ember bolt");
    t.wait("1 results").send(ESC);
    t.send("c").wait("Clone Ember Bolt into the campaign").wait("Replace the original in this campaign");
    t.send(ENTER).wait("Ember Bolt now has a campaign version").wait("Editing Ember Bolt · campaign version");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("ember bolt").wait("1 results").send(ENTER);
    t.wait("campaign version");
}

#[test]
fn cloning_as_a_copy_asks_for_a_new_name() {
    let mut t = Tui::scenario("compendium.clone", Home::campaign());
    search(&mut t, "ember bolt");
    t.wait("1 results").send(ESC);
    t.send("c").wait("Separate copy…").send("j").send(ENTER).wait("Ember Bolt (copy)");
    t.send(&ctrl('u')).typed("Ember Dart").send(ENTER);
    t.wait("Editing Ember Dart");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("#spell ember").wait("2 results");
    t.wait("Ember Bolt").wait("Ember Dart");
    t.send(&ctrl('u')).typed("ember dart").wait("1 results").send(ENTER);
    t.wait("Spell  ·  E2E (campaign)").wait("Make a ranged spell attack. On a hit: 3d6 fire damage.");
}

#[test]
fn o_switches_to_the_book_original_and_back() {
    let mut t = Tui::scenario("compendium.original", Home::campaign());
    search(&mut t, "ember bolt");
    t.wait("1 results").send(ESC);
    t.send("c").send(ENTER).wait("Editing Ember Bolt · campaign version");
    // time is the fourth field.
    t.send("j").send("j").send("j").send(ENTER).wait("❯ 1 action");
    t.send(&ctrl('u')).typed("1 bonus action").send(ENTER).wait("Saved time");
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed("ember bolt").wait("1 results").send(ENTER);
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
