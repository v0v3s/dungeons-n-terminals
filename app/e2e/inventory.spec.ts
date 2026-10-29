import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, openSheet, pick, sheetTab, status, test } from "./fixtures";

const line = (page: Page, name: string) => page.locator("button.line", { hasText: name });

/// Pick a library item by its exact name: magic items made from it (+1
/// Shield, Sentinel Shield…) match its name too, and sort before it.
async function pickExactly(page: Page, name: string) {
  const picker = page.getByRole("dialog").last();
  await picker.getByRole("textbox").fill(name);
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const exact = new RegExp(`^${escaped}$`);
  await picker.locator("li button").filter({ has: page.locator(".label", { hasText: exact }) }).first().click();
}

async function unpack(page: Page) {
  await line(page, "Scholar’s Pack").click();
  await choose(page, "Scholar’s Pack", "Unpack");
  await expect(status(page)).toContainText("Unpacked Scholar’s Pack");
}

test("a pack unpacks into its container", { tag: "@inventory.unpack" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await expect(line(page, "Backpack")).toBeVisible();
  await expect(line(page, "Ink Pen")).toBeVisible();
  await expect(line(page, "Tinderbox")).toBeVisible();
  await expect(line(page, "Scholar’s Pack")).toHaveCount(0);
});

test("a container folds its contents away", { tag: "@inventory.fold" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await page.getByRole("button", { name: "Fold" }).click();
  await expect(line(page, "Ink Pen")).toHaveCount(0);
  await page.getByRole("button", { name: "Unfold" }).click();
  await expect(line(page, "Ink Pen")).toBeVisible();
});

test("a container's settings change, and any item can become one", { tag: "@inventory.container" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await line(page, "Backpack").click();
  await choose(page, "Backpack", "Container settings…");
  await answer(page, "Backpack: container", "weightless");
  await expect(line(page, "Backpack")).toContainText("weightless");

  await line(page, "Spellbook").click();
  await choose(page, "Spellbook", "Make it a container…");
  await answer(page, "Spellbook: container", "5");
  await expect(line(page, "Spellbook")).toContainText("/5 lb");
});

test("items go straight into a container", { tag: "@inventory.add-into" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await line(page, "Backpack").click();
  await choose(page, "Backpack", "Add a custom item into it…");
  await answer(page, "A custom item", "Lucky Pebble");
  const pebble = page.locator(".lrow", { hasText: "Lucky Pebble" });
  await expect(pebble).toBeVisible();
  expect(await pebble.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft))).toBeGreaterThan(0);

  await line(page, "Backpack").click();
  await choose(page, "Backpack", "Add an item into it…");
  await pickExactly(page, "Dice");
  const dice = page.locator(".lrow", { hasText: "Dice" });
  await expect(dice).toBeVisible();
  expect(await dice.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft))).toBeGreaterThan(0);
});

test("a monk marks weapons as monk weapons; others are not asked", { tag: "@inventory.monk-weapon" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await line(page, "Dagger").click();
  await expect(dialog(page, "Dagger").getByRole("button", { name: /monk weapon/ })).toHaveCount(0);
  await page.keyboard.press("Escape");

  await openSheet(page, "Inventory", "Brawler");
  await line(page, "Dagger").click();
  const toggle = dialog(page, "Dagger").getByRole("button", { name: /monk weapon/ });
  const before = await toggle.textContent();
  await toggle.click();
  await line(page, "Dagger").click();
  await expect(dialog(page, "Dagger").getByRole("button", { name: /monk weapon/ })).not.toHaveText(before ?? "");
});

test("+ Homebrew makes a campaign item from a template and carries it", { tag: "@inventory.homebrew-item" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await page.getByRole("button", { name: "+ Homebrew" }).click();
  const menu = page.getByRole("dialog", { name: "A new homebrew item" });
  await menu.getByRole("button").first().click();
  const prompt = page.getByRole("dialog").last();
  await prompt.getByRole("textbox").fill("Test Trinket");
  await prompt.getByRole("textbox").press("Enter");
  await page.keyboard.press("Escape");
  await expect(line(page, "Test Trinket")).toBeVisible();
});

/// Add an item from the library through + Add item.
async function addItem(page: Page, name: string) {
  await page.getByRole("button", { name: "+ Add item" }).click();
  await expect(dialog(page, "Add an item")).toBeVisible();
  await pickExactly(page, name);
  await expect(line(page, name)).toBeVisible();
}

/// The sheet's Armor Class, read on the Overview tab (then back to Inventory).
async function armorClass(page: Page): Promise<number> {
  await sheetTab(page, "Overview");
  const ac = Number(await page.locator("button.tile", { hasText: "Armor Class" }).locator("b").textContent());
  await sheetTab(page, "Inventory");
  return ac;
}

/// An Equipment slot's row (Body, Main hand, …).
const slot = (page: Page, label: string) => page.locator(".equipment .slot").filter({ has: page.getByText(label, { exact: true }) });

test("an item is added from the library", { tag: "@inventory.add" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await expect(line(page, "Dice")).toHaveCount(0);
  await addItem(page, "Dice");
});

test("a custom item is added by its name", { tag: "@inventory.add-custom" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await page.getByRole("button", { name: "+ Custom" }).click();
  await answer(page, "A custom item", "Strange Feather");
  await expect(line(page, "Strange Feather")).toBeVisible();
  // A custom item has no entry to read.
  await line(page, "Strange Feather").click();
  await expect(dialog(page, "Strange Feather").getByRole("button", { name: "Read it" })).toHaveCount(0);
});

test("armor is worn in a slot, a weapon and a shield held; AC and attacks follow", { tag: "@inventory.equip" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const bare = await armorClass(page);

  await addItem(page, "Padded Armor");
  await line(page, "Padded Armor").click();
  await choose(page, "Padded Armor", "Wear it in…");
  await expect(dialog(page, "Wear it where?")).toBeVisible();
  await pick(page, ["Body"]);
  await expect(page.getByRole("checkbox", { name: "Equipped: Padded Armor" })).toHaveAttribute("aria-checked", "true");
  await expect(slot(page, "Body")).toContainText("Padded Armor");
  // Padded Armor: 11 + Dexterity, against 10 + Dexterity bare.
  const coat = await armorClass(page);
  expect(coat).toBe(bare + 1);

  await addItem(page, "Shield");
  await line(page, "Shield").click();
  await choose(page, "Shield", "Hold in the off hand");
  await expect(slot(page, "Off hand")).toContainText("Shield");
  expect(await armorClass(page)).toBe(coat + 2);

  await line(page, "Dagger").click();
  await choose(page, "Dagger", "Hold in the main hand");
  await expect(slot(page, "Main hand")).toContainText("Dagger");
  await sheetTab(page, "Overview");
  await expect(page.locator(".attacks .held")).toContainText("Dagger");
  await sheetTab(page, "Inventory");

  // Put away and taken off: back to bare.
  await line(page, "Dagger").click();
  await choose(page, "Dagger", "Put it away");
  await expect(slot(page, "Main hand")).not.toContainText("Dagger");
  await line(page, "Shield").click();
  await choose(page, "Shield", "Put it away");
  await line(page, "Padded Armor").click();
  await choose(page, "Padded Armor", "Take it off");
  await expect(slot(page, "Body")).not.toContainText("Padded Armor");
  expect(await armorClass(page)).toBe(bare);
  await sheetTab(page, "Overview");
  await expect(page.locator(".attacks .held")).toContainText("nothing");
});

test("attuning to an item adds its bonuses; ending it takes them away", { tag: "@inventory.attune" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const bare = await armorClass(page);
  await addItem(page, "Ring of Protection");
  await expect(line(page, "Ring of Protection")).toContainText("☆");
  await line(page, "Ring of Protection").click();
  await choose(page, "Ring of Protection", "Equip (wear or hold)");
  await line(page, "Ring of Protection").click();
  await choose(page, "Ring of Protection", "Attune");
  await expect(line(page, "Ring of Protection")).toContainText("★");
  await expect(page.locator(".totals")).toContainText(/1\s*\/ 3\s*attuned/);
  expect(await armorClass(page)).toBe(bare + 1);

  await line(page, "Ring of Protection").click();
  await choose(page, "Ring of Protection", "End attunement");
  await expect(line(page, "Ring of Protection")).not.toContainText("★");
  await expect(page.locator(".totals")).toContainText(/0\s*\/ 3\s*attuned/);
  expect(await armorClass(page)).toBe(bare);
});

test("one more, one fewer", { tag: "@inventory.quantity" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const knives = line(page, "Dagger");
  await expect(knives).toContainText("×2");
  await knives.click();
  await choose(page, "Dagger", "One more");
  await expect(knives).toContainText("×3");
  await knives.click();
  await choose(page, "Dagger", "One fewer");
  await knives.click();
  await choose(page, "Dagger", "One fewer");
  await expect(knives).not.toContainText("×");
  // One left: no fewer to take.
  await knives.click();
  await expect(dialog(page, "Dagger").getByRole("button", { name: "One fewer" })).toHaveCount(0);
});

test("an item is removed", { tag: "@inventory.remove" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await line(page, "Arcane Focus").click();
  await choose(page, "Arcane Focus (Quarterstaff)", "Remove");
  await expect(line(page, "Arcane Focus")).toHaveCount(0);
  await expect(line(page, "Robe")).toBeVisible();
});

test("an item goes into a container and back out loose", { tag: "@inventory.put-in" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  const book = page.locator(".lrow", { hasText: "Spellbook" });
  const indent = () => book.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft));
  expect(await indent()).toBe(0);
  await line(page, "Spellbook").click();
  await choose(page, "Spellbook", "Put it in…");
  await expect(dialog(page, "Put it in…")).toBeVisible();
  await pick(page, ["Backpack"]);
  await expect.poll(indent).toBeGreaterThan(0);
  // Folding the backpack hides it with the rest.
  await page.locator(".lrow", { hasText: "Backpack" }).getByRole("button", { name: "Fold" }).click();
  await expect(line(page, "Spellbook")).toHaveCount(0);
  await page.locator(".lrow", { hasText: "Backpack" }).getByRole("button", { name: "Unfold" }).click();

  await line(page, "Spellbook").click();
  await choose(page, "Spellbook", "Put it in…");
  await pick(page, ["Carried loose"]);
  await expect.poll(indent).toBe(0);
});

test("coins change by amounts, or gold is set", { tag: "@inventory.coins" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const coins = page.locator("button.coins");
  const amount = async (c: string) => {
    const span = coins.locator("span", { has: page.locator(`b.c-${c}`) }).first();
    return (await span.count()) ? Number(await span.locator("b").textContent()) : 0;
  };
  const gold = await amount("gp");
  await coins.click();
  await answer(page, "Coins", "+15gp");
  await expect.poll(() => amount("gp")).toBe(gold + 15);
  await coins.click();
  await answer(page, "Coins", "+5sp");
  await expect.poll(() => amount("sp")).toBe(5);
  await coins.click();
  await answer(page, "Coins", "-3sp");
  await expect.poll(() => amount("sp")).toBe(2);
  await coins.click();
  await answer(page, "Coins", "20");
  await expect.poll(() => amount("gp")).toBe(20);
  await expect.poll(() => amount("sp")).toBe(2);
});

test("an item's entry is read from the inventory", { tag: "@inventory.read" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await line(page, "Robe").click();
  await choose(page, "Robe", "Read it");
  // The menu, titled the same, closes as the entry opens.
  await expect(dialog(page, "Robe").last()).toContainText("A Robe has vocational or ceremonial significance.");
  await expect(dialog(page, "Robe").last()).toContainText("System Reference Document 5.2.1");
});

test("the box by a line equips it or puts it away; other things have none", { tag: "@inventory.equip-box" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const staff = page.getByRole("checkbox", { name: "Equipped: Quarterstaff", exact: true });
  await staff.click();
  await expect(staff).toHaveAttribute("aria-checked", "true");
  await expect(slot(page, "Main hand")).toContainText("Quarterstaff");
  await staff.click();
  await expect(staff).toHaveAttribute("aria-checked", "false");
  await expect(slot(page, "Main hand")).not.toContainText("Quarterstaff");
  // Nothing to wear or hold: no box.
  await expect(page.getByRole("checkbox", { name: "Equipped: Parchment" })).toHaveCount(0);
});
