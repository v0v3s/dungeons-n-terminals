import type { Page } from "@playwright/test";
import { dialog, expect, openSheet, pick, sheetTab, status, test } from "./fixtures";

const pane = (page: Page, title: string) =>
  page.locator("section.pane", {
    has: page.locator(".title", { hasText: title }),
  });
const row = (page: Page, name: string) => page.locator(".features .row", { hasText: name });
/// The overview's tile with this label (Speed, Armor Class, …).
const tile = (page: Page, label: string) => page.locator("button.tile", { hasText: label });

test("features are grouped by where they come from, and read", { tag: "@features.read" }, async ({ page }) => {
  await openSheet(page, "Features");
  await expect(pane(page, "Wizard 1")).toContainText("Arcane Recovery");
  await expect(pane(page, "Wizard 1")).toContainText("Ritual Adept");
  await expect(pane(page, "Species")).toContainText("Human");
  await expect(pane(page, "Background")).toContainText("Sage");
  await expect(pane(page, "Feats")).toContainText("Alert");
  await expect(pane(page, "Feats")).toContainText("Magic Initiate; Wizard");

  await row(page, "Arcane Recovery").locator("button.feature").click();
  await expect(dialog(page, "Arcane Recovery")).toContainText("choose expended spell slots to recover");
});

test("a feature's own rules change the sheet, and removing them changes it back", { tag: "@features.rules" }, async ({ page }) => {
  await openSheet(page);
  await expect(tile(page, "Speed")).toContainText("30");
  await sheetTab(page, "Features");
  await row(page, "Ritual Adept").getByRole("button", { name: "rules" }).click();
  const rules = dialog(page, "Rules · Ritual Adept");
  await expect(rules).toContainText("Nothing automates this feature yet");
  const box = rules.getByRole("textbox");
  await box.fill(`${await box.inputValue()}\n[[feature.effect]]\ntarget = "speed"\nadd = "10"\n`);
  // Checked as it is typed: what it changes on the character.
  await expect(rules.locator(".good").first()).toContainText("40");
  await rules.getByRole("button", { name: "Save" }).click();
  await expect(status(page)).toContainText("Saved your rules");
  await sheetTab(page, "Overview");
  await expect(tile(page, "Speed")).toContainText("40");

  await sheetTab(page, "Features");
  await row(page, "Ritual Adept").getByRole("button", { name: "rules" }).click();
  await expect(rules).toContainText("Your rules for this feature.");
  await rules.getByRole("button", { name: "Remove mine" }).click();
  await expect(status(page)).toContainText("Removed your rules");
  await sheetTab(page, "Overview");
  await expect(tile(page, "Speed")).toContainText("30");
});

test("broken rules say what is wrong and cannot be saved", { tag: "@features.rules" }, async ({ page }) => {
  await openSheet(page, "Features");
  await row(page, "Ritual Adept").getByRole("button", { name: "rules" }).click();
  const rules = dialog(page, "Rules · Ritual Adept");
  const box = rules.getByRole("textbox");
  await box.fill(`${await box.inputValue()}\n[[feature.effect]]\ntarget = "speed"\nadd = \n`);
  await expect(rules.locator(".bad").first()).toBeVisible();
  await expect(rules.getByRole("button", { name: "Save" })).toBeDisabled();
});

test("a weapon mastery is swapped for another weapon", { tag: "@features.mastery-swap" }, async ({ page }) => {
  await openSheet(page, "Features", "Ranger");
  const masteries = pane(page, "Weapon Mastery");
  await expect(masteries).toContainText("Longbow");
  await masteries.locator(".row", { hasText: "Longbow" }).getByRole("button", { name: "swap" }).click();
  await pick(page, ["Longsword"]);
  await expect(masteries).toContainText("Longsword");
  await expect(masteries).not.toContainText("Longbow");
  await expect(masteries).toContainText("Scimitar");
});
