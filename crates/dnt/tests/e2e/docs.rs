//! The documentation's screenshots of the terminal app (docs/wiki/images),
//! made from the real program: `DNT_SHOTS=docs/wiki/images cargo test -p dnt
//! --test e2e docs`. Without DNT_SHOTS they are made and checked only.

use crate::harness::*;
use crate::seed::Home;

/// Keep the picture (when asked to) and check it shows `shows`.
fn keep(t: &Tui, name: &str, shows: &str) {
    let svg = t.svg();
    assert!(svg.starts_with("<svg") && svg.contains(shows), "{name}: {}", t.screen());
    if let Ok(dir) = std::env::var("DNT_SHOTS") {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(dir).join(format!("{name}.svg"));
        std::fs::write(&path, svg).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

#[test]
fn screenshots_of_the_terminal_app() {
    let mut t = Tui::for_docs(Home::empty());
    t.wait("Create a campaign");
    keep(&t, "tui-welcome", "Create a campaign");

    let mut t = Tui::for_docs(Home::party());
    t.send("2").send("G").wait("Human · Sage · Wizard 1");
    t.send(ENTER).wait("Overview");
    keep(&t, "tui-sheet", "Tester");

    // The help covers the sheet: its keys first.
    t.send("?").wait_gone("Hit points");
    keep(&t, "tui-help", "dnt");
    t.send(ESC);

    let mut t = Tui::for_docs(Home::party());
    t.send("2").send("n").typed("Nova").send(ENTER).wait("Choose a species");
    keep(&t, "tui-creation", "Choose a species");

    let mut t = Tui::for_docs(Home::campaign());
    t.send("1").send("/").typed("fireball").send(ENTER).wait("Using a Higher-Level Spell Slot");
    keep(&t, "tui-compendium", "Fireball");

    let mut t = Tui::for_docs(Home::campaign());
    t.send("3").wait("Sessions").send("c").wait("Campaign");
    keep(&t, "tui-notes", "Sessions");
}
