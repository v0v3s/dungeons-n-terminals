use crate::harness::*;
use crate::seed::Home;

/// A new homebrew spell, "Frost Lance", open in the entry editor. Its
/// fields: name, level, school, time, range, components, duration, …
fn new_spell(t: &mut Tui) {
    t.send("1").wait("Browse");
    t.send("n").wait("Create a campaign entry");
    // Magic item, Weapon, Armor, Adventuring gear, Spell.
    t.send("j").send("j").send("j").send("j").send(ENTER);
    t.typed("Frost Lance").send(ENTER).wait("Editing Frost Lance · campaign entry");
}

/// Close the editor and read `name` in the compendium.
fn read(t: &mut Tui, name: &str) {
    t.send(ESC).wait("Browse");
    t.send("/").send(&ctrl('u')).typed(name).wait("1 results").send(ENTER);
}

/// Replace the text of the open JSON editor (vim keys) and save it.
fn replace_json(t: &mut Tui, json: &str) {
    t.wait("NORMAL").send("d").send("d").send("i").typed(json).send(ESC).send(ESC);
}

#[test]
fn editing_a_field_shows_in_the_compendium() {
    let mut t = Tui::scenario("homebrew.edit-field", Home::campaign());
    new_spell(&mut t);
    t.send("j").send("j").send("j").send("j").send(ENTER).wait("❯ 60 feet");
    t.send(&ctrl('u')).typed("120 feet").send(ENTER).wait("Saved range");
    assert!(t.line("range").contains("120 feet"), "{}", t.screen());
    read(&mut t, "frost lance");
    t.wait("Spell  ·  E2E (campaign)");
    assert!(t.line("Range").contains("120 feet"), "{}", t.screen());
}

#[test]
fn editing_a_field_as_json() {
    let mut t = Tui::scenario("homebrew.edit-json", Home::campaign());
    new_spell(&mut t);
    t.send("J").wait("name · JSON").wait("\"Frost Lance\"");
    replace_json(&mut t, "\"Frost Spear\"");
    t.wait("Saved name").wait("Editing Frost Spear");
    read(&mut t, "frost spear");
    t.wait("Spell  ·  E2E (campaign)");
}

#[test]
fn json_that_does_not_parse_is_not_saved() {
    let mut t = Tui::scenario("homebrew.edit-json", Home::campaign());
    new_spell(&mut t);
    t.send("J").wait("\"Frost Lance\"");
    replace_json(&mut t, "\"Frost");
    // Still in the JSON editor, the entry unchanged.
    t.wait("name · JSON").wait("Not valid JSON");
    assert!(t.line("Preview").contains("name · JSON"), "{}", t.screen());
    t.wait("Frost Lance");
    t.send("d").send("d").send("i").typed("\"Frost Lance\"").send(ESC).send(ESC).wait("Editing Frost Lance");
}

#[test]
fn editing_the_whole_entry_in_the_editor() {
    let mut t = Tui::with_editor("homebrew.edit-json", Home::campaign(), "Describe what the spell does.", "Frost bites.");
    new_spell(&mut t);
    t.send("E").wait_after_editor("Frost bites.");
    read(&mut t, "frost lance");
    t.wait("Frost bites.");
    assert!(!t.shows("Describe what the spell does."), "{}", t.screen());
}

#[test]
fn x_reverts_a_field_to_the_book() {
    let mut t = Tui::scenario("homebrew.revert-field", Home::campaign());
    t.send("1").wait("Browse").send("/").typed("ember bolt").wait("1 results").send(ESC);
    t.send("c").send(ENTER).wait("Editing Ember Bolt · campaign version");
    // time is the fourth field.
    t.send("j").send("j").send("j").send(ENTER).wait("❯ 1 action");
    t.send(&ctrl('u')).typed("1 bonus action").send(ENTER).wait("Saved time");
    assert!(t.line(" time ").starts_with(" │ ● time"), "{}", t.screen());
    t.send("x");
    t.wait_until("time back to the book's", |s| s.lines().any(|l| l.contains(" time ") && l.contains("1 action") && !l.contains("bonus")));
    assert!(!t.line(" time ").contains('●'), "{}", t.screen());
    read(&mut t, "ember bolt");
    assert!(t.line("Casting Time").contains("1 action"), "{}", t.screen());
    assert!(!t.line("Casting Time").contains("bonus"), "{}", t.screen());
}

#[test]
fn a_adds_a_field() {
    let mut t = Tui::scenario("homebrew.add-field", Home::campaign());
    new_spell(&mut t);
    t.send("a").typed("page").send(ENTER).wait("page · JSON");
    replace_json(&mut t, "42");
    t.wait("Saved page");
    read(&mut t, "frost lance");
    t.wait("Spell  ·  E2E (campaign) · p. 42");
}

#[test]
fn promoting_makes_an_entry_global() {
    let mut t = Tui::scenario("homebrew.promote", Home::campaign());
    new_spell(&mut t);
    t.send("P").wait("Frost Lance is now global homebrew, available in every campaign");
    t.wait("Editing Frost Lance · global homebrew");
    t.send(ESC).wait("Browse");
    // A new campaign has it too.
    t.send("4").wait("Campaigns").send("n").typed("Second").send(ENTER).wait("◈ Second");
    t.send("1").wait("Browse").send("/").typed("frost lance").wait("1 results").send(ENTER);
    t.wait("Describe what the spell does.");
}

#[test]
fn deleting_asks_first() {
    let mut t = Tui::scenario("homebrew.delete", Home::campaign());
    new_spell(&mut t);
    // Any other key keeps it.
    t.send("D").send("n").wait("Editing Frost Lance");
    t.send("D").send("y").wait("Browse").wait_gone("Editing Frost Lance");
    t.send("/").typed("frost lance").wait("0 results").wait("Nothing matches.");
}

#[test]
fn a_new_item_from_the_inventory_is_carried() {
    let mut t = Tui::scenario("inventory.homebrew-item", Home::party());
    t.send("2").wait("Brawler").send("G").send(ENTER).wait("Kindred · Archivist · Mage 1");
    t.send(TAB).send(TAB).send(TAB).wait("Grimoire");
    t.send("a").wait("Add an item").wait("Create new item…").send(ENTER);
    t.wait("Create a new item").wait("Adventuring gear");
    // Magic item, Weapon.
    t.send("j").send(ENTER).typed("Frost Pick").send(ENTER);
    t.wait("Created Frost Pick and added it to Tester").wait("Editing Frost Pick · campaign entry");
    // Back on the sheet, the item is carried.
    t.send(ESC).wait("Kindred · Archivist · Mage 1");
    assert!(t.line("│    Frost Pick").contains(" 1 "), "{}", t.screen());
    t.quit();
    let saved = std::fs::read_dir(t.home.campaign_file("characters"))
        .unwrap()
        .map(|e| std::fs::read_to_string(e.unwrap().path()).unwrap_or_default())
        .find(|s| s.contains("Tester"))
        .expect("Tester's file");
    assert!(saved.contains("Frost Pick"), "{saved}");
}
