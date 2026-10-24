import { dialog, expect, openSheet, status, test } from "./fixtures";

test.beforeEach(async ({ page }) => openSheet(page));

/// The saves, in the order of the abilities: Intelligence is the fourth.
const intSave = (page: import("@playwright/test").Page) => page.locator("button.num.save").nth(3);

test("holding a number explains it without rolling; tapping a name explains the score", { tag: "@explain.number" }, async ({ page }) => {
  await intSave(page).click({ button: "right" });
  await expect(dialog(page, "Intelligence save")).toContainText("Intelligence + proficiency");
  await expect(status(page)).toHaveCount(0);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Intelligence", exact: true }).click();
  await expect(dialog(page, "Intelligence")).toContainText("base score");
  await page.keyboard.press("Escape");

  await page.locator("button", { hasText: /AC$/ }).first().click({ button: "right" });
  await expect(dialog(page, "Armor Class")).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator("button.skill", { hasText: "Arcana" }).click({ button: "right" });
  await expect(dialog(page, "Arcana")).toContainText("Intelligence");
});

test("tapping a check, a save, a skill and initiative rolls them", { tag: "@roll.checks" }, async ({ page }) => {
  await intSave(page).click();
  await expect(page.locator(".status.roll")).toContainText("Intelligence");
  await page.locator("button.num:not(.save)").nth(1).click();
  await expect(page.locator(".status.roll")).toContainText("Dexterity");
  await page.locator("button.skill", { hasText: "Arcana" }).click();
  await expect(page.locator(".status.roll")).toContainText("Arcana");
  await page.locator("button", { hasText: /Init\.$/ }).first().click();
  await expect(page.locator(".status.roll")).toContainText("Initiative");
});
