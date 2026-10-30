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
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Soldier");
    // Swap Strength's 15 with Dexterity's 14.
    see(&mut t, "Ability scores · Standard array");
    t.send("h");
    see(&mut t, "Strength ‹ 14 ›");
    t.send(ENTER);
    answer(&mut t, "Background (Soldier): ability increases", "+2 / +1");
    answer(&mut t, "Background (Soldier): +2 to which ability?", "Dexterity");
    answer(&mut t, "Background (Soldier): +1 to which ability?", "Constitution");
    // The Human's Versatile: an origin feat, Skilled: three skills of your choice.
    answer(&mut t, "Choose a feat", "Skilled");
    answer(&mut t, "Choose your starting class", "Ranger");
    tick(&mut t, "Ranger: choose 3 skill(s)", &["Nature", "Perception", "Survival"]);
    answer(&mut t, "Human: choose 1 skill(s)", "Medicine");
    tick(&mut t, "Skilled: choose 3", &["Acrobatics", "Insight", "Stealth"]);
    tick(&mut t, "Weapon Mastery: choose 2", &["Longbow", "Shortsword"]);
    answer(&mut t, "Class (Ranger): starting equipment", "A:");
    answer(&mut t, "Background (Soldier): starting equipment", "A:");
    answer(&mut t, "Choose a gaming set", "Playing Cards");

    see(&mut t, "╭ Review ─");
    see(&mut t, "Species Human");
    see(&mut t, "Background Soldier");
    see(&mut t, "Abilities STR 14 DEX 15 CON 13 INT 12 WIS 10");
    see(&mut t, "Background (Soldier) Dexterity +2, Constitution +1");
    see(&mut t, "Feat Savage Attacker");
    see(&mut t, "Feat Skilled");
    see(&mut t, "Class Ranger 1");
    see(&mut t, "Skills (Class) Nature, Perception,");
    see(&mut t, "Skills (Human) Medicine");
    see(&mut t, "Skills (Skilled) Acrobatics, Insight,");
    see(&mut t, "Weapon Mastery Longbow, Shortsword");
    see(&mut t, "Equipment Playing Cards");

    t.send(ENTER);
    see(&mut t, "Human · Soldier · Ranger 1 · Level 1");
    t.wait("Longbow").wait("Shortsword");
    // Dexterity 15 + 2 from the background.
    assert!(row(&t, "│ Dexterity").contains("Dexterity 17"), "{}", t.screen());
    t.send("q").wait("Newbie");
    assert!(row(&t, "Newbie").contains("Lv 1"), "{}", t.screen());
}

#[test]
fn a_lineage_of_the_species_is_chosen() {
    let mut t = start(Tui::scenario("creation.lineage", Home::party()), "Cinder");
    answer(&mut t, "Choose a species", "Tiefling");
    see(&mut t, "Tiefling: choose a lineage");
    t.wait("Tiefling; Abyssal Legacy");
    answer(&mut t, "Tiefling: choose a lineage", "Chthonic");
    see(&mut t, "Choose a background");
    to_review(&mut t);
    t.send(ENTER);
    see(&mut t, "Tiefling; Chthonic Legacy · ");
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
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Acolyte");
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
    see(&mut t, "Background (Acolyte): ability increases");
    to_review(&mut t);
    see(&mut t, "Abilities STR 15 DEX 15 CON 15 INT 8 WIS 8");
    see(&mut t, "(Point buy)");
    t.send(ENTER);
    see(&mut t, "Human · Acolyte");
    assert!(row(&t, "│ Strength").contains("Strength 15"), "{}", t.screen());
    assert!(row(&t, "│ Constitution").contains("Constitution 15"), "{}", t.screen());
}

#[test]
fn a_subclass_is_chosen_at_level_1() {
    let mut t = start(Tui::scenario("creation.subclass", Home::party()), "Tidecaller");
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Sage");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Hexer");
    see(&mut t, "Choose a subclass");
    t.wait("The Lantern").wait("The Tide");
    answer(&mut t, "Choose a subclass", "Tide");
    see(&mut t, "Hexer: choose 2 skill(s)");
    to_review(&mut t);
    see(&mut t, "Subclass The Tide");
    t.send(ENTER);
    see(&mut t, "Human · Sage · Hexer 1");
    t.send(TAB).wait("The Tide");
}

#[test]
fn a_class_features_choices_are_made_at_level_1() {
    let mut t = start(Tui::scenario("creation.feature-choice", Home::party()), "Warder");
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Sage");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Fighter");
    until(&mut t, "Fighting Style: choose one");
    t.wait("Archery").wait("Two-Weapon Fighting");
    answer(&mut t, "Fighting Style: choose one", "Two-Weapon");
    see(&mut t, "Weapon Mastery: choose 3");
    t.wait("Greatsword");
    // Three at most.
    for weapon in ["Flail", "Greatsword", "Longbow", "Scimitar"] {
        t.typed(weapon);
        t.send(" ").send(&ctrl('u'));
    }
    see(&mut t, "3/3 chosen");
    assert!(row(&t, "Scimitar").contains("○ Scimitar"), "{}", t.screen());
    t.send(ENTER);
    to_review(&mut t);
    see(&mut t, "Fighting Style Two-Weapon Fighting");
    see(&mut t, "Weapon Mastery Flail, Greatsword, Longbow");
    t.send(ENTER);
    see(&mut t, "Fighter 1 · Level 1");
    t.send(TAB).wait("Two-Weapon Fighting");
    let s = t.screen();
    let group: String = s.lines().skip_while(|l| !squash(l).contains("│ Weapon Mastery │")).collect::<Vec<_>>().join("\n");
    assert!(group.contains("Flail") && group.contains("Greatsword") && group.contains("Longbow"), "{s}");
    assert!(!group.contains("Scimitar"), "{s}");
}

#[test]
fn an_item_is_picked_for_an_equipment_choice() {
    let mut t = start(Tui::scenario("creation.equipment-pick", Home::party()), "Picker");
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Acolyte");
    until(&mut t, "Choose your starting class");
    answer(&mut t, "Choose your starting class", "Bard");
    until(&mut t, "Class (Bard): starting equipment");
    answer(&mut t, "Class (Bard): starting equipment", "A:");
    see(&mut t, "Choose a musical instrument");
    t.wait("Bagpipes").wait("Drum");
    answer(&mut t, "Choose a musical instrument", "Drum");
    answer(&mut t, "Background (Acolyte): starting equipment", "A:");
    see(&mut t, "Choose a holy symbol");
    t.wait("Amulet").wait("Emblem");
    t.typed("Emblem");
    t.send(ENTER);
    see(&mut t, "╭ Review ─");
    see(&mut t, "Drum (a musical instrument)");
    see(&mut t, "Emblem (a holy symbol)");
    t.send(ENTER);
    see(&mut t, "Bard 1 · Level 1");
    t.send(TAB).send(TAB).send(TAB);
    see(&mut t, "╭ Inventory");
    t.wait("Drum").wait("Emblem");
    assert!(!t.shows("Bagpipes"), "{}", t.screen());
    assert!(!t.shows("Amulet"), "{}", t.screen());
}

#[test]
fn going_back_changes_an_answer_and_esc_at_the_start_cancels() {
    let mut t = start(Tui::scenario("creation.back", Home::party()), "Undecided");
    answer(&mut t, "Choose a species", "Human");
    see(&mut t, "Choose a background");
    t.send(ESC);
    see(&mut t, "Choose a species");
    answer(&mut t, "Choose a species", "Tiefling");
    // The new species asks its own question.
    answer(&mut t, "Tiefling: choose a lineage", "Abyssal");
    see(&mut t, "Choose a background");
    assert!(!t.shows("Human"), "{}", t.screen());
    to_review(&mut t);
    see(&mut t, "Species Tiefling");
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
    see(&mut t, "Elf speed 30 ft · darkvision 60 ft · 1 of Insight");
    answer(&mut t, "Choose a species", "Human");
    see(&mut t, "Choose a background");
    see(&mut t, "Sage Arcana, History · +2/+1 CON/INT/WIS");
    answer(&mut t, "Choose a background", "Sage");
    until(&mut t, "Choose your starting class");
    // A class: its hit die, main abilities and saves.
    t.wait("d10 · DEX & WIS · saves STR, DEX");
}

#[test]
fn the_steps_list_what_was_chosen_in_each() {
    let mut t = start(Tui::scenario("creation.checklist", Home::party()), "Lister");
    answer(&mut t, "Choose a species", "Human");
    answer(&mut t, "Choose a background", "Sage");
    see(&mut t, "Ability scores · Standard array");
    // Under each finished step, what was chosen in it.
    let s = t.screen();
    let lines: Vec<String> = s.lines().map(squash).collect();
    let at = lines.iter().position(|l| l.contains("✓ Background")).unwrap_or_else(|| panic!("{s}"));
    assert!(lines[at + 1].contains("│ Background") && lines[at + 2].contains("│ Sage"), "{s}");
    let species = lines.iter().position(|l| l.contains("✓ Species")).unwrap();
    assert!(lines[species + 2].contains("│ Human"), "{s}");
    assert!(lines.iter().any(|l| l.contains("▸ Ability scores")), "{s}");
}
