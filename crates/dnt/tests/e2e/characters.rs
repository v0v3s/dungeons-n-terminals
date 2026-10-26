use crate::harness::*;
use crate::seed::Home;
use crate::steps::see;

#[test]
fn the_roster_lists_the_party_with_a_preview() {
    let mut t = Tui::scenario("roster.list", Home::party());
    t.send("2").wait("Brawler").wait("Hexa").wait("Ranger").wait("Tester");
    assert!(t.line("Brawler").contains("Lv 1"), "{}", t.screen());
    t.wait("Stoneling · Drifter · Monk 1");
    t.send("j").wait("Ash Lineage · Acolyte of Gears · Hexer 1");
    t.send("G").wait("Kindred · Archivist · Mage 1");
}

#[test]
fn editing_the_file_changes_the_sheet() {
    let mut t = Tui::with_editor("character.edit-file", Home::party(), "Tester", "Edited");
    t.send("2").send("G").wait("Kindred · Archivist · Mage 1");
    // The roster reads the edited file: Tester is now Edited.
    t.send("e").wait_after_editor("◆ Edited");
    assert!(!t.shows("◆ Tester"), "{}", t.screen());
}

#[test]
fn deleting_a_character_asks_first() {
    let mut t = Tui::scenario("roster.delete", Home::party());
    t.send("2").wait("Brawler");
    t.send("D");
    see(&mut t, "Delete Brawler?");
    see(&mut t, "y delete any key keep");
    // Any other key keeps it.
    t.send("n").wait_gone("Delete Brawler?");
    assert!(t.line("Brawler").contains("Lv 1"), "{}", t.screen());
    assert!(t.home.campaign_file("characters/brawler.toml").exists());
    t.send("D");
    see(&mut t, "Delete Brawler?");
    t.send("y").wait("Deleted Brawler");
    t.wait("Hexa").wait("Tester");
    assert!(!t.shows("◆ Brawler"), "{}", t.screen());
    assert!(!t.home.campaign_file("characters/brawler.toml").exists());
}

#[test]
fn the_sheet_is_exported_as_markdown_and_html() {
    let mut t = Tui::scenario("character.export", Home::party());
    t.send("2").send("G").send(ENTER).wait("Kindred · Archivist · Mage 1");
    t.send("X");
    see(&mut t, "Exported");
    // exports/ or exports\ on Windows.
    see(&mut t, "tester.md and");
    t.quit();
    let md = std::fs::read_to_string(t.home.campaign_file("exports/tester.md")).expect("the Markdown");
    let html = std::fs::read_to_string(t.home.campaign_file("exports/tester.html")).expect("the HTML");
    for text in ["Tester", "Mage", "Thornknife"] {
        assert!(md.contains(text), "{text} in the Markdown:\n{md}");
        assert!(html.contains(text), "{text} in the HTML:\n{html}");
    }
    assert!(html.to_lowercase().contains("<html"), "{html}");
}

#[test]
fn the_characters_note_opens_from_the_sheet() {
    let mut t = Tui::scenario("character.note", Home::party());
    t.send("2").send("G").send(ENTER).wait("Kindred · Archivist · Mage 1");
    for _ in 0..4 {
        t.send(TAB);
    }
    t.wait("Party/Tester").wait("No notes for this character yet.");
    t.send(ENTER);
    see(&mut t, "Notes for Tester");
    see(&mut t, "Notes · E2E");
    t.wait("# Tester").wait("## Backstory").wait("## Goals");
    see(&mut t, "character Tester");
    t.quit();
    let note = std::fs::read_to_string(t.home.campaign_file("notes/Party/Tester.md")).expect("the note");
    assert!(note.contains("character: Tester"), "{note}");
}

#[test]
fn what_the_rules_cannot_work_out_is_shown() {
    let mut t = Tui::scenario("character.warnings", Home::party());
    t.send("2").send("G").send(ENTER).wait("Kindred · Archivist · Mage 1");
    assert!(!t.shows("warning"), "{}", t.screen());
    // A Hexer level without the Charisma for it.
    t.send("u");
    see(&mut t, "New class: Hexer ⚠ needs Charisma 13");
    t.typed("Hexer");
    t.send(ENTER);
    see(&mut t, "Choose a subclass");
    t.send(ENTER);
    see(&mut t, "Hit points for this level");
    t.send(ENTER);
    see(&mut t, "Confirm level up");
    t.send(ENTER);
    see(&mut t, "Mage 1 / Hexer 1 · Level 2");
    see(&mut t, "⚠ 1 warning(s)");
    see(&mut t, "Warnings");
    see(&mut t, "⚠ multiclassing Hexer requires Charisma 13");
    // The roster says so too.
    t.send("q").wait("Brawler");
    see(&mut t, "⚠ 1 warning(s) — open to see");
}
