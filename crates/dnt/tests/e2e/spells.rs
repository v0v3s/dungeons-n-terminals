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
    t.send("a").wait("Wizard 1: add a cantrip or spellbook spell").wait("Burning Hands").wait("Ray of Frost");
    // A spell of another class is not offered.
    assert!(!t.shows("Hunter's Mark"), "{}", t.screen());
    t.typed("burning").send(ENTER).wait("Added Burning Hands");
    assert!(t.line("○ Burning Hands").contains("spellbook"), "{}", t.screen());
    add(&mut t, "ray of frost", "Ray of Frost");
    assert!(t.shows("◦ Ray of Frost"), "{}", t.screen());
    t.wait("cantrips 1/3  prepared 0/4  book 1/6");
    // A cantrip fills a gap; a spellbook spell does once prepared.
    t.wait("9 spell(s) to choose");
}

#[test]
fn a_grant_chooses_its_spells() {
    let mut t = Tui::scenario("spells.choose", Home::party());
    spells_of(&mut t, "Hexa");
    t.wait("Magic Initiate; Cleric").wait("+ choose 2 Cleric cantrip");
    // The class's 2 cantrips and 2 spells, and the feat's 2 cantrips and 1 spell.
    t.wait("7 spell(s) to choose");
    t.send("G").send("k").send(ENTER).wait("Choose 2 Cleric cantrip");
    t.typed("guidance").send(" ").send(&ctrl('u')).typed("sacred").send(" ").send(ENTER);
    t.wait("Added Guidance, Sacred Flame").wait_gone("+ choose 2 Cleric cantrip");
    t.wait("5 spell(s) to choose");
    // The chosen cantrips are listed under the feat that grants them.
    let s = t.screen();
    let feat = s.find("Magic Initiate; Cleric").unwrap();
    assert!(s[feat..].contains("◆ Guidance") && s[feat..].contains("◆ Sacred Flame"), "{s}");
}

#[test]
fn a_class_header_chooses_the_class_spells() {
    let mut t = Tui::scenario("spells.choose", Home::party());
    spells_of(&mut t, "Hexa");
    t.wait("cantrips 0/2  prepared 0/2");
    t.send("g").send(ENTER).wait("Warlock 1: add");
    t.typed("eldritch").send(ENTER).wait("Added Eldritch Blast");
    t.wait("cantrips 1/2  prepared 0/2");
}

#[test]
fn preparing_a_spellbook_spell_and_unpreparing_it() {
    let mut t = Tui::scenario("spells.prepare", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "burning", "Burning Hands");
    to_spell(&mut t, "Burning Hands");
    t.send("p").wait("Prepared Burning Hands").wait("prepared 1/4  book 1/6").wait("● Burning Hands");
    t.send("p").wait("prepared 0/4  book 1/6").wait("○ Burning Hands");
}

#[test]
fn casting_spends_a_slot_and_notes_concentration() {
    let mut t = Tui::scenario("spells.cast", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "fog cloud", "Fog Cloud");
    to_spell(&mut t, "Fog Cloud");
    t.send("p").wait("Prepared Fog Cloud");
    assert!(t.line("Slots").contains("1 ●●"), "{}", t.screen());
    t.send(ENTER).wait("Cast Fog Cloud").wait("Level 1 slot");
    assert!(t.line("Level 1 slot").contains("2 left"), "{}", t.screen());
    t.send(ENTER).wait("◎ concentrating on Fog Cloud");
    assert!(t.line("Slots").contains("1 ●○"), "{}", t.screen());
    // The overview tracks the slot too.
    t.send(BACKTAB).send(BACKTAB).wait("Spell slots");
    assert!(t.line("●○").contains("1/2"), "{}", t.screen());
    t.send(TAB).send(TAB).wait("╭ Spells").send("C").wait("Stopped concentrating on Fog Cloud").wait_gone("◎ concentrating");
}

#[test]
fn a_cantrip_needs_no_slot() {
    let mut t = Tui::scenario("spells.cast", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "ray of frost", "Ray of Frost");
    to_spell(&mut t, "Ray of Frost");
    t.send(ENTER).wait("Ray of Frost is a cantrip: no slot needed");
    assert!(t.line("Slots").contains("1 ●●"), "{}", t.screen());
}

#[test]
fn an_innate_spell_is_cast_free_and_comes_back_on_a_rest() {
    let mut t = Tui::scenario("spells.cast-free", Home::party());
    spells_of(&mut t, "Hexa");
    // A High Elf casts Detect Magic once a day from level 3.
    assert!(!t.shows("Detect Magic"), "{}", t.screen());
    // Warlock 2 takes two more invocations; Warlock 3 a patron.
    t.send("u").wait("Level up: choose a class").send(ENTER).wait("Hit points for this level");
    t.send(ENTER).wait("Eldritch Invocations: choose 2");
    t.typed("fiendish").send(" ").send(&ctrl('u')).typed("mask").send(" ").send(ENTER);
    t.wait("Confirm level up").send(ENTER).wait("Warlock 2");
    t.send("u").wait("Level up: choose a class").send(ENTER).wait("Choose a subclass");
    t.send(ENTER).wait("Hit points for this level");
    t.send(ENTER).wait("Confirm level up").send(ENTER).wait("Warlock 3");
    t.wait("Detect Magic");
    assert!(t.line("◆ Detect Magic").contains("1/1 free"), "{}", t.screen());
    // The slots, without the concentration noted on the same line.
    let slots = |t: &Tui| t.line("Slots").split('◎').next().unwrap().trim_end().to_string();
    let before = slots(&t);
    assert!(before.contains("pact 2 ●●"), "{}", t.screen());
    to_spell(&mut t, "Detect Magic");
    t.send(ENTER).wait_until("the free use is spent", |s| s.contains("0/1 free"));
    // No pact slot is spent.
    assert_eq!(slots(&t), before, "{}", t.screen());
    t.send("L").wait("Take a long rest?").send("y").wait_until("the free use is back", |s| s.contains("1/1 free"));
}

#[test]
fn a_favorite_is_listed_on_top() {
    let mut t = Tui::scenario("spells.favorite", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "ray of frost", "Ray of Frost");
    to_spell(&mut t, "Ray of Frost");
    t.send("f").wait("Ray of Frost is a favorite").wait("★ Favorites");
    assert!(t.line("Ray of Frost ★").contains("◦ Ray of Frost ★"), "{}", t.screen());
    t.send("f").wait_gone("★ Favorites");
    assert!(!t.shows("Ray of Frost ★"), "{}", t.screen());
}

#[test]
fn removing_a_spell() {
    let mut t = Tui::scenario("spells.remove", Home::party());
    spells_of(&mut t, "Tester");
    add(&mut t, "ray of frost", "Ray of Frost");
    add(&mut t, "burning", "Burning Hands");
    to_spell(&mut t, "Ray of Frost");
    t.send("x").wait("Removed Ray of Frost").wait("cantrips 0/3  prepared 0/4  book 1/6");
    assert!(!t.screen().contains("◦ Ray of Frost"), "{}", t.screen());
    assert!(t.shows("○ Burning Hands"), "{}", t.screen());
}

#[test]
fn rolling_a_spell_uses_its_class_numbers() {
    let mut t = Tui::scenario("spells.roll", Home::party());
    spells_of(&mut t, "Tester");
    // Intelligence 13: DC 8 + 2 + 1, attack 2 + 1.
    t.wait("DC 11 · +3");
    add(&mut t, "ray of frost", "Ray of Frost");
    add(&mut t, "acid splash", "Acid Splash");
    // An attack: the class's +3, and damage.
    to_spell(&mut t, "Ray of Frost");
    t.send("r").wait("Ray of Frost: ");
    let line = t.line("Ray of Frost: ");
    assert!(line.contains("to hit (") && line.contains(" + 3 = ") && line.contains("cold damage"), "{line}");
    // A save: the class's DC, and damage.
    to_spell(&mut t, "Acid Splash");
    t.send("r").wait("Acid Splash: ");
    let line = t.line("Acid Splash: ");
    assert!(line.contains("DC 11 (Dexterity save)") && line.contains("acid damage"), "{line}");
}
