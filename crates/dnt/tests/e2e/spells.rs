use crate::harness::*;
use crate::seed::Home;

/// Open a character of the party (roster order) on the Spells tab.
fn spells_of(t: &mut Tui, name: &str) {
    let at = ["Brawler", "Hexa", "Ranger", "Tester"].iter().position(|n| *n == name).unwrap();
    t.send("2").wait(name);
    t.send(&"j".repeat(at)).send(ENTER).wait(&format!("◆ {name}"));
    t.send(TAB).send(TAB).wait("╭ Spells");
}

/// The spell the detail pane shows (the one under the cursor).
fn shown(t: &Tui) -> String {
    shown_on(&t.screen())
}

fn shown_on(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let top = lines.iter().position(|l| l.contains("╭ Spells")).expect("the spell list");
    lines[top + 1..top + 3].iter().filter_map(|l| l.split('│').nth(3)).map(str::trim).find(|s| !s.is_empty()).unwrap_or("").to_string()
}

/// Move the cursor down onto a spell.
#[track_caller]
fn to_spell(t: &mut Tui, name: &str) {
    t.send("g");
    // The first row is a header: its pane shows the legend.
    t.wait_until("the cursor is at the top", |s| s.contains("◦ cantrip   ● prepared/known"));
    for _ in 0..20 {
        let here = shown(t);
        if here == name {
            return;
        }
        t.send("j");
        // Wait for the pane to follow the cursor (headers show the same legend).
        let start = std::time::Instant::now();
        while shown(t) == here && start.elapsed() < std::time::Duration::from_millis(300) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    panic!("[{}] no spell {name:?} to move to; the screen:\n{}", t.scenario, t.screen());
}

/// Add a spell to the class under the cursor through its picker.
fn add(t: &mut Tui, filter: &str, name: &str) {
    t.send("a").wait("add a cantrip or spellbook spell");
    t.typed(filter).send(ENTER).wait(&format!("Added {name}"));
}

#[test]
fn a_mage_adds_cantrips_and_spellbook_spells() {
    let mut t = Tui::scenario("spells.add", Home::party());
    spells_of(&mut t, "Tester");
    t.wait("cantrips 0/3  prepared 0/4  book 0/6");
    t.send("a").wait("Mage 1: add a cantrip or spellbook spell").wait("Ember Bolt").wait("Frost Needle");
    // A spell of another class is not offered.
    assert!(!t.shows("Hunter's Brand"), "{}", t.screen());
    t.typed("ember").send(ENTER).wait("Added Ember Bolt");
    assert!(t.line("○ Ember Bolt").contains("spellbook"), "{}", t.screen());
    add(&mut t, "frost", "Frost Needle");
    assert!(t.shows("◦ Frost Needle"), "{}", t.screen());
    t.wait("cantrips 1/3  prepared 0/4  book 1/6");
    // A cantrip fills a gap; a spellbook spell does once prepared.
    t.wait("6 spell(s) to choose");
}

#[test]
fn a_grant_chooses_its_spells() {
    let mut t = Tui::scenario("spells.choose", Home::party());
    spells_of(&mut t, "Hexa");
    t.wait("Spark Initiate; Mage").wait("+ choose 2 Mage cantrip");
    t.send("G").send(ENTER).wait("Choose 2 Mage cantrip");
    t.typed("frost").send(" ").send(&ctrl('u')).typed("spark").send(" ").send(ENTER);
    t.wait("Added Frost Needle, Spark").wait_gone("+ choose 2 Mage cantrip");
    // The chosen cantrips are listed under the feat that grants them.
    let s = t.screen();
    let feat = s.find("Spark Initiate; Mage").unwrap();
    assert!(s[feat..].contains("◆ Frost Needle") && s[feat..].contains("◆ Spark"), "{s}");
}

#[test]
fn a_class_header_chooses_the_class_spells() {
    let mut t = Tui::scenario("spells.choose", Home::party());
    spells_of(&mut t, "Hexa");
    t.wait("cantrips 0/2  known 0/2");
    t.send("g").send(ENTER).wait("Hexer 1: add");
    t.typed("hex bolt").send(ENTER).wait("Added Hex Bolt");
    t.wait("cantrips 1/2  known 0/2");
}

#[test]
fn preparing_a_spellbook_spell_and_unpreparing_it() {
    let mut t = Tui::scenario("spells.prepare", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "ember", "Ember Bolt");
    to_spell(&mut t, "Ember Bolt");
    t.send("p").wait("Prepared Ember Bolt").wait("prepared 1/4  book 1/6").wait("● Ember Bolt");
    t.send("p").wait("prepared 0/4  book 1/6").wait("○ Ember Bolt");
}

#[test]
fn casting_spends_a_slot_and_notes_concentration() {
    let mut t = Tui::scenario("spells.cast", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "ward of", "Ward of Thorns");
    to_spell(&mut t, "Ward of Thorns");
    t.send("p").wait("Prepared Ward of Thorns");
    assert!(t.line("Slots").contains("1 ●●"), "{}", t.screen());
    t.send(ENTER).wait("Cast Ward of Thorns").wait("Level 1 slot");
    assert!(t.line("Level 1 slot").contains("2 left"), "{}", t.screen());
    t.send(ENTER).wait("◎ concentrating on Ward of Thorns");
    assert!(t.line("Slots").contains("1 ●○"), "{}", t.screen());
    // The overview tracks the slot too.
    t.send(BACKTAB).send(BACKTAB).wait("Spell slots");
    assert!(t.line("●○").contains("1/2"), "{}", t.screen());
    t.send(TAB).send(TAB).wait("╭ Spells").send("C").wait("Stopped concentrating on Ward of Thorns").wait_gone("◎ concentrating");
}

#[test]
fn a_cantrip_needs_no_slot() {
    let mut t = Tui::scenario("spells.cast", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "frost", "Frost Needle");
    to_spell(&mut t, "Frost Needle");
    t.send(ENTER).wait("Frost Needle is a cantrip: no slot needed");
    assert!(t.line("Slots").contains("1 ●●"), "{}", t.screen());
}

#[test]
fn an_innate_spell_is_cast_free_and_comes_back_on_a_rest() {
    let mut t = Tui::scenario("spells.cast-free", Home::party());
    spells_of(&mut t, "Hexa");
    // Emberkin cast Ember Bolt once a day from level 3.
    assert!(!t.shows("Ember Bolt"), "{}", t.screen());
    for level in ["Hexer 2", "Hexer 3"] {
        t.send("u").wait("Level up: choose a class").send(ENTER).wait("Hit points for this level");
        t.send(ENTER).wait("Confirm level up").send(ENTER).wait(level);
    }
    t.wait("Ember Bolt");
    assert!(t.line("◆ Ember Bolt").contains("1/1 free"), "{}", t.screen());
    let slots = t.line("Slots");
    to_spell(&mut t, "Ember Bolt");
    t.send(ENTER).wait_until("the free use is spent", |s| s.contains("0/1 free"));
    // No pact slot is spent.
    assert_eq!(t.line("Slots"), slots, "{}", t.screen());
    t.send("L").wait("Take a long rest?").send("y").wait_until("the free use is back", |s| s.contains("1/1 free"));
}

#[test]
fn a_favorite_is_listed_on_top() {
    let mut t = Tui::scenario("spells.favorite", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "frost", "Frost Needle");
    to_spell(&mut t, "Frost Needle");
    t.send("f").wait("Frost Needle is a favorite").wait("★ Favorites");
    assert!(t.line("Frost Needle ★").contains("◦ Frost Needle ★"), "{}", t.screen());
    t.send("f").wait_gone("★ Favorites");
    assert!(!t.shows("Frost Needle ★"), "{}", t.screen());
}

#[test]
fn removing_a_spell() {
    let mut t = Tui::scenario("spells.remove", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "frost", "Frost Needle");
    add(&mut t, "ember", "Ember Bolt");
    to_spell(&mut t, "Frost Needle");
    t.send("x").wait("Removed Frost Needle").wait("cantrips 0/3  prepared 0/4  book 1/6");
    assert!(!t.screen().contains("◦ Frost Needle"), "{}", t.screen());
    assert!(t.shows("○ Ember Bolt"), "{}", t.screen());
}

#[test]
fn rolling_a_spell_uses_its_class_numbers() {
    let mut t = Tui::scenario("spells.roll", Home::party());
    spells_of(&mut t, "Tester");
    t.wait("DC 12 · +4");
    add(&mut t, "frost", "Frost Needle");
    add(&mut t, "spark", "Spark");
    // An attack: the class's +4, and damage.
    to_spell(&mut t, "Frost Needle");
    t.send("r").wait("Frost Needle: ");
    let line = t.line("Frost Needle: ");
    assert!(line.contains("to hit (") && line.contains(" + 4 = ") && line.contains("cold damage"), "{line}");
    // A save: the class's DC, and damage.
    to_spell(&mut t, "Spark");
    t.send("r").wait("Spark: ");
    let line = t.line("Spark: ");
    assert!(line.contains("DC 12 (Dexterity save)") && line.contains("lightning damage"), "{line}");
}
