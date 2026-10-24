import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, openSheet, status, test } from "./fixtures";

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
