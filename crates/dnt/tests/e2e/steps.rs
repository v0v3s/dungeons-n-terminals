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
