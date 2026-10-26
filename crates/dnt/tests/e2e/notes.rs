use crate::harness::*;
use crate::seed::Home;

const MIRA: &str = "---\ntype: npc\ntags: npc\nstatus: alive\n---\n# Mira Vell\n\n**Who:** the bell-ringer of Hollowmere.\n";
const GRIM: &str = "---\ntype: npc\nstatus: dead\n---\n# Grim Hollow\n\nA #villain who hid the bell.\n";
const LOST_BELL: &str = "---\ntype: quest\nstatus: active\n---\n# Lost Bell\n\nAsk [[Mira Vell]] about it.\nThe bell hums like [[spell:Frost Needle]].\nFind [[Old Tower]].\nIt rings at midnight.\n";
const DONE_DEAL: &str = "---\ntype: quest\nstatus: done\n---\n# Done Deal\n";
const SESSION: &str = "---\ntype: session\ndate: 2026-01-02\n---\n# 2026-01-02\n\nWe met [[Mira Vell]].\n";

/// The campaign with its notes as dnt makes them on first open, plus
/// `notes` (path in the notes folder, text).
fn with_notes(notes: &[(&str, &str)]) -> Home {
    let home = Home::campaign();
    let ws = home.workspace();
    home.store().notes(ws.active_campaign().expect("the campaign")).expect("the notes");
    for (path, text) in notes {
        let file = home.campaign_file(&format!("notes/{path}"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    home
}

fn note(t: &Tui, path: &str) -> Option<String> {
    std::fs::read_to_string(t.home.campaign_file(&format!("notes/{path}"))).ok()
}

/// The row of the screen holding `text`.
fn row(t: &Tui, text: &str) -> usize {
    t.screen().lines().position(|l| l.contains(text)).unwrap_or_else(|| panic!("no {text:?} on the screen:\n{}", t.screen()))
}

/// `YYYY-MM-DD` at the start of `s`.
fn starts_with_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10 && b[..10].iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
}

/// Open a note by searching for its title.
fn open(t: &mut Tui, title: &str) {
    t.send("/").wait("Search notes").typed(title).send(ENTER).wait_gone("Search notes");
}

/// Whether the folder tree (the left column) shows `text`.
fn in_tree(screen: &str, text: &str) -> bool {
    screen.lines().any(|l| l.chars().take(36).collect::<String>().contains(text))
}

#[test]
fn folders_open_and_close() {
    let mut t = Tui::scenario("notes.browse", with_notes(&[("NPCs/Mira Vell.md", MIRA)]));
    t.send("3").wait("Notes · E2E").wait("Campaign").wait("Inbox").wait("▾ Sessions").wait("▾ Lore");
    let shown = |s: &str| in_tree(s, "Mira Vell");
    t.wait_until("Mira Vell in the tree", shown);
    // Enter on a folder closes it, l opens it again.
    t.send("jjj").send(ENTER).wait("▸ NPCs").wait_until("Mira Vell hidden", |s| !shown(s));
    t.send("l").wait("▾ NPCs").wait_until("Mira Vell in the tree", shown);
    // h on a note goes to its folder, h on an open folder closes it.
    t.send("j").send("h").send("h").wait("▸ NPCs").wait_until("Mira Vell hidden", |s| !shown(s));
    t.send(RIGHT).wait_until("Mira Vell in the tree", shown);
    t.send("j").send(ENTER).wait("NPCs/Mira Vell").wait("the bell-ringer of Hollowmere").wait("● Mira Vell");
}

#[test]
fn a_new_note_comes_from_its_folders_template() {
    let mut t = Tui::scenario("notes.new", Home::campaign());
    t.send("3").wait("▾ NPCs");
    t.send("jjj").send("a").wait("New note in NPCs").typed("Mira Vell").send(ENTER);
    t.wait("Created NPCs/Mira Vell.md").wait("NPCs/Mira Vell").wait("type: npc").wait("**Wants:**").wait("NORMAL");
    let text = note(&t, "NPCs/Mira Vell.md").expect("the note is saved");
    assert!(text.contains("# Mira Vell") && text.contains("status: alive"), "{text}");
    t.send(ESC).wait("j k  move");
}

#[test]
fn a_new_folder() {
    let mut t = Tui::scenario("notes.new", Home::campaign());
    t.send("3").wait("▾ NPCs");
    t.send("A").wait("New folder").typed("Downtime").send(ENTER).wait("Created folder Downtime").wait("▾ Downtime");
    assert!(t.home.campaign_file("notes/Downtime").is_dir());
    // A note in a folder without a template of its own gets the plain one.
    t.send("G").send("a").wait("New note in Downtime").typed("Carousing").send(ENTER).wait("Downtime/Carousing");
    assert_eq!(note(&t, "Downtime/Carousing.md").unwrap().trim(), "# Carousing");
}

#[test]
fn renaming_a_note_and_a_folder() {
    let mut t = Tui::scenario("notes.rename", with_notes(&[("NPCs/Mira Vell.md", MIRA)]));
    t.send("3").wait("Mira Vell");
    t.send("jjjj").send("r").wait("Rename NPCs/Mira Vell.md").wait("❯ Mira Vell");
    t.send(&BACKSPACE.repeat(9)).typed("Mira the Bold").send(ENTER).wait("Renamed to NPCs/Mira the Bold.md").wait("Mira the Bold");
    assert!(note(&t, "NPCs/Mira the Bold.md").is_some_and(|n| n.contains("bell-ringer")));
    assert!(note(&t, "NPCs/Mira Vell.md").is_none());
    // The last folder.
    t.send("G").send("r").wait("Rename Lore").send(&BACKSPACE.repeat(4)).typed("Legends").send(ENTER);
    t.wait("Renamed to Legends").wait_until("Legends in the tree", |s| in_tree(s, "Legends  0"));
    assert!(t.home.campaign_file("notes/Legends").is_dir() && !t.home.campaign_file("notes/Lore").exists());
}

#[test]
fn deleting_asks_first() {
    let mut t = Tui::scenario("notes.delete", with_notes(&[("NPCs/Mira Vell.md", MIRA), ("Lore/Old Song.md", "# Old Song\n")]));
    t.send("3").wait("Mira Vell").wait("Old Song");
    // Esc and anything but y keep it.
    t.send("jjjj").send("d").wait("Delete NPCs/Mira Vell.md?").send(ESC).wait_gone("Delete NPCs");
    t.send("d").wait("Delete NPCs/Mira Vell.md?").typed("n").send(ENTER).wait_gone("Delete NPCs");
    assert!(t.shows("Mira Vell") && note(&t, "NPCs/Mira Vell.md").is_some());
    t.send("d").wait("Delete NPCs/Mira Vell.md?").send(ENTER).wait("Deleted NPCs/Mira Vell.md");
    assert!(note(&t, "NPCs/Mira Vell.md").is_none());
    // A folder, with what is in it.
    t.send("G").send("k").send("d").wait("Delete Lore?").typed("y").send(ENTER).wait("Deleted Lore").wait_gone("Old Song");
    assert!(!t.shows("▾ Lore") && !t.home.campaign_file("notes/Lore").exists());
}

#[test]
fn moving_a_top_level_folder() {
    let mut t = Tui::scenario("notes.folder-order", Home::campaign());
    t.send("3").wait("▾ NPCs");
    assert!(row(&t, "▾ Sessions") < row(&t, "▾ NPCs") && row(&t, "▾ NPCs") < row(&t, "▾ Quests"));
    // Down past Quests.
    t.send("jjj").send("J");
    t.wait_until("NPCs below Quests", |s| s.find("▾ Quests") < s.find("▾ NPCs"));
    // Up twice: to the top.
    t.send("K").send("K");
    t.wait_until("NPCs above Sessions", |s| s.find("▾ NPCs") < s.find("▾ Sessions"));
    // The order is kept with the campaign.
    let saved = std::fs::read_to_string(t.home.campaign_file("campaign.toml")).unwrap();
    assert!(saved.find("\"NPCs\"") < saved.find("\"Sessions\"") && saved.find("\"Sessions\"") < saved.find("\"Quests\""), "{saved}");
}

#[test]
fn todays_session_note() {
    let mut t = Tui::scenario("notes.session", Home::campaign());
    t.send("3").wait("▾ Sessions  0");
    t.send("s").wait("Sessions/").wait("type: session").wait("## Recap").wait("## Next time").wait("▾ Sessions  1");
    let title = t.line(" Sessions/");
    let name = &title[title.find("Sessions/").unwrap() + "Sessions/".len()..];
    assert!(starts_with_date(name), "named by the date: {title}");
    let date = &name[..10];
    // The template's date and title are today's.
    t.wait(&format!("date: {date}")).wait(&format!("# {date}"));
    assert!(note(&t, &format!("Sessions/{date}.md")).is_some_and(|n| n.contains("## Recap")));
}

#[test]
fn quick_capture_lands_in_the_inbox() {
    let mut t = Tui::scenario("notes.capture", Home::campaign());
    // From anywhere: here, the compendium.
    t.send("1").send(&alt('c')).wait("Quick capture → Inbox").typed("The bridge is out").send(ENTER);
    t.wait_gone("Quick capture").wait("Captured to Inbox");
    t.send("3").send("i").wait("The bridge is out");
    let line = t.line("The bridge is out");
    let at = line.find("- ").expect("a list item") + 2;
    // `- YYYY-MM-DD HH:MM text`.
    let stamp = &line[at..];
    assert!(starts_with_date(stamp) && stamp.as_bytes()[13] == b':' && stamp[16..].starts_with(" The bridge is out"), "{line}");
    let inbox = note(&t, "Inbox.md").unwrap();
    assert!(inbox.lines().any(|l| l.starts_with("- ") && l.ends_with(" The bridge is out")), "{inbox}");
}

#[test]
fn editing_and_saving_a_note() {
    let mut t = Tui::scenario("notes.edit", Home::campaign());
    t.send("3").send("i").wait("Quick notes land here").wait("NORMAL");
    t.send("G").send("o").wait("INSERT").typed("Bring rope");
    t.send(ESC).wait("NORMAL").wait("Inbox ●");
    // Esc in normal mode saves and goes back to the tree.
    t.send(ESC).wait("Saved Inbox.md").wait_gone("Inbox ●");
    assert!(note(&t, "Inbox.md").unwrap().contains("Bring rope\n"));
    // Ctrl+S saves and stays.
    t.send(TAB).send("o").typed("Buy lamp oil").send(ESC).send(&ctrl('s')).wait("Saved Inbox.md");
    assert!(note(&t, "Inbox.md").unwrap().contains("Buy lamp oil\n"));
    assert!(t.shows("NORMAL"));
}

#[test]
fn following_links_and_going_back() {
    let home = with_notes(&[("NPCs/Mira Vell.md", MIRA), ("Quests/Lost Bell.md", LOST_BELL)]);
    let mut t = Tui::scenario("notes.links", home);
    t.send("3");
    open(&mut t, "lost bell");
    t.wait("Quests/Lost Bell").wait("→ Old Tower  (new)");
    // A note.
    t.send("jjjjjj").send("0").send("f[").send("gd").wait("NPCs/Mira Vell").wait("the bell-ringer of Hollowmere");
    t.send(&ctrl('o')).wait("Quests/Lost Bell");
    // A compendium entry.
    t.send("j").send("0").send("f[").send("gd").wait("Frost Needle").wait("Make a ranged spell attack");
    t.send("3").wait("Quests/Lost Bell");
    // A missing note: made in the chosen folder, from its template.
    t.send("j").send("0").send("f[").send("gd").wait("Create “Old Tower” in which folder?");
    t.typed("Places").send(ENTER).wait("Places/Old Tower").wait("**What it is:**");
    assert!(note(&t, "Places/Old Tower.md").is_some_and(|n| n.contains("# Old Tower")));
    t.send(&ctrl('o')).wait("Quests/Lost Bell").wait("→ Old Tower");
    assert!(!t.shows("(new)"));
}

#[test]
fn previewing_a_link() {
    let home = with_notes(&[("NPCs/Mira Vell.md", MIRA), ("Quests/Lost Bell.md", LOST_BELL)]);
    let mut t = Tui::scenario("notes.link-preview", home);
    t.send("3");
    open(&mut t, "lost bell");
    t.wait("Quests/Lost Bell");
    t.send("jjjjjj").send("0").send("f[").send("K").wait("any key closes").wait("the bell-ringer of Hollowmere");
    t.send("x").wait_gone("any key closes");
    // Still here, unchanged.
    t.wait("Quests/Lost Bell").wait("Ask [[Mira Vell]] about it.");
    t.send("j").send("0").send("f[").send("K").wait("any key closes").wait("Make a ranged spell attack");
    t.send(ESC).wait_gone("any key closes").wait("Quests/Lost Bell");
    // A missing note.
    t.send("j").send("0").send("f[").send("K").wait("No such note yet");
}

#[test]
fn notes_that_link_here() {
    let home = with_notes(&[
        ("NPCs/Mira Vell.md", MIRA),
        ("NPCs/Grim Hollow.md", GRIM),
        ("Quests/Lost Bell.md", LOST_BELL),
        ("Sessions/2026-01-02.md", SESSION),
    ]);
    let mut t = Tui::scenario("notes.backlinks", home);
    t.send("3");
    open(&mut t, "mira");
    t.wait("NPCs/Mira Vell").wait("Mentioned in · 2").wait("• Lost Bell").wait("• 2026-01-02");
    t.send(ESC);
    open(&mut t, "grim");
    t.wait("NPCs/Grim Hollow").wait("Mentioned in · 0").wait("Nothing links here yet.");
}

#[test]
fn query_blocks_are_answered() {
    let home = with_notes(&[
        ("NPCs/Mira Vell.md", MIRA),
        ("NPCs/Grim Hollow.md", GRIM),
        ("Quests/Lost Bell.md", LOST_BELL),
        ("Quests/Done Deal.md", DONE_DEAL),
        ("Sessions/2026-01-02.md", SESSION),
        ("Lore/Old Song.md", "# Old Song\n\n#unread\n"),
        ("Lore/Known Tale.md", "# Known Tale\n"),
        ("Lore/Reading List.md", "# Reading List\n\n```dnt Lore to read\n@Lore #unread\n```\n"),
    ]);
    let mut t = Tui::scenario("notes.queries", home);
    // The campaign hub's lists.
    t.send("3").send("c").wait("Campaign");
    t.wait("Open quests · 1").wait("• Lost Bell").wait("Recent sessions · 1").wait("• 2026-01-02").wait("People · 1").wait("• Mira Vell");
    assert!(!t.shows("• Done Deal"), "a done quest is not open");
    assert!(!t.shows("• Grim Hollow"), "the dead are not listed");
    // A note's own block.
    t.send(ESC);
    open(&mut t, "reading list");
    t.wait("Lore/Reading List").wait("Lore to read · 1").wait("• Old Song");
    assert!(!t.shows("• Known Tale"));
}

#[test]
fn searching_notes() {
    let home = with_notes(&[
        ("NPCs/Mira Vell.md", MIRA),
        ("NPCs/Grim Hollow.md", GRIM),
        ("Quests/Lost Bell.md", LOST_BELL),
        ("Quests/Done Deal.md", DONE_DEAL),
    ]);
    let mut t = Tui::scenario("notes.search", home);
    t.send("3").send("/").wait("Search notes");
    // Words: the title.
    t.typed("mira").wait(" 1 found ").send(ENTER).wait("NPCs/Mira Vell");
    // A #tag.
    t.send(ESC).send("/").wait("Search notes").typed("#villain").wait(" 1 found ").send(ENTER).wait("NPCs/Grim Hollow");
    // key:value in the front matter.
    t.send(ESC).send(&ctrl('p')).wait("Search notes").typed("status:done").wait(" 1 found ").send(ENTER).wait("Quests/Done Deal");
    // @Folder.
    t.send(ESC).send("/").wait("Search notes").typed("@Quests").wait(" 2 found ");
    t.typed(" !status:done").wait(" 1 found ").send(ENTER).wait("Quests/Lost Bell");
    // Nothing.
    t.send(ESC).send("/").typed("zzz").wait(" 0 found ").send(ESC).wait_gone("Search notes");
}

#[test]
fn editor_commands() {
    let mut t = Tui::scenario("notes.commands", with_notes(&[("NPCs/Mira Vell.md", MIRA)]));
    t.send("3").send("i").wait("Quick notes land here");
    // :w
    t.send(":").typed("w").send(ENTER).wait("No changes");
    t.send("G").send("o").typed("Ask about the bell").send(ESC).wait("Inbox ●");
    t.send(":").typed("w").send(ENTER).wait("Saved Inbox.md").wait_gone("Inbox ●");
    assert!(note(&t, "Inbox.md").unwrap().contains("Ask about the bell"));
    // :e opens a note by its title.
    t.send(":").typed("e Mira Vell").send(ENTER).wait("NPCs/Mira Vell").wait("bell-ringer");
    t.send(":").typed("e Nobody").send(ENTER).wait("No note called Nobody");
    // :new makes a note next to the open one.
    t.send(":").typed("new Bram Oakes").send(ENTER).wait("NPCs/Bram Oakes").wait("**Wants:**");
    assert!(note(&t, "NPCs/Bram Oakes.md").is_some());
    // :session, :inbox, :hub.
    t.send(":").typed("session").send(ENTER).wait("## Recap");
    assert!(starts_with_date(&t.line(" Sessions/")[t.line(" Sessions/").find("Sessions/").unwrap() + 9..]));
    t.send(":").typed("inbox").send(ENTER).wait("Ask about the bell");
    t.send(":").typed("hub").send(ENTER).wait("type: campaign").wait("## House rules");
    // :find opens the search with the words.
    t.send(":").typed("find bram").send(ENTER).wait("Search notes").wait(" 1 found ").send(ENTER).wait("NPCs/Bram Oakes");
    // :q closes the note.
    t.send(":").typed("q").send(ENTER).wait("No note open");
}
