import { dialog, expect, openSheet, test } from "./fixtures";

test.beforeEach(async ({ page }) => openSheet(page));

/// The saves, in the order of the abilities: Intelligence is the fourth.
const intSave = (page: import("@playwright/test").Page) => page.locator("button.num.save").nth(3);

test("holding a save explains it, and does not roll", async ({ page }) => {
  await intSave(page).click({ button: "right" });
  await expect(dialog(page, "Intelligence save")).toContainText("Intelligence + proficiency");
  await expect(page.locator(".status.roll")).toHaveCount(0);
});

test("tapping a save rolls it", async ({ page }) => {
  await intSave(page).click();
  await expect(page.locator(".status.roll")).toContainText("Intelligence");
});

test("tapping an ability's name explains its score", async ({ page }) => {
  await page.getByRole("button", { name: "Intelligence", exact: true }).click();
  await expect(dialog(page, "Intelligence")).toContainText("base score");
});

test("holding a tile explains it", async ({ page }) => {
  await page.getByRole("button", { name: /AC/ }).click({ button: "right" });
  await expect(dialog(page, "Armor Class")).toBeVisible();
});

test("holding a skill explains it", async ({ page }) => {
  await page.getByRole("button", { name: /Arcana/ }).click({ button: "right" });
  await expect(dialog(page, "Arcana")).toContainText("Intelligence + proficiency");
});
