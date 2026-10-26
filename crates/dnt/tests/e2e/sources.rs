use crate::harness::*;
use crate::seed::Home;

/// The Campaigns screen with the books pane focused.
fn books(t: &mut Tui) {
    t.send("4").send("h").send(TAB).wait("Books · E2E");
}

/// The compendium searched for `query`.
fn search(t: &mut Tui, query: &str) {
    t.send("1").send("/").send(&ctrl('u')).typed(query);
}

/// The row of a book in the books pane.
fn book(t: &Tui, id: &str) -> String {
    t.screen()
        .lines()
        .find(|l| l.starts_with(" │") && l.contains(&format!(" {id} ")))
        .unwrap_or_else(|| panic!("no book {id}:\n{}", t.screen()))
        .to_string()
}

#[test]
fn a_book_turned_off_leaves_the_compendium_and_the_pickers() {
    let mut t = Tui::scenario("sources.toggle-book", Home::party());
    search(&mut t, "mirefolk");
    t.wait("1 results");
    t.send(ESC).send(ESC);
    books(&mut t);
    assert!(book(&t, "TSX").contains("●"), "{}", t.screen());
    t.send("j").send(" ");
    t.wait_until("TSX is off", |s| s.lines().any(|l| l.contains(" TSX ") && l.contains("○")));
    assert!(book(&t, "TST").contains("●"), "{}", t.screen());
    search(&mut t, "mirefolk");
    t.wait("0 results");
    t.send(ESC).send(ESC);
    // Nor can a new character pick it.
    t.send("2").send("n").typed("Zed").send(ENTER).wait("Choose a species").wait("Stoneling");
    assert!(!t.shows("Mirefolk"), "{}", t.screen());
    t.send(ESC).send(ESC);
    // On again, it is back.
    books(&mut t);
    t.send("j").send(" ");
    t.wait_until("TSX is on", |s| s.lines().any(|l| l.contains(" TSX ") && l.contains("●")));
    search(&mut t, "mirefolk");
    t.wait("1 results");
}

#[test]
fn a_preset_chooses_its_books() {
    let mut t = Tui::scenario("sources.preset", Home::party());
    books(&mut t);
    t.wait("preset: 2024 Core");
    t.send("p").wait("preset: 2014 Core");
    // The preset's books are not the test books: they are all off.
    assert!(book(&t, "TST").contains("○") && book(&t, "TSX").contains("○"), "{}", t.screen());
    assert!(t.line(" dnt ").contains("XPHB"), "{}", t.screen());
    t.send("1").wait("All 0");
}

#[test]
fn all_turns_every_book_on() {
    let mut t = Tui::scenario("sources.all", Home::party());
    books(&mut t);
    t.send("p").wait("preset: 2014 Core");
    t.send("a");
    t.wait_until("every book is on", |s| s.lines().filter(|l| l.contains("●")).count() == 2);
    assert!(t.line(" dnt ").contains("all sources"), "{}", t.screen());
    t.send("1").wait("All 203");
}

#[test]
fn reprinted_entries_show_their_older_version() {
    let mut t = Tui::scenario("sources.reprinted", Home::party());
    search(&mut t, "spark");
    t.wait("Spark Affinity");
    assert!(!t.screen().lines().any(|l| l.contains("Spark ") && l.contains("TSX")), "{}", t.screen());
    t.send(ESC).send(ESC);
    books(&mut t);
    t.wait("reprinted originals: hidden");
    t.send("r").wait("reprinted originals: shown");
    search(&mut t, "spark");
    t.wait_until("the TSX Spark is listed", |s| s.lines().any(|l| l.contains("✦ Spark ") && l.contains("TSX")));
}

#[test]
fn the_books_filter_by_name() {
    let mut t = Tui::scenario("sources.filter", Home::party());
    books(&mut t);
    t.send("/").typed("outsk").wait("/outsk");
    t.wait("Proving Grounds: Outskirts");
    assert!(!t.shows("The Proving Grounds"), "{}", t.screen());
    t.send(ENTER).wait("Proving Grounds: Outskirts");
    // Esc while filtering clears it.
    t.send("/").typed("proving").wait("The Proving Grounds");
    t.send(ESC).wait("The Proving Grounds").wait("Outskirts");
    assert!(!t.shows("/proving"), "{}", t.screen());
}

#[test]
fn homebrew_turned_on_shows_and_all_off_removes_it() {
    let mut t = Tui::scenario("sources.homebrew", Home::party());
    search(&mut t, "sundial");
    t.wait("0 results");
    t.send(ESC).send(ESC);
    books(&mut t);
    t.send(TAB).wait("Homebrew · E2E").wait("0 on · 1 imported");
    t.send(" ").wait("1 on · 1 imported");
    assert!(t.line("Proving Grounds Brew").contains("●"), "{}", t.screen());
    search(&mut t, "sundial");
    t.wait("Pocket Sundial");
    t.send(ESC).send(ESC);
    t.send("4").wait("Homebrew · E2E");
    t.send("x").wait("0 on · 1 imported");
    search(&mut t, "sundial");
    t.wait("0 results");
}

#[test]
fn the_homebrew_filters_by_name_or_author() {
    let mut t = Tui::scenario("sources.homebrew-filter", Home::party());
    books(&mut t);
    t.send(TAB).wait("Homebrew · E2E").wait("Proving Grounds Brew");
    // By author.
    t.send("/").typed("test guild").wait("/test guild").wait("Proving Grounds Brew");
    t.send(ENTER);
    // By name, and no match.
    t.send("/").typed("brew").wait("Proving Grounds Brew");
    t.send(ENTER).send("/").typed("nothing").wait("/nothing");
    t.wait_gone("Proving Grounds Brew");
    t.send(ESC).wait("Proving Grounds Brew");
}
