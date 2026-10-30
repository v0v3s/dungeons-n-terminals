use unicode_width::UnicodeWidthChar;

use crate::harness::*;
use crate::seed::Home;

#[test]
fn the_number_keys_move_between_sections() {
    let mut t = Tui::scenario("nav.sections", Home::party());
    t.wait("Browse");
    t.send("2").wait("Characters").wait("Preview").wait("Tester");
    t.send("3").wait("Notes · E2E");
    t.send("4").wait("Books · E2E");
    t.send("1").wait("Browse").wait_gone("Books · E2E");
    // While typing, alt and a number.
    t.send("/").typed("fireball").wait("Fireball");
    t.send(&alt('2')).wait("Preview").wait("Tester");
}

#[test]
fn quitting_from_a_list() {
    let mut t = Tui::scenario("nav.sections", Home::party());
    t.send("2").wait("Tester");
    t.send("q");
    t.wait_until("dnt quits", |s| !s.contains("Characters"));
}

/// The first group of the help: the keys of the screen it was opened on.
fn help_for(t: &mut Tui, group: &str) {
    t.send("?").wait("This screen");
    let screen = t.screen();
    let lines: Vec<&str> = screen.lines().collect();
    let at = lines.iter().position(|l| l.contains("This screen")).unwrap();
    let first = lines[at + 1..].iter().find(|l| !l.trim_matches([' ', '│']).is_empty()).unwrap();
    assert!(first.contains(group), "the help starts with {group:?}:\n{screen}");
    t.send(ESC).wait_gone("This screen");
}

#[test]
fn the_help_shows_the_keys_of_every_screen() {
    let mut t = Tui::scenario("nav.help", Home::party());
    help_for(&mut t, "Compendium: list");
    t.send("2").wait("Tester");
    help_for(&mut t, "Characters: roster");
    t.send("3").wait("Notes · E2E");
    help_for(&mut t, "Notes: tree");
    t.send("4").wait("Books · E2E");
    help_for(&mut t, "Campaigns");
    t.send("2").send("G").send(ENTER).wait("Overview");
    help_for(&mut t, "Sheet: Overview");
    t.send(TAB).send(TAB).wait("Spells");
    help_for(&mut t, "Sheet: Spells");
    // F1 too, and the other screens follow.
    t.send("\x1bOP").wait("This screen").wait("Everywhere");
    t.send("G").wait("Notes: commands (:)");
    t.send("q").wait_gone("This screen");
}

/// The screen as cells, wide characters taking two.
fn grid(screen: &str) -> Vec<Vec<char>> {
    screen
        .lines()
        .map(|l| {
            let mut row = Vec::new();
            for c in l.chars() {
                row.push(c);
                row.extend(std::iter::repeat_n('\0', c.width().unwrap_or(0).saturating_sub(1)));
            }
            row
        })
        .collect()
}

/// Every row fits the width, and every box's sides run straight down
/// from its top corners to its bottom ones.
#[track_caller]
fn fits(t: &Tui, what: &str) {
    // A whole frame, not one half drawn.
    t.settle();
    let screen = t.screen();
    let g = grid(&screen);
    let fail = |why: String| panic!("{what}: {why}\n{screen}");
    let at = |r: usize, c: usize| g.get(r).and_then(|row| row.get(c)).copied().unwrap_or(' ');
    for (r, row) in g.iter().enumerate() {
        if row.len() > COLS as usize {
            fail(format!("row {r} is {} wide", row.len()));
        }
        for (c, &ch) in row.iter().enumerate() {
            let (side, bottom): (&[char], char) = match ch {
                '╭' => (&['│', '├'], '╰'),
                '╮' => (&['│', '┤', '█', '║'], '╯'),
                _ => continue,
            };
            let mut down = r + 1;
            loop {
                let below = at(down, c);
                if below == bottom {
                    break;
                }
                if !side.contains(&below) {
                    fail(format!("the box at row {r}, column {c} breaks at row {down} ({below:?})"));
                }
                down += 1;
            }
        }
    }
}

#[test]
fn every_section_and_sheet_tab_fits_the_window() {
    let mut t = Tui::scenario("layout.fits", Home::party());
    t.wait("Browse");
    fits(&t, "compendium");
    t.send(ENTER);
    fits(&t, "an entry");
    t.send("3").wait("Notes · E2E");
    fits(&t, "notes");
    t.send("4").wait("Books · E2E");
    fits(&t, "campaigns");
    t.send(TAB).send(TAB).wait("Homebrew · E2E");
    fits(&t, "campaign homebrew");
    t.send("?").wait("This screen");
    fits(&t, "help");
    t.send(ESC);
    t.send("2").wait("Tester");
    fits(&t, "roster");
    for (i, name) in ["Brawler", "Hexa", "Ranger", "Tester"].into_iter().enumerate() {
        t.send("2").send("g");
        for _ in 0..i {
            t.send("j");
        }
        t.send(ENTER).wait("Overview").wait(name);
        for tab in ["Overview", "Features", "Spells", "Inventory", "Notes"] {
            fits(&t, &format!("{name}'s {tab}"));
            t.send(TAB);
        }
        t.send(ESC).wait("Preview");
    }
    t.send("n").typed("Zed").send(ENTER).wait("Choose a species");
    fits(&t, "new character");
}
