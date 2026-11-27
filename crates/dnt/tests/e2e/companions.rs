use crate::harness::*;
use crate::sheet_play::{rolled, status, wait_status};
use crate::steps::*;

/// Open Mira's Companions tab.
fn companions(t: &mut Tui) {
    open_mira(t);
    t.send(&TAB.repeat(4)).wait("Companions  ");
}

/// Where the cursor goes from a companion's name: hit points,
/// initiative, the six abilities, then its skills and actions.
const FIRST_SKILL: usize = 9;

/// Open Mira's Spells tab.
fn spells(t: &mut Tui) {
    open_mira(t);
    t.send(&TAB.repeat(2)).wait("╭ Spells");
}

/// The spell the detail pane shows (the one under the cursor).
fn shown(t: &Tui) -> String {
    let s = t.screen();
    let lines: Vec<&str> = s.lines().collect();
    let top = lines.iter().position(|l| l.contains("╭ Spells")).expect("the spell list");
    lines[top + 1..top + 3].iter().filter_map(|l| l.split('│').nth(3)).map(str::trim).find(|s| !s.is_empty()).unwrap_or("").to_string()
}

/// Cast a spell from the Spells tab, choosing how (`ritual`, `level 4`).
#[track_caller]
fn cast(t: &mut Tui, spell: &str, how: &str) {
    t.send("g");
    for _ in 0..10 {
        if shown(t) == spell {
            break;
        }
        t.send("j");
    }
    assert_eq!(shown(t), spell, "{}", t.screen());
    t.send(ENTER).wait(&format!("Cast {spell}")).typed(how).send(ENTER);
}

#[test]
fn casting_a_familiar_spell_gives_one_familiar_with_its_own_card() {
    let mut t = Tui::scenario("companions.familiar", with_mira());
    spells(&mut t);
    // Mira has slots, and it is a ritual: the cast asks how.
    t.send("g");
    cast(&mut t, "Find Familiar", "ritual");
    t.wait("Familiar: which creature?");
    // The Owl is a creature it allows; a Wolf is not.
    t.typed("wolf").wait("Nothing matches.");
    t.send(&ctrl('u')).typed("owl").wait("CR 0 · Beast").send(ENTER);
    t.wait("Familiar: which type?").wait("Celestial").typed("fey").send(ENTER);
    wait_status(&mut t, "Owl joins (Companions tab)");
    // Its card.
    t.send(&TAB.repeat(2));
    see(&mut t, "╭ Owl ─");
    see(&mut t, "Tiny Fey, Unaligned");
    assert!(row(&t, "Hit Points").contains("1/1"), "{}", t.screen());
    see(&mut t, "Armor Class 11 Initiative +1");
    see(&mut t, "Speed 5 ft., Fly 60 ft.");
    see(&mut t, "Wisdom 12 +1 +1");
    see(&mut t, "Perception +5");
    see(&mut t, "Passive Perception 15");
    let talons = row(&t, "Talons");
    assert!(talons.contains("+3 1 slashing"), "{talons}");
    // Its Perception rolls, in its name.
    t.send(&"j".repeat(FIRST_SKILL)).send("r");
    wait_status(&mut t, "Owl: Perception: ");
    assert!((6..=25).contains(&rolled(&t, "Owl: Perception")), "{}", status(&t));
    // The spell gives one at a time: cast again, it says so.
    t.send(BACKTAB).send(BACKTAB).wait("╭ Spells");
    cast(&mut t, "Find Familiar", "ritual");
    t.wait("Familiar: which creature?").typed("owl").wait("CR 0 · Beast").send(ENTER);
    t.wait("Familiar: which type?").typed("fey").send(ENTER);
    wait_status(&mut t, "Find Familiar gives 1 at once");
    // It is kept with the character.
    let file = std::fs::read_to_string(t.home.campaign_file("characters/mira.toml")).unwrap();
    assert!(file.contains("creature:Owl|SRD52") && file.contains("fey"), "{file}");
    assert_eq!(file.matches("creature:Owl").count(), 1, "{file}");
}

#[test]
fn a_books_find_steed_cast_at_level_4_follows_the_level_and_its_owner() {
    let mut t = Tui::scenario("companions.summon", with_mira());
    spells(&mut t);
    // One creature: straight to its type; the level is the slot's.
    cast(&mut t, "Call Steed", "level 4");
    t.wait("Steed: which type?").typed("fey").send(ENTER);
    wait_status(&mut t, "Otherworldly Steed joins");
    t.send(&TAB.repeat(2));
    see(&mut t, "╭ Otherworldly Steed ─");
    see(&mut t, "Large Fey");
    see(&mut t, "Armor Class 14");
    assert!(row(&t, "Hit Points").contains("45/45"), "{}", t.screen());
    see(&mut t, "Fly 60 ft.");
    // Its slam: Mira's spell attack (+6), 1d8 + the spell's level; only the
    // Fey one.
    let slam = row(&t, "Otherworldly Slam (Fey)");
    assert!(slam.contains("+6 1d8 + 4 psychic"), "{slam}");
    assert_eq!(t.screen().matches("Otherworldly Slam").count(), 1, "{}", t.screen());
    assert!(!t.shows("Healing Touch") && !t.shows("Fell Glare"), "{}", t.screen());
    // It rolls: to hit, and its damage.
    t.send("G").send("r");
    wait_status(&mut t, "Otherworldly Steed: Otherworldly Slam (Fey): ");
    assert!(status(&t).contains("psychic damage"), "{}", status(&t));
}

#[test]
fn any_creature_follows_and_is_hurt_healed_renamed_and_dismissed() {
    let mut t = Tui::scenario("companions.follower", with_mira());
    // A creature of one's own: the Creature template, in the compendium.
    t.send("1").wait("Browse");
    t.send("n").wait("Create a campaign entry").wait("Creature");
    t.send(&"j".repeat(9)).send(ENTER);
    t.typed("Hireling").send(ENTER).wait("Created Hireling");
    t.send(ESC).wait("Browse");

    companions(&mut t);
    // No feature of Mira's gives one: straight to the creatures.
    t.wait("No companions yet.");
    t.send("a").wait("A follower: which creature?");
    // Dire Wolf, Giant Wolf Spider, Werewolf, Winter Wolf, Wolf.
    t.typed("wolf").wait("Winter Wolf").send(&DOWN.repeat(4)).send(ENTER);
    wait_status(&mut t, "Wolf joins");
    see(&mut t, "╭ Wolf ─");
    see(&mut t, "Follower · Wolf");
    assert!(row(&t, "Hit Points").contains("11/11"), "{}", t.screen());
    see(&mut t, "Armor Class 12");
    assert!(row(&t, "Bite").contains("+4 1d6 + 2 piercing"), "{}", t.screen());
    // Its Bite rolls to hit, with its damage.
    t.send("G").send("r");
    wait_status(&mut t, "Wolf: Bite: ");
    assert!(status(&t).contains("piercing damage"), "{}", status(&t));

    // Damage, then healing.
    t.send("-").wait("Damage to Wolf").typed("5").send(ENTER);
    wait_status(&mut t, "6/11 hit points");
    assert!(row(&t, "Hit Points").contains("6/11"), "{}", t.screen());
    t.send("+").wait("Heal Wolf").typed("3").send(ENTER);
    wait_status(&mut t, "9/11 hit points");

    // Renamed.
    t.send("m").wait("Rename…").typed("rename").send(ENTER).wait("Rename Wolf");
    t.send(&ctrl('u')).typed("Grey").send(ENTER);
    wait_status(&mut t, "Renamed");
    see(&mut t, "╭ Grey ─");
    // A condition.
    t.send("C").wait("Grey: conditions").send(" ");
    wait_status(&mut t, "Blinded");
    t.send(ESC);
    see(&mut t, "Conditions Blinded");
    // Away, and back.
    t.send("m").typed("away").send(ENTER);
    wait_status(&mut t, "Away");
    see(&mut t, "╭ Grey · away ─");
    see(&mut t, "Companions 0 · 1 away");
    t.send("m").typed("back").send(ENTER);
    wait_status(&mut t, "Back");
    see(&mut t, "╭ Grey ─");
    // It leaves.
    t.send("x").wait("Grey leaves the character?").send("y");
    wait_status(&mut t, "Grey leaves");
    t.wait("No companions yet.");

    // The Hireling joins the same way.
    t.send("a").wait("A follower: which creature?").typed("hireling").wait("Hireling").send(ENTER);
    wait_status(&mut t, "Hireling joins");
    assert!(row(&t, "Hit Points").contains("11/11"), "{}", t.screen());
    see(&mut t, "Armor Class 12");
    assert!(row(&t, "Strike").contains("+3 1d6 + 1 bludgeoning"), "{}", t.screen());
    t.send("G").send("r");
    wait_status(&mut t, "Hireling: Strike: ");
}
