import { adb } from "./android";
import { drag, swipe } from "./touch";
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

test("swiping changes the tab, and the select shows it", { tag: ["@sheet.swipe-tabs", "@android"] }, async ({ page }) => {
  const select = page.locator(".sheet .tabs select");
  for (const next of ["Features", "Spells", "Inventory"]) {
    await swipe(page, page.locator(".sheet > .body"), 0.8, 0.2);
    await expect(select).toHaveValue(next);
  }
  // Notes is the character's note, elsewhere: Inventory is the last swiped to.
  await swipe(page, page.locator(".sheet > .body"), 0.8, 0.2);
  await expect(select).toHaveValue("Inventory");
  await swipe(page, page.locator(".sheet > .body"), 0.2, 0.8);
  await expect(select).toHaveValue("Spells");
});

test("dragging shows the next tab; short of 45% it slides back, past it the tab opens", { tag: ["@sheet.swipe-tabs", "@android"] }, async ({ page }) => {
  const body = page.locator(".sheet > .body");
  const active = page.locator(".sheet .tabs select");
  const hint = page.locator(".sheet .swipe-hint");
  // A third of the way: the gap names Features, not yet lit; it slides back.
  await drag(page, body, 0.8, [0.65, 0.5], async () => {
    await expect(hint).toContainText("Features");
    await expect(hint).not.toHaveClass(/ready/);
  });
  await expect(hint).toHaveCount(0);
  await expect(active).toHaveValue("Overview");
  // Half the way: it opens.
  await drag(page, body, 0.8, [0.65, 0.5, 0.3], async () => {
    await expect(hint).toHaveClass(/ready/);
  });
  await expect(active).toHaveValue("Features");
  await expect(hint).toHaveCount(0);
});
