use crate::harness::*;
use crate::seed::Home;

#[test]
fn the_roster_lists_the_party_with_a_preview() {
    let mut t = Tui::scenario("roster.list", Home::party());
    t.send("2").wait("Brawler").wait("Hexa").wait("Ranger").wait("Tester");
    assert!(t.line("Brawler").contains("Lv 1"), "{}", t.screen());
    t.wait("Stoneling · Drifter · Monk 1");
    t.send("j").wait("Ash Lineage · Acolyte of Gears · Hexer 1");
    t.send("G").wait("Kindred · Archivist · Mage 1");
}
