use crate::harness::*;
use crate::seed::Home;
use crate::steps::{row, see, shows, squash};

/// On the roster, press `n` and name the character.
fn start(mut t: Tui, name: &str) -> Tui {
    t.send("2").wait("Tester");
    t.send("n").wait("New character");
    t.typed(name);
    t.send(ENTER).wait("Choose a species");
    t
}

/// Wait for a question, type a filter and choose the first match.
#[track_caller]
fn answer(t: &mut Tui, question: &str, filter: &str) {
    see(t, question);
    t.typed(filter);
    t.send(ENTER);
}

/// Tick each of `labels` (typed as filters) in a multi-choice question.
#[track_caller]
fn tick(t: &mut Tui, question: &str, labels: &[&str]) {
    see(t, question);
    for label in labels {
        t.typed(label);
        t.send(" ").send(&ctrl('u'));
    }
    see(t, &format!("{}/{} chosen", labels.len(), labels.len()));
    t.send(ENTER);
}

/// "2/3 chosen" on a multi-choice question.
fn chosen(screen: &str) -> Option<(usize, usize)> {
    let at = screen.find(" chosen")?;
    let (have, need) = screen[..at].rsplit(' ').next()?.split_once('/')?;
    Some((have.parse().ok()?, need.parse().ok()?))
}

/// Answer the questions as offered (the first choices) until `question`.
#[track_caller]
fn until(t: &mut Tui, question: &str) {
    for _ in 0..40 {
        let before = t.screen();
        let s = squash(&before);
        if s.contains(&squash(question)) {
            return;
        }
        if let Some((have, need)) = chosen(&s) {
            for n in have..need {
                t.send(" ");
                see(t, &format!("{}/{need} chosen", n + 1));
                t.send(DOWN);
            }
        }
        let before = if chosen(&s).is_some() { t.screen() } else { before };
        t.send(ENTER);
        // Answered: the next question is drawn.
        t.wait_until("the next question", |now| now != before);
    }
    see(t, question);
}

/// Answer the rest as offered, up to the review.
#[track_caller]
fn to_review(t: &mut Tui) {
    until(t, "╭ Review ─");
}

#[test]
fn a_character_is_made_step_by_step() {
    let mut t = start(Tui::scenario("creation.full", Home::party()), "Newbie");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Drifter");
    // Swap Strength's 15 with Dexterity's 14.
    see(&mut t, "Ability scores · Standard array");
    t.send("h");
    see(&mut t, "Strength ‹ 14 ›");
    t.send(ENTER);
    answer(&mut t, "Background (Drifter): ability increases", "+2 / +1");
    answer(&mut t, "Background (Drifter): +2 to which ability?", "Dexterity");
    answer(&mut t, "Background (Drifter): +1 to which ability?", "Constitution");
    answer(&mut t, "Choose a feat", "Quick Wits");
    answer(&mut t, "Choose your starting class", "Warden");
    tick(&mut t, "Warden: choose 3 skill(s)", &["Athletics", "Nature", "Perception"]);
    answer(&mut t, "Kindred: choose 1 skill(s)", "Medicine");
    tick(&mut t, "Drifter: choose 2 skill(s)", &["Stealth", "Survival"]);
    answer(&mut t, "Favored Ground: choose one", "Forest");
    tick(&mut t, "Weapon Mastery: choose 2", &["Longfang", "Thunderbow"]);
    answer(&mut t, "Class (Warden): starting equipment", "A:");
    answer(&mut t, "Background (Drifter): starting equipment", "A:");
    answer(&mut t, "Choose a musical instrument", "Reed Pipe");

    see(&mut t, "╭ Review ─");
    see(&mut t, "Species Kindred");
    see(&mut t, "Background Drifter");
    see(&mut t, "Abilities STR 14 DEX 15 CON 13 INT 12 WIS 10");
    see(&mut t, "Background (Drifter) Dexterity +2, Constitution +1");
    see(&mut t, "Feat Lucky Charm");
    see(&mut t, "Feat Quick Wits");
    see(&mut t, "Class Warden 1");
    see(&mut t, "Skills (Class) Athletics, Nature,");
    see(&mut t, "Skills (Kindred) Medicine");
    see(&mut t, "Skills (Drifter) Stealth, Survival");
    see(&mut t, "Favored Ground Forest Ground");
    see(&mut t, "Weapon Mastery Longfang, Thunderbow");
    see(&mut t, "Equipment Reed Pipe");

    t.send(ENTER);
    see(&mut t, "Kindred · Drifter · Warden 1 · Level 1");
    t.wait("Longfang").wait("Thunderbow");
    // Dexterity 15 + 2 from the background.
    assert!(row(&t, "│ Dexterity").contains("Dexterity 17"), "{}", t.screen());
    t.send("q").wait("Newbie");
    assert!(row(&t, "Newbie").contains("Lv 1"), "{}", t.screen());
}

#[test]
fn a_lineage_of_the_species_is_chosen() {
    let mut t = start(Tui::scenario("creation.lineage", Home::party()), "Cinder");
    answer(&mut t, "Choose a species", "Emberkin");
    see(&mut t, "Emberkin: choose a lineage");
    t.wait("Emberkin; Ash Lineage");
    answer(&mut t, "Emberkin: choose a lineage", "Cinder");
    see(&mut t, "Choose a background");
    to_review(&mut t);
    t.send(ENTER);
    see(&mut t, "Emberkin; Cinder Lineage · ");
}

#[test]
fn a_subspecies_is_chosen() {
    let mut t = start(Tui::scenario("creation.lineage", Home::party()), "Hillfolk");
    answer(&mut t, "Choose a species", "Stoneling");
    see(&mut t, "Stoneling: choose a subrace");
    t.wait("Stoneling (Deep)");
    answer(&mut t, "Stoneling: choose a subrace", "Hill");
    see(&mut t, "Choose a background");
    // The subrace's own increase is asked for.
    until(&mut t, "Species (Stoneling (Hill)): ability increases");
    to_review(&mut t);
    t.wait("Stoneling (Hill)");
    t.send(ENTER);
    see(&mut t, "Level 1");
    t.send(TAB).wait("Stoneling (Hill)");
}

#[test]
fn scores_are_bought_with_27_points() {
    let mut t = start(Tui::scenario("creation.point-buy", Home::party()), "Buyer");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Archivist");
    // The scores start as the standard array; `m` changes the method.
    see(&mut t, "Ability scores · Standard array");
    t.send("m");
    see(&mut t, "Ability scores · Point buy");
    see(&mut t, "▸Point buy");
    see(&mut t, "Points left 27/27");
    t.send("l");
    see(&mut t, "Points left 26/27");
    t.send("h");
    see(&mut t, "Points left 27/27");
    // Strength, Dexterity and Constitution to 15: 9 points each.
    for (i, ability) in ["Strength", "Dexterity", "Constitution"].into_iter().enumerate() {
        if i > 0 {
            t.send("j");
        }
        for _ in 0..7 {
            t.send("l");
        }
        see(&mut t, &format!("{ability} ‹ 15 ›"));
    }
    see(&mut t, "Points left 0/27");
    // Nothing more to spend, and 15 is the most.
    t.send("l");
    see(&mut t, "Constitution ‹ 15 ›");
    t.send("j").send("l");
    see(&mut t, "Intelligence ‹ 8 ›");
    see(&mut t, "Points left 0/27");
    t.send(ENTER);
    see(&mut t, "Background (Archivist): ability increases");
    to_review(&mut t);
    see(&mut t, "Abilities STR 15 DEX 15 CON 15 INT 8 WIS 8");
    see(&mut t, "(Point buy)");
    t.send(ENTER);
    see(&mut t, "Kindred · Archivist");
    assert!(row(&t, "│ Strength").contains("Strength 15"), "{}", t.screen());
    assert!(row(&t, "│ Constitution").contains("Constitution 15"), "{}", t.screen());
}

#[test]
fn a_subclass_is_chosen_at_level_1() {
    let mut t = start(Tui::scenario("creation.subclass", Home::party()), "Tidecaller");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Archivist");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Hexer");
    see(&mut t, "Choose a subclass");
    t.wait("The Lantern").wait("The Tide");
    answer(&mut t, "Choose a subclass", "Tide");
    see(&mut t, "Hexer: choose 2 skill(s)");
    to_review(&mut t);
    see(&mut t, "Subclass The Tide");
    t.send(ENTER);
    see(&mut t, "Kindred · Archivist · Hexer 1");
    t.send(TAB).wait("The Tide");
}

#[test]
fn a_class_features_choices_are_made_at_level_1() {
    let mut t = start(Tui::scenario("creation.feature-choice", Home::party()), "Warder");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Archivist");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Warden");
    until(&mut t, "Favored Ground: choose one");
    t.wait("Forest Ground").wait("Mountain Ground");
    answer(&mut t, "Favored Ground: choose one", "Mountain");
    see(&mut t, "Weapon Mastery: choose 2");
    t.wait("Twin Sickle");
    // Two at most.
    for weapon in ["Longfang", "Thunderbow", "Twin Sickle"] {
        t.typed(weapon);
        t.send(" ").send(&ctrl('u'));
    }
    see(&mut t, "2/2 chosen");
    assert!(row(&t, "Twin Sickle").contains("○ Twin Sickle"), "{}", t.screen());
    t.send(ENTER);
    to_review(&mut t);
    see(&mut t, "Favored Ground Mountain Ground");
    see(&mut t, "Weapon Mastery Longfang, Thunderbow");
    t.send(ENTER);
    see(&mut t, "Warden 1 · Level 1");
    t.send(TAB).wait("Mountain Ground");
    let s = t.screen();
    let group: String = s.lines().skip_while(|l| !squash(l).contains("│ Weapon Mastery │")).collect::<Vec<_>>().join("\n");
    assert!(group.contains("Longfang") && group.contains("Thunderbow"), "{s}");
}

#[test]
fn an_item_is_picked_for_an_equipment_choice() {
    let mut t = start(Tui::scenario("creation.equipment-pick", Home::party()), "Picker");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Drifter");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Hexer");
    until(&mut t, "Class (Hexer): starting equipment");
    answer(&mut t, "Class (Hexer): starting equipment", "A:");
    t.wait("Crystal Orb").wait("Hex Pouch");
    t.typed("Pouch");
    t.send(ENTER);
    answer(&mut t, "Background (Drifter): starting equipment", "A:");
    see(&mut t, "Choose a musical instrument");
    t.wait("Reed Pipe");
    answer(&mut t, "Choose a musical instrument", "Hand Drum");
    see(&mut t, "╭ Review ─");
    see(&mut t, "Hex Pouch (an arcane focus)");
    see(&mut t, "Drum (a musical instrument)");
    t.send(ENTER);
    see(&mut t, "Hexer 1 · Level 1");
    t.send(TAB).send(TAB).send(TAB);
    see(&mut t, "╭ Inventory");
    t.wait("Hex Pouch").wait("Hand Drum");
    assert!(!t.shows("Crystal Orb"), "{}", t.screen());
    assert!(!t.shows("Reed Pipe"), "{}", t.screen());
}

#[test]
fn going_back_changes_an_answer_and_esc_at_the_start_cancels() {
    let mut t = start(Tui::scenario("creation.back", Home::party()), "Undecided");
    answer(&mut t, "Choose a species", "Kindred");
    see(&mut t, "Choose a background");
    t.send(ESC);
    see(&mut t, "Choose a species");
    answer(&mut t, "Choose a species", "Emberkin");
    // The new species asks its own question.
    answer(&mut t, "Emberkin: choose a lineage", "Ash");
    see(&mut t, "Choose a background");
    assert!(!t.shows("Kindred"), "{}", t.screen());
    to_review(&mut t);
    see(&mut t, "Species Emberkin");
    // Back from the review, step by step, then out.
    for _ in 0..40 {
        if !shows(&t, "New character · Undecided") {
            break;
        }
        t.send(ESC);
    }
    t.wait("Tester");
    assert!(!t.shows("Undecided"), "{}", t.screen());
    // Nothing was made.
    t.quit();
    let files = std::fs::read_dir(t.home.campaign_file("characters")).unwrap().count();
    assert_eq!(files, 4);
}

#[test]
fn each_choice_says_what_it_gives() {
    let mut t = start(Tui::scenario("creation.row-hints", Home::party()), "Reader");
    // A species: its speed and skills; a background: skills, increases, feat.
    t.wait("speed 30 ft · any 1 skill");
    answer(&mut t, "Choose a species", "Kindred");
    see(&mut t, "Choose a background");
    t.wait("Archivist  History, Investigation · +2/+1 INT/WIS/CHA");
    answer(&mut t, "Choose a background", "Archivist");
    until(&mut t, "Choose your starting class");
    // A class: its hit die, main abilities and saves.
    t.wait("d10 · DEX & WIS · saves STR, DEX");
}

#[test]
fn the_steps_list_what_was_chosen_in_each() {
    let mut t = start(Tui::scenario("creation.checklist", Home::party()), "Lister");
    answer(&mut t, "Choose a species", "Kindred");
    answer(&mut t, "Choose a background", "Archivist");
    see(&mut t, "Ability scores · Standard array");
    // Under each finished step, what was chosen in it.
    let s = t.screen();
    let lines: Vec<String> = s.lines().map(squash).collect();
    let at = lines.iter().position(|l| l.contains("✓ Background")).unwrap_or_else(|| panic!("{s}"));
    assert!(lines[at + 1].contains("│ Background") && lines[at + 2].contains("│ Archivist"), "{s}");
    let species = lines.iter().position(|l| l.contains("✓ Species")).unwrap();
    assert!(lines[species + 2].contains("│ Kindred"), "{s}");
    assert!(lines.iter().any(|l| l.contains("▸ Ability scores")), "{s}");
}
