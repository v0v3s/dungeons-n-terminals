import { adb } from "./android";
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

/// Touch and hold the middle of an element with a finger on the device's
/// screen (its WebView's place on the screen, plus the element's place).
async function touchAndHold(page: import("@playwright/test").Page, target: import("@playwright/test").Locator, ms = 900) {
  await target.scrollIntoViewIfNeeded();
  const box = (await target.boundingBox())!;
  const dpr = await page.evaluate(() => devicePixelRatio);
  adb("shell", "uiautomator", "dump", "/sdcard/dnt-e2e-ui.xml");
  const ui = adb("shell", "cat", "/sdcard/dnt-e2e-ui.xml");
  const at = ui.match(/class="android\.webkit\.WebView"[^>]*bounds="\[(\d+),(\d+)\]/);
  if (!at) throw new Error("No WebView on the screen");
  const x = Math.round(Number(at[1]) + (box.x + box.width / 2) * dpr);
  const y = Math.round(Number(at[2]) + (box.y + box.height / 2) * dpr);
  adb("shell", "input", "swipe", String(x), String(y), String(x), String(y), String(ms));
}

test("on a phone, touching and holding a number explains it without rolling", { tag: ["@explain.touch-hold", "@android"] }, async ({ page }) => {
  await touchAndHold(page, intSave(page));
  await expect(dialog(page, "Intelligence save")).toContainText("Intelligence + proficiency");
  await expect(status(page)).toHaveCount(0);
  await expect(page.locator(".rolled")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await touchAndHold(page, page.locator("button.skill", { hasText: "Arcana" }));
  await expect(dialog(page, "Arcana")).toContainText("Intelligence");
  await expect(status(page)).toHaveCount(0);
  await expect(page.locator(".rolled")).toHaveCount(0);
});
