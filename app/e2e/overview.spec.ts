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

/// A finger swipe across the element, from `from` to `to` of its width.
async function swipe(page: import("@playwright/test").Page, target: import("@playwright/test").Locator, from: number, to: number) {
  const box = (await target.boundingBox())!;
  const dpr = await page.evaluate(() => devicePixelRatio);
  const ui = (adb("shell", "uiautomator", "dump", "/sdcard/dnt-e2e-ui.xml"), adb("shell", "cat", "/sdcard/dnt-e2e-ui.xml"));
  const at = ui.match(/class="android\.webkit\.WebView"[^>]*bounds="\[(\d+),(\d+)\]/);
  if (!at) throw new Error("No WebView on the screen");
  const x = (f: number) => String(Math.round(Number(at[1]) + (box.x + box.width * f) * dpr));
  const y = String(Math.round(Number(at[2]) + (box.y + Math.min(box.height, 300) / 2) * dpr));
  adb("shell", "input", "swipe", x(from), y, x(to), y, "250");
}

test("swiping changes the tab, and the tab bar keeps it in sight", { tag: ["@sheet.swipe-tabs", "@android"] }, async ({ page }) => {
  const bar = page.locator(".sheet .tabs");
  const active = bar.locator("button.active");
  for (const next of ["Features", "Spells", "Inventory"]) {
    await swipe(page, page.locator(".sheet > .body"), 0.8, 0.2);
    await expect(active).toHaveText(next);
    // The open tab lies within the bar, not scrolled off its side.
    await expect
      .poll(async () => {
        const [a, b] = [(await active.boundingBox())!, (await bar.boundingBox())!];
        return a.x >= b.x - 1 && a.x + a.width <= b.x + b.width + 1;
      })
      .toBe(true);
  }
  await swipe(page, page.locator(".sheet > .body"), 0.2, 0.8);
  await expect(active).toHaveText("Spells");
});

/// A finger held on the element: put down at `from` of its width, moved
/// through `through`, lifted, each step a separate touch event.
async function drag(page: import("@playwright/test").Page, target: import("@playwright/test").Locator, from: number, through: number[], check?: () => Promise<void>) {
  const box = (await target.boundingBox())!;
  const dpr = await page.evaluate(() => devicePixelRatio);
  const ui = (adb("shell", "uiautomator", "dump", "/sdcard/dnt-e2e-ui.xml"), adb("shell", "cat", "/sdcard/dnt-e2e-ui.xml"));
  const at = ui.match(/class="android\.webkit\.WebView"[^>]*bounds="\[(\d+),(\d+)\]/);
  if (!at) throw new Error("No WebView on the screen");
  const x = (f: number) => String(Math.round(Number(at[1]) + (box.x + box.width * f) * dpr));
  const y = String(Math.round(Number(at[2]) + (box.y + Math.min(box.height, 300) / 2) * dpr));
  adb("shell", "input", "motionevent", "DOWN", x(from), y);
  for (const f of through) adb("shell", "input", "motionevent", "MOVE", x(f), y);
  await check?.();
  // Held still a moment before lifting: not a flick.
  await page.waitForTimeout(300);
  adb("shell", "input", "motionevent", "UP", x(through.at(-1)!), y);
}

test("dragging shows the next tab; short of 60% it slides back, past it the tab opens", { tag: ["@sheet.swipe-tabs", "@android"] }, async ({ page }) => {
  const body = page.locator(".sheet > .body");
  const active = page.locator(".sheet .tabs button.active");
  // A third of the way: the Features tab is seen beside, then it slides back.
  await drag(page, body, 0.8, [0.65, 0.5], async () => {
    await expect(page.locator(".sheet .beside")).toContainText("Mage 1");
  });
  await expect(page.locator(".sheet .beside")).toHaveCount(0);
  await expect(active).toHaveText("Overview");
  // Most of the way: it opens.
  await drag(page, body, 0.9, [0.7, 0.5, 0.3, 0.15]);
  await expect(active).toHaveText("Features");
  await expect(page.locator(".sheet .beside")).toHaveCount(0);
});
