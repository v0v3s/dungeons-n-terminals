//! Helpers for playing a character on its sheet: opening it, moving the
//! overview cursor, reading the status line and the numbers on screen.

use crate::harness::*;

/// The party in roster order.
const ROSTER: [&str; 4] = ["Brawler", "Hexa", "Ranger", "Tester"];

/// Open a character of the party on the Overview.
pub fn open(t: &mut Tui, name: &str) {
    let at = ROSTER.iter().position(|n| *n == name).expect("a member of the party");
    t.send("2").wait(name);
    t.send("g");
    t.send(&"j".repeat(at));
    t.send(ENTER).wait(&format!("◆ {name}")).wait("Armor Class");
}

/// Overview cursor positions (every character has six abilities and six
/// numbers before its attacks).
pub const STRENGTH: usize = 0;
pub const AC: usize = 6;
pub const INITIATIVE: usize = 8;
pub const FIRST_ATTACK: usize = 12;

/// Put the overview cursor on item `n` (in cursor order).
pub fn to(t: &mut Tui, n: usize) {
    t.send("g");
    if n > 0 {
        t.send(&"j".repeat(n));
    }
}

/// The status line (the screen's last row).
pub fn status(t: &Tui) -> String {
    t.screen().lines().last().unwrap_or_default().to_string()
}

/// Wait for the status line to show `text`.
#[track_caller]
pub fn wait_status(t: &mut Tui, text: &str) {
    t.wait_until(&format!("the status line shows {text:?}"), |s| s.lines().last().is_some_and(|l| l.contains(text)));
}

/// The total of a roll on the status line, `<title>: <total> (…)`.
#[track_caller]
pub fn rolled(t: &Tui, title: &str) -> i64 {
    let line = status(t);
    let at = line.find(&format!("{title}: ")).unwrap_or_else(|| panic!("no {title:?} roll on the status line: {line}"));
    let rest = &line[at + title.len() + 2..];
    let number: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    number.parse().unwrap_or_else(|_| panic!("no total after {title:?}: {line}"))
}

/// The number shown in a stat box (`Armor Class`, `Initiative`, …).
#[track_caller]
pub fn stat_box(t: &Tui, label: &str) -> String {
    let screen = t.screen();
    let lines: Vec<&str> = screen.lines().collect();
    let row = lines.iter().position(|l| l.contains(label)).unwrap_or_else(|| panic!("no {label:?} box:\n{screen}"));
    let col = lines[row].chars().collect::<Vec<_>>();
    let start = lines[row].split(label).next().unwrap().chars().count();
    // The box's value sits two rows above its label, within the box.
    let value_row: Vec<char> = lines[row - 2].chars().collect();
    let left = (0..start).rev().find(|&i| col.get(i) == Some(&'│')).unwrap_or(0);
    let right = (start..col.len()).find(|&i| col[i] == '│').unwrap_or(col.len());
    value_row[left + 1..right.min(value_row.len())].iter().collect::<String>().trim().to_string()
}

/// The Hit Points row of the Tracking pane, e.g. `5/7  +2 temp`.
pub fn hit_points(t: &Tui) -> String {
    let line = t.line("Hit Points");
    let after = line.split("Hit Points").nth(1).unwrap_or_default();
    after.trim_start_matches([' ', '█', '░']).split('│').next().unwrap_or_default().trim().to_string()
}
