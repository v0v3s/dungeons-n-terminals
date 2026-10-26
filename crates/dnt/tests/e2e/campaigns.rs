use crate::harness::*;
use crate::seed::Home;

/// The campaign's row in the list (not the header).
#[track_caller]
fn row(t: &Tui, name: &str) -> String {
    let s = t.screen();
    s.lines().find(|l| l.starts_with(" │") && l.contains(name)).unwrap_or_else(|| panic!("no campaign {name}:\n{s}")).to_string()
}

/// A new campaign named `name`, made with keys on the Campaigns screen.
fn new_campaign(t: &mut Tui, name: &str) {
    t.send("4").send("n").wait("New campaign");
    t.typed(name).send(ENTER).wait("Created campaign");
}

#[test]
fn another_campaign_is_created_and_listed() {
    let mut t = Tui::scenario("campaigns.create", Home::campaign());
    new_campaign(&mut t, "Second Run");
    t.wait("Books · Second Run");
    assert!(row(&t, "◈ Second Run").contains("active"), "{}", t.screen());
    assert!(!row(&t, "E2E").contains("active"), "{}", t.screen());
}

#[test]
fn an_empty_name_creates_nothing() {
    let mut t = Tui::scenario("campaigns.create", Home::campaign());
    t.send("4").send("n").wait("New campaign");
    t.send(ENTER).wait_gone("New campaign");
    assert!(!t.shows("Created campaign"), "{}", t.screen());
}

#[test]
fn activating_a_campaign_shows_its_characters_and_notes() {
    let mut t = Tui::scenario("campaigns.activate", Home::party());
    new_campaign(&mut t, "Other");
    t.send("2");
    assert!(!t.shows("Tester"), "the new campaign has no characters: {}", t.screen());
    t.send("4").send("k").send(ENTER).wait("Active campaign: E2E");
    assert!(row(&t, "◈ E2E").contains("active"), "{}", t.screen());
    assert!(!row(&t, "Other").contains("active"), "{}", t.screen());
    t.send("2").wait("Brawler").wait("Tester");
    t.send("3").wait("Notes · E2E");
    t.send("4").send("j").send(ENTER).wait("Active campaign: Other");
    t.send("3").wait("Notes · Other");
}

#[test]
fn a_renamed_campaign_stays_active() {
    let mut t = Tui::scenario("campaigns.rename", Home::party());
    t.send("4").send("r").wait("Rename the campaign");
    t.send(BACKSPACE).send(BACKSPACE).send(BACKSPACE).typed("Grand Tour").send(ENTER);
    t.wait("Renamed the campaign to Grand Tour");
    assert!(row(&t, "◈ Grand Tour").contains("active"), "{}", t.screen());
    assert!(t.line(" dnt ").contains("◈ Grand Tour"), "{}", t.screen());
    t.send("2").wait("Tester");
}

#[test]
fn deleting_a_campaign_asks_first() {
    let mut t = Tui::scenario("campaigns.delete", Home::party());
    new_campaign(&mut t, "Doomed");
    t.send("D").wait("Delete Doomed?");
    // Any other key keeps it.
    t.send("n").wait_gone("Delete Doomed?");
    row(&t, "Doomed");
    t.send("D").wait("Delete Doomed?");
    t.send("y").wait("Deleted Doomed");
    assert!(!t.screen().lines().any(|l| l.starts_with(" │") && l.contains("Doomed")), "{}", t.screen());
    row(&t, "E2E");
    t.send("2").wait("Tester");
}
