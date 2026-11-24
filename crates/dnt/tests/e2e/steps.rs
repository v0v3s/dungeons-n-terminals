//! Helpers of the character, creation and level-up scenarios: the screen
//! read with its runs of spaces as one, and characters written as files.

use crate::harness::Tui;
use crate::seed::Home;

/// Text with each run of spaces as one space.
pub fn squash(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        out.push(c);
    }
    out
}

/// Wait for `text` on screen, spaces squashed.
#[track_caller]
pub fn see(t: &mut Tui, text: &str) {
    let want = squash(text);
    t.wait_until(&format!("the screen shows {text:?}"), |s| squash(s).contains(&want));
}

/// Whether the screen shows `text`, spaces squashed.
pub fn shows(t: &Tui, text: &str) -> bool {
    squash(&t.screen()).contains(&squash(text))
}

/// The screen's row holding `text`, spaces squashed.
#[track_caller]
pub fn row(t: &Tui, text: &str) -> String {
    let want = squash(text);
    let s = t.screen();
    s.lines().map(squash).find(|l| l.contains(&want)).unwrap_or_else(|| panic!("[{}] no row with {text:?}; the screen:\n{s}", t.scenario))
}

/// A character file in the campaign, as a player writes one by hand.
pub fn character_file(home: &Home, slug: &str, toml: &str) {
    std::fs::write(home.campaign_file("characters").join(format!("{slug}.toml")), toml).expect("the character file");
}

/// A level 1 Runebinder (the invented class made from the Hexer), last on
/// the roster: always prepared Shield with a free cast, Jump for a Binding.
pub const WREN: &str = r#"name = "Wren"
species = "race:Halfling|SRD52"
background = "background:Sage|SRD52"

[abilities]
str = 8
dex = 14
con = 12
int = 10
wis = 13
cha = 15

[[levels]]
class = "class:Runebinder|TST"

[subclasses]
"class:Runebinder|TST" = "subclass:Grove|Runebinder|TST|TST"
"#;

/// The Runebinder's own rules for Hexer Patron: Bindings to spend.
pub const BINDINGS: &str = "[[feature]]\nid = \"classFeature:Hexer Patron|Hexer|TST|1\"\nclass = \"Runebinder|TST\"\n[[feature.resource]]\nkey = \"bindings\"\nname = \"Bindings\"\nmax = \"level('Runebinder') + 1\"\nrecharge = \"long_rest\"\n";

/// A rule that grants Wren spells: a cantrip, and a level 1 Wizard spell
/// to choose, cast free once a day.
pub const GIFTS: &str = "[[feature]]\nid = \"classFeature:Hexer Patron|Hexer|TST|1\"\nclass = \"Runebinder|TST\"\nname = \"Patron's Gifts\"\n[[feature.spell]]\nspell = \"spell:Light|SRD52\"\ngrant = \"known\"\n[[feature.spell]]\nchoose = \"level=1|class=Wizard\"\ngrant = \"free\"\nuses = 1\n";

/// Add Wren's rule that grants spells.
pub fn with_gifts(home: Home) -> Home {
    let dir = home.store().overlays_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("gifts.toml"), GIFTS).unwrap();
    home
}

/// The party with Wren, and Wren's rules when `rules`.
pub fn with_wren(rules: bool) -> Home {
    let home = Home::party();
    character_file(&home, "wren", WREN);
    if rules {
        let dir = home.store().overlays_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bindings.toml"), BINDINGS).unwrap();
    }
    home
}

/// Open Wren (last on the roster) on the Overview.
pub fn open_wren(t: &mut Tui) {
    t.send("2").wait("Wren").send("G").send(crate::harness::ENTER).wait("◆ Wren").wait("Armor Class");
}
