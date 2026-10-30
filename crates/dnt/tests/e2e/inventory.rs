use crate::harness::*;
use crate::seed::Home;

/// Open a character of the party (roster order) on a tab of the sheet.
fn sheet(t: &mut Tui, name: &str, tabs: usize) {
    let at = ["Brawler", "Hexa", "Ranger", "Tester"].iter().position(|n| *n == name).unwrap();
    t.send("2").wait(name);
    t.send(&"j".repeat(at)).send(ENTER).wait(&format!("◆ {name}"));
    t.send(&TAB.repeat(tabs));
}

fn inventory_of(t: &mut Tui, name: &str) {
    sheet(t, name, 3);
    t.wait("╭ Inventory");
}

/// The item the detail pane shows (the one under the cursor).
fn focus(t: &Tui) -> String {
    focus_on(&t.screen())
}

fn focus_on(screen: &str) -> String {
    let title = screen.lines().find(|l| l.contains("╭ Inventory")).unwrap_or("");
    title.split('╭').nth(3).unwrap_or("").trim_matches(|c: char| c == '─' || c == '╮' || c.is_whitespace()).to_string()
}

/// Move the cursor onto an item, from the top.
#[track_caller]
fn to_item(t: &mut Tui, name: &str) {
    t.send(&"k".repeat(30));
    let first = row_names(t).into_iter().next().unwrap_or_default();
    t.wait_until("the cursor is on the first item", |s| {
        let f = focus_on(s);
        !f.is_empty() && first.starts_with(&f)
    });
    loop {
        let here = focus(t);
        if here == name {
            return;
        }
        t.send("j");
        // Wait for the cursor to move; at the last item it does not.
        let start = std::time::Instant::now();
        while focus(t) == here && start.elapsed() < std::time::Duration::from_secs(1) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if focus(t) == here {
            panic!("[{}] no item {name:?} to move to; the screen:\n{}", t.scenario, t.screen());
        }
    }
}

/// The item names in the inventory list, top to bottom.
fn row_names(t: &Tui) -> Vec<String> {
    let s = t.screen();
    let lines: Vec<&str> = s.lines().collect();
    let top = lines.iter().position(|l| l.contains("╭ Inventory")).expect("the inventory");
    lines[top + 1..]
        .iter()
        .filter_map(|l| l.split('│').nth(1))
        .take_while(|c| !c.contains("────"))
        .map(|c| c.trim_start_matches(|ch: char| " ◉◈★☆▾▸".contains(ch)).split("  ").next().unwrap_or("").trim().to_string())
        .filter(|n| !n.is_empty() && n != "item")
        .collect()
}

/// The inventory row of an item: its text in the list pane.
#[track_caller]
fn row(t: &Tui, name: &str) -> String {
    let s = t.screen();
    s.lines()
        .filter_map(|l| l.split('│').nth(1))
        .find(|cell| cell.split_whitespace().collect::<Vec<_>>().join(" ").contains(name))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("[{}] no item row {name:?}; the screen:\n{s}", t.scenario))
}

/// Whether an item's row is indented into a container.
fn inside(t: &Tui, name: &str) -> bool {
    row(t, name).starts_with("      ")
}

/// Armor Class as the overview shows it.
fn armor_class(t: &Tui) -> String {
    let s = t.screen();
    let lines: Vec<&str> = s.lines().collect();
    let at = lines.iter().position(|l| l.contains("Armor Class")).expect("the overview");
    lines[at - 2].split('│').nth(3).unwrap().trim().to_string()
}

/// Add an item from the library.
fn add(t: &mut Tui, filter: &str, name: &str) {
    t.send("a").wait("Add an item");
    t.typed(filter).send(ENTER).wait(&format!("Added {name}"));
}

/// Add an item from the library that is not the first match (magic items
/// made from it sort before it): move down to it.
fn add_below(t: &mut Tui, filter: &str, down: usize, name: &str) {
    t.send("a").wait("Add an item");
    t.typed(filter).wait(name).send(&DOWN.repeat(down)).send(ENTER).wait(&format!("Added {name}"));
}

/// Unpack Tester's Scholar’s Pack into its Backpack.
fn unpack(t: &mut Tui) {
    to_item(t, "Scholar’s Pack");
    t.send("p").wait("Unpacked Scholar’s Pack").wait("▾ Backpack");
}

/// Whether the inventory list has a row for an item.
fn listed(t: &Tui, name: &str) -> bool {
    t.screen().lines().filter_map(|l| l.split('│').nth(1)).any(|cell| cell.contains(name))
}

/// The container settings prompt.
const CONTAINER: &str = "Container: 30 lb · weightless · no limit · off · empty resets";

/// Type into a prompt that starts with text, clearing it first.
fn retype(t: &mut Tui, text: &str) {
    t.send(&BACKSPACE.repeat(30)).typed(text).send(ENTER);
}

#[test]
fn adding_an_item_from_the_library() {
    let mut t = Tui::scenario("inventory.add", Home::party());
    inventory_of(&mut t, "Tester");
    t.send("a").wait("Add an item").wait("Create new item…").wait("Custom item…");
    t.typed("dice").wait("Dice").send(ENTER).wait("Added Dice");
    assert!(!inside(&t, "Dice"), "{}", t.screen());
    assert_eq!(focus(&t), "Dice");
    // A magic item that needs attunement says so.
    add(&mut t, "ring of prot", "Ring of Protection");
    assert!(row(&t, "Ring of Protection").contains("☆ Ring of Protection"), "{}", t.screen());
}

#[test]
fn adding_a_custom_item_by_name() {
    let mut t = Tui::scenario("inventory.add-custom", Home::party());
    inventory_of(&mut t, "Tester");
    t.send("a").wait("Add an item").typed("custom").send(ENTER).wait("Custom item name");
    t.typed("Lucky Feather").send(ENTER).wait_gone("Custom item name");
    assert!(row(&t, "Lucky Feather").contains(" 1 "), "{}", t.screen());
    assert_eq!(focus(&t), "Lucky Feather");
}

#[test]
fn adding_into_the_container_under_the_cursor() {
    let mut t = Tui::scenario("inventory.add-into", Home::party());
    inventory_of(&mut t, "Tester");
    unpack(&mut t);
    // The cursor on an item in the Backpack: new items go in there too.
    to_item(&mut t, "Ink");
    assert!(inside(&t, "Ink"), "{}", t.screen());
    add(&mut t, "dice", "Dice");
    assert!(inside(&t, "Dice"), "{}", t.screen());
    t.send("a").wait("Add an item").typed("custom").send(ENTER).wait("Custom item name");
    t.typed("Lucky Feather").send(ENTER).wait("Lucky Feather");
    assert!(inside(&t, "Lucky Feather"), "{}", t.screen());
    // On a loose item, new items stay loose.
    to_item(&mut t, "Quarterstaff");
    add(&mut t, "bell", "Bell");
    assert!(!inside(&t, "Bell"), "{}", t.screen());
}

#[test]
fn wearing_armor_and_holding_a_weapon_and_a_shield() {
    let mut t = Tui::scenario("inventory.equip", Home::party());
    sheet(&mut t, "Tester", 0);
    assert_eq!(armor_class(&t), "12");
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    // Armor is worn in a slot: AC follows.
    add_below(&mut t, "padded", 6, "Padded Armor");
    t.send(" ").wait("Wear Padded Armor").wait("Body").send(ENTER).wait("Wearing Padded Armor (Body)");
    assert!(row(&t, "Padded Armor").contains("◉ Padded Armor"), "{}", t.screen());
    assert!(t.line("Body ").contains("Padded Armor"), "{}", t.screen());
    // A weapon is held in a hand.
    to_item(&mut t, "Dagger");
    t.send(" ").wait("Dagger: main hand");
    assert!(row(&t, "Dagger").contains("◈ Dagger"), "{}", t.screen());
    assert!(t.line("Main hand").contains("Dagger"), "{}", t.screen());
    // A shield too.
    add_below(&mut t, "shield", 8, "Shield");
    t.send(" ").wait_until("the shield is in hand", |s| s.lines().any(|l| l.contains("Off hand") && l.contains("Shield")));
    // Padded Armor 11 + Dexterity 2, and the Shield 2.
    t.send(TAB).send(TAB).wait("Armor Class");
    assert_eq!(armor_class(&t), "15", "{}", t.screen());
    assert!(t.line("main  Dagger").contains("1d4 + 2 piercing"), "{}", t.screen());
    // Put away: AC and attacks follow.
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    to_item(&mut t, "Padded Armor");
    t.send(" ").wait_until("the armor is off", |s| !s.contains("◉ Padded Armor"));
    to_item(&mut t, "Dagger");
    t.send(" ").wait_until("the dagger is put away", |s| !s.contains("◈ Dagger"));
    to_item(&mut t, "Shield");
    t.send(" ").wait_until("the shield is put away", |s| !s.contains("◈ Shield"));
    t.send(TAB).send(TAB).wait("Armor Class");
    assert_eq!(armor_class(&t), "12", "{}", t.screen());
    assert!(!t.shows("main  Dagger"), "{}", t.screen());
}

#[test]
fn attuning_to_an_item_and_ending_it() {
    let mut t = Tui::scenario("inventory.attune", Home::party());
    inventory_of(&mut t, "Tester");
    add(&mut t, "ring of prot", "Ring of Protection");
    t.send(" ").wait("Wear Ring of Protection").send(ENTER).wait("Wearing Ring of Protection");
    t.send("A").wait("Attuned to Ring of Protection").wait("Attunement 1 / 3");
    assert!(row(&t, "Ring of Protection").contains("★ Ring of Protection"), "{}", t.screen());
    t.send(TAB).send(TAB).wait("Armor Class");
    assert_eq!(armor_class(&t), "13", "{}", t.screen());
    // Its saves bonus too: Strength +2 becomes +3.
    assert!(t.line("Strength").contains("+3"), "{}", t.screen());
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    to_item(&mut t, "Ring of Protection");
    t.send("A").wait("Attunement 0 / 3");
    t.send(TAB).send(TAB).wait("Armor Class");
    assert_eq!(armor_class(&t), "12", "{}", t.screen());
}

#[test]
fn changing_how_many() {
    let mut t = Tui::scenario("inventory.quantity", Home::party());
    inventory_of(&mut t, "Tester");
    to_item(&mut t, "Dagger");
    // Name, how many, weight.
    let shows = |n: &'static str, lb: &'static str| {
        move |s: &str| s.lines().filter_map(|l| l.split('│').nth(1)).any(|c| c.split_whitespace().collect::<Vec<_>>() == ["Dagger", n, lb])
    };
    t.wait_until("2 daggers", shows("2", "2.0"));
    t.send("+").wait_until("3 daggers", shows("3", "3.0"));
    t.send("-").wait_until("2 daggers", shows("2", "2.0"));
    t.send("-").wait_until("1 dagger", shows("1", "1.0"));
}

#[test]
fn removing_an_item() {
    let mut t = Tui::scenario("inventory.remove", Home::party());
    inventory_of(&mut t, "Tester");
    to_item(&mut t, "Arcane Focus (Quarterstaff)");
    t.send("x").wait("Removed Arcane Focus (Quarterstaff)");
    assert!(!listed(&t, "Arcane Focus"), "{}", t.screen());
    assert_eq!(focus(&t), "Robe");
    // Gone from the attacks too.
    t.send(BACKTAB).send(BACKTAB).send(BACKTAB).wait("Armor Class");
    assert!(t.shows("Dagger") && !t.shows("Arcane Focus"), "{}", t.screen());
}

#[test]
fn putting_an_item_in_a_container_and_out() {
    let mut t = Tui::scenario("inventory.put-in", Home::party());
    inventory_of(&mut t, "Tester");
    unpack(&mut t);
    to_item(&mut t, "Dagger");
    t.send("M").wait("Put Dagger in…").wait("Carried loose");
    t.typed("backpack").send(ENTER).wait_gone("Put Dagger in…");
    assert!(inside(&t, "Dagger"), "{}", t.screen());
    // 17 lb from the pack, and the two daggers.
    assert!(row(&t, "Backpack").contains("Backpack 19/30 lb"), "{}", t.screen());
    to_item(&mut t, "Dagger");
    t.send("M").wait("Put Dagger in…").typed("loose").send(ENTER).wait_gone("Put Dagger in…");
    assert!(!inside(&t, "Dagger"), "{}", t.screen());
    assert!(row(&t, "Backpack").contains("Backpack 17/30 lb"), "{}", t.screen());
}

#[test]
fn unpacking_an_equipment_pack() {
    let mut t = Tui::scenario("inventory.unpack", Home::party());
    inventory_of(&mut t, "Tester");
    assert!(!t.shows("Ink Pen"), "{}", t.screen());
    to_item(&mut t, "Scholar’s Pack");
    t.send("p").wait("Unpacked Scholar’s Pack: 8 items").wait("▾ Backpack");
    assert!(!listed(&t, "Scholar’s Pack"), "{}", t.screen());
    // (Its Book and Parchment share their names with the Sage's, loose.)
    for item in ["Ink", "Ink Pen", "Lamp", "Oil", "Tinderbox"] {
        assert!(inside(&t, item), "{item}: {}", t.screen());
    }
    assert!(row(&t, "Oil").split_whitespace().any(|c| c == "10"), "{}", t.screen());
}

#[test]
fn container_settings_and_making_an_item_a_container() {
    let mut t = Tui::scenario("inventory.container", Home::party());
    inventory_of(&mut t, "Tester");
    // Any item can hold things.
    to_item(&mut t, "Spellbook");
    t.send("w").wait(CONTAINER).typed("10").send(ENTER).wait("Container: 10 lb");
    assert!(row(&t, "Spellbook").contains("Spellbook 0/10 lb"), "{}", t.screen());
    to_item(&mut t, "Book (history)");
    t.send("M").wait("Put Book (history) in…").typed("spellbook").send(ENTER).wait_gone("Put Book (history) in…");
    assert!(inside(&t, "Book (history)"), "{}", t.screen());
    // A container's own settings change: a weightless one adds nothing.
    unpack(&mut t);
    let carried = t.line("Carried");
    to_item(&mut t, "Backpack");
    t.send("w").wait(CONTAINER);
    retype(&mut t, "30 weightless");
    t.wait("Container: 30 lb, weightless");
    t.wait_until("the backpack's contents weigh nothing", |s| !s.contains(&carried));
    // 50 lb, less the 17 lb in the Backpack.
    assert!(t.line("Carried").contains("33.0"), "{}", t.screen());
    // And off: no longer a container.
    to_item(&mut t, "Spellbook");
    t.send("w").wait(CONTAINER);
    retype(&mut t, "off");
    t.wait("Not a container");
    // No capacity on its row ("Spellbook" itself has an "lb" in it).
    assert!(!row(&t, "Spellbook").contains(" lb"), "{}", t.screen());
}

#[test]
fn a_monk_weapon_follows_martial_arts() {
    let mut t = Tui::scenario("inventory.monk-weapon", Home::party());
    inventory_of(&mut t, "Brawler");
    assert!(row(&t, "Dagger").contains("monk weapon"), "{}", t.screen());
    to_item(&mut t, "Dagger");
    t.send("m").wait("Dagger is not a monk weapon");
    assert!(!row(&t, "Dagger").contains("monk weapon"), "{}", t.screen());
    t.send(TAB).send(TAB).wait("Armor Class");
    assert!(t.line("  Dagger ").contains("1d4 + 4 piercing"), "{}", t.screen());
    // Marked again: the Martial Arts die.
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    to_item(&mut t, "Dagger");
    t.send("m").wait("monk weapon");
    t.send(TAB).send(TAB).wait("Armor Class");
    assert!(t.line("  Dagger ").contains("1d6 + 4 piercing"), "{}", t.screen());
}

#[test]
fn a_monk_weapon_without_martial_arts() {
    let mut t = Tui::scenario("inventory.monk-weapon", Home::party());
    inventory_of(&mut t, "Tester");
    to_item(&mut t, "Dagger");
    t.send("m").wait("Dagger is marked as a monk weapon (it counts once you have Martial Arts)");
    // Without Martial Arts it does not count: the attack stays the same.
    t.send(TAB).send(TAB).wait("Armor Class");
    assert!(t.line("  Dagger ").contains("1d4 + 2 piercing"), "{}", t.screen());
    // The mark was kept: pressed again, it goes.
    t.send(BACKTAB).send(BACKTAB).wait("╭ Inventory");
    to_item(&mut t, "Dagger");
    t.send("m").wait("Dagger is not a monk weapon");
}

#[test]
fn changing_the_coins() {
    let mut t = Tui::scenario("inventory.coins", Home::party());
    inventory_of(&mut t, "Tester");
    let coins = |want: &'static str| {
        move |s: &str| {
            s.lines()
                .filter_map(|l| l.split('│').nth(1))
                .any(|c| c.split_whitespace().collect::<Vec<_>>().join(" ") == format!("Coins {want}"))
        }
    };
    let change = |t: &mut Tui, text: &str| {
        t.send("g").wait("Coins (+15gp, -3sp, 20 sets gold)").typed(text).send(ENTER).wait_gone("Coins (+15gp");
    };
    t.wait_until("13 gold", coins("13 gp"));
    change(&mut t, "+15gp");
    t.wait_until("28 gold", coins("28 gp"));
    change(&mut t, "+5sp");
    change(&mut t, "-3sp");
    t.wait_until("28 gold and 2 silver", coins("28 gp 2 sp"));
    // A number alone sets the gold.
    change(&mut t, "20");
    t.wait_until("20 gold and 2 silver", coins("20 gp 2 sp"));
    change(&mut t, "lots");
    t.wait("Could not read");
}

#[test]
fn folding_a_container() {
    let mut t = Tui::scenario("inventory.fold", Home::party());
    inventory_of(&mut t, "Tester");
    unpack(&mut t);
    to_item(&mut t, "Backpack");
    t.send(ENTER).wait("▸ Backpack").wait_gone("Ink Pen");
    t.send(ENTER).wait("▾ Backpack").wait("Ink Pen");
    // ← folds from inside (to the container first), → unfolds.
    to_item(&mut t, "Lamp");
    t.send(LEFT).wait_until("the cursor is on the Backpack", |s| focus_on(s) == "Backpack");
    t.send(LEFT).wait("▸ Backpack").wait_gone("Lamp");
    t.send(RIGHT).wait("▾ Backpack").wait("Lamp");
}
