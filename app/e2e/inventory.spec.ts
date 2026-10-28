import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, openSheet, pick, sheetTab, status, test } from "./fixtures";

const line = (page: Page, name: string) => page.locator("button.line", { hasText: name });

async function unpack(page: Page) {
  await line(page, "Scribe's Pack").click();
  await choose(page, "Scribe's Pack", "Unpack");
  await expect(status(page)).toContainText("Unpacked Scribe's Pack");
}

test("a pack unpacks into its container", { tag: "@inventory.unpack" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await expect(line(page, "Satchel")).toBeVisible();
  await expect(line(page, "Ink Pot")).toBeVisible();
  await expect(page.locator("button.line", { hasText: "a little bag of sand" })).toBeVisible();
  await expect(line(page, "Scribe's Pack")).toHaveCount(0);
});

test("a container folds its contents away", { tag: "@inventory.fold" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await page.getByRole("button", { name: "Fold" }).click();
  await expect(line(page, "Ink Pot")).toHaveCount(0);
  await page.getByRole("button", { name: "Unfold" }).click();
  await expect(line(page, "Ink Pot")).toBeVisible();
});

test("a container's settings change, and any item can become one", { tag: "@inventory.container" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await line(page, "Satchel").click();
  await choose(page, "Satchel", "Container settings…");
  await answer(page, "Satchel: container", "weightless");
  await expect(line(page, "Satchel")).toContainText("weightless");

  await line(page, "Grimoire").click();
  await choose(page, "Grimoire", "Make it a container…");
  await answer(page, "Grimoire: container", "5");
  await expect(line(page, "Grimoire")).toContainText("/5 lb");
});

test("items go straight into a container", { tag: "@inventory.add-into" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  await line(page, "Satchel").click();
  await choose(page, "Satchel", "Add a custom item into it…");
  await answer(page, "A custom item", "Lucky Pebble");
  const pebble = page.locator(".lrow", { hasText: "Lucky Pebble" });
  await expect(pebble).toBeVisible();
  expect(await pebble.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft))).toBeGreaterThan(0);

  await line(page, "Satchel").click();
  await choose(page, "Satchel", "Add an item into it…");
  const picker = page.getByRole("dialog").last();
  await picker.getByRole("textbox").fill("Bone Dice");
  await picker.locator("li button", { hasText: "Bone Dice" }).first().click();
  const dice = page.locator(".lrow", { hasText: "Bone Dice" });
  await expect(dice).toBeVisible();
  expect(await dice.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft))).toBeGreaterThan(0);
});

test("a monk marks weapons as monk weapons; others are not asked", { tag: "@inventory.monk-weapon" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await line(page, "Thornknife").click();
  await expect(dialog(page, "Thornknife").getByRole("button", { name: /monk weapon/ })).toHaveCount(0);
  await page.keyboard.press("Escape");

  await openSheet(page, "Inventory", "Brawler");
  await line(page, "Ashstaff").click();
  const toggle = dialog(page, "Ashstaff").getByRole("button", { name: /monk weapon/ });
  const before = await toggle.textContent();
  await toggle.click();
  await line(page, "Ashstaff").click();
  await expect(dialog(page, "Ashstaff").getByRole("button", { name: /monk weapon/ })).not.toHaveText(before ?? "");
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
  await pick(page, [name]);
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
  await expect(line(page, "Bone Dice")).toHaveCount(0);
  await addItem(page, "Bone Dice");
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

  await addItem(page, "Quilted Coat");
  await line(page, "Quilted Coat").click();
  await choose(page, "Quilted Coat", "Wear it in…");
  await expect(dialog(page, "Wear it where?")).toBeVisible();
  await pick(page, ["Body"]);
  await expect(page.getByRole("checkbox", { name: "Equipped: Quilted Coat" })).toHaveAttribute("aria-checked", "true");
  await expect(slot(page, "Body")).toContainText("Quilted Coat");
  const coat = await armorClass(page);
  expect(coat).toBe(bare + 1);

  await addItem(page, "Buckler");
  await line(page, "Buckler").click();
  await choose(page, "Buckler", "Hold in the off hand");
  await expect(slot(page, "Off hand")).toContainText("Buckler");
  expect(await armorClass(page)).toBe(coat + 2);

  await line(page, "Thornknife").click();
  await choose(page, "Thornknife", "Hold in the main hand");
  await expect(slot(page, "Main hand")).toContainText("Thornknife");
  await sheetTab(page, "Overview");
  await expect(page.locator(".attacks .held")).toContainText("Thornknife");
  await sheetTab(page, "Inventory");

  // Put away and taken off: back to bare.
  await line(page, "Thornknife").click();
  await choose(page, "Thornknife", "Put it away");
  await expect(slot(page, "Main hand")).not.toContainText("Thornknife");
  await line(page, "Buckler").click();
  await choose(page, "Buckler", "Put it away");
  await line(page, "Quilted Coat").click();
  await choose(page, "Quilted Coat", "Take it off");
  await expect(slot(page, "Body")).not.toContainText("Quilted Coat");
  expect(await armorClass(page)).toBe(bare);
  await sheetTab(page, "Overview");
  await expect(page.locator(".attacks .held")).toContainText("nothing");
});

test("attuning to an item adds its bonuses; ending it takes them away", { tag: "@inventory.attune" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const bare = await armorClass(page);
  await addItem(page, "Ring of Warding");
  await expect(line(page, "Ring of Warding")).toContainText("☆");
  await line(page, "Ring of Warding").click();
  await choose(page, "Ring of Warding", "Equip (wear or hold)");
  await line(page, "Ring of Warding").click();
  await choose(page, "Ring of Warding", "Attune");
  await expect(line(page, "Ring of Warding")).toContainText("★");
  await expect(page.locator(".totals")).toContainText(/1\s*\/ 3\s*attuned/);
  expect(await armorClass(page)).toBe(bare + 1);

  await line(page, "Ring of Warding").click();
  await choose(page, "Ring of Warding", "End attunement");
  await expect(line(page, "Ring of Warding")).not.toContainText("★");
  await expect(page.locator(".totals")).toContainText(/0\s*\/ 3\s*attuned/);
  expect(await armorClass(page)).toBe(bare);
});

test("one more, one fewer", { tag: "@inventory.quantity" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const knives = line(page, "Thornknife");
  await expect(knives).toContainText("×2");
  await knives.click();
  await choose(page, "Thornknife", "One more");
  await expect(knives).toContainText("×3");
  await knives.click();
  await choose(page, "Thornknife", "One fewer");
  await knives.click();
  await choose(page, "Thornknife", "One fewer");
  await expect(knives).not.toContainText("×");
  // One left: no fewer to take.
  await knives.click();
  await expect(dialog(page, "Thornknife").getByRole("button", { name: "One fewer" })).toHaveCount(0);
});

test("an item is removed", { tag: "@inventory.remove" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await line(page, "Ashstaff").click();
  await choose(page, "Ashstaff", "Remove");
  await expect(line(page, "Ashstaff")).toHaveCount(0);
  await expect(line(page, "Grimoire")).toBeVisible();
});

test("an item goes into a container and back out loose", { tag: "@inventory.put-in" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  await unpack(page);
  const grimoire = page.locator(".lrow", { hasText: "Grimoire" });
  const indent = () => grimoire.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft));
  expect(await indent()).toBe(0);
  await line(page, "Grimoire").click();
  await choose(page, "Grimoire", "Put it in…");
  await expect(dialog(page, "Put it in…")).toBeVisible();
  await pick(page, ["Satchel"]);
  await expect.poll(indent).toBeGreaterThan(0);
  // Folding the satchel hides it with the rest.
  await page.locator(".lrow", { hasText: "Satchel" }).getByRole("button", { name: "Fold" }).click();
  await expect(line(page, "Grimoire")).toHaveCount(0);
  await page.locator(".lrow", { hasText: "Satchel" }).getByRole("button", { name: "Unfold" }).click();

  await line(page, "Grimoire").click();
  await choose(page, "Grimoire", "Put it in…");
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
  await line(page, "Grimoire").click();
  await choose(page, "Grimoire", "Read it");
  // The menu, titled the same, closes as the entry opens.
  await expect(dialog(page, "Grimoire").last()).toContainText("A mage's spellbook.");
  await expect(dialog(page, "Grimoire").last()).toContainText("The Proving Grounds");
});

test("the box by a line equips it or puts it away; other things have none", { tag: "@inventory.equip-box" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  const staff = page.getByRole("checkbox", { name: "Equipped: Ashstaff", exact: true });
  await staff.click();
  await expect(staff).toHaveAttribute("aria-checked", "true");
  await expect(slot(page, "Main hand")).toContainText("Ashstaff");
  await staff.click();
  await expect(staff).toHaveAttribute("aria-checked", "false");
  await expect(slot(page, "Main hand")).not.toContainText("Ashstaff");
  // Nothing to wear or hold: no box.
  await expect(page.getByRole("checkbox", { name: "Equipped: Ink Quill" })).toHaveCount(0);
});
