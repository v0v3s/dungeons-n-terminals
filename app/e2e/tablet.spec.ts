// A tablet held sideways: the phone's screen made a tablet's (its size and
// density, as adb sets them) for these tests, and given back after.
import type { Locator } from "@playwright/test";
import { adb } from "./android";
import { answer, expect, go, openSheet, sheetTab, test } from "./fixtures";

test.describe("a tablet sideways", () => {
  test.beforeEach(async ({ page }) => {
    test.setTimeout(120_000);
    adb("shell", "wm", "size", "1920x1200");
    adb("shell", "wm", "density", "280");
    await expect.poll(() => page.evaluate(() => innerWidth), { timeout: 15_000 }).toBeGreaterThan(1000);
  });
  test.afterEach(async ({ page }) => {
    adb("shell", "wm", "size", "reset");
    adb("shell", "wm", "density", "reset");
    await expect.poll(() => page.evaluate(() => innerWidth), { timeout: 15_000 }).toBeLessThan(760);
  });

  /// How far short of its container's right edge an element ends (px).
  const short = (inner: Locator, outer: Locator) =>
    Promise.all([inner.boundingBox(), outer.boundingBox()]).then(([i, o]) => o!.x + o!.width - (i!.x + i!.width));

  test("everything spans the screen, the sheet as on a phone", { tag: ["@layout.tablet", "@android"] }, async ({ page }) => {
    const main = page.locator("main");
    // The sheet: its tabs as icons, the name in the app bar only.
    await openSheet(page, "Skills");
    await expect(page.locator(".sheet .icontabs")).toBeVisible();
    await expect(page.locator(".sheet .head")).toBeHidden();
    // Its two columns share the width.
    const panes = page.locator(".skills-tab > .pane");
    const [a, b] = [(await panes.nth(0).boundingBox())!, (await panes.nth(1).boundingBox())!];
    expect(b.x).toBeGreaterThan(a.x + a.width);
    expect(Math.abs(a.width - b.width)).toBeLessThan(a.width * 0.05);
    expect(await short(panes.nth(1), main)).toBeLessThan(40);
    await sheetTab(page, "Overview");

    // The characters, and a wider + New character.
    await go(page, "Characters");
    // From the open sheet back to the list.
    const back = page.getByRole("button", { name: "Back", exact: true });
    if (await back.isVisible()) await back.click();
    const card = page.locator("button.card").first();
    await expect(card).toBeVisible();
    expect(await short(card, main)).toBeLessThan(40);
    const add = page.getByRole("button", { name: "+ New character" });
    const label = await add.evaluate((b) => {
      const r = document.createRange();
      r.selectNodeContents(b);
      return r.getBoundingClientRect().width;
    });
    expect((await add.boundingBox())!.width).toBeGreaterThan(label * 1.6);

    // Creation: the scores' Next beside Back and Cancel.
    await add.click();
    await answer(page, "New character", "Nova");
    const ask = page.locator(".creation .ask");
    await ask.locator("li button", { hasText: "Human" }).first().click();
    await ask.locator("li button", { hasText: "Sage" }).first().click();
    const nav = page.locator(".creation .nav");
    await expect(nav.getByRole("button", { name: "Next" })).toBeVisible();
    await expect(nav.getByRole("button", { name: "Cancel" })).toBeVisible();
    await nav.getByRole("button", { name: "Cancel" }).click();

    // Campaigns and Data.
    await go(page, "Campaigns");
    expect(await short(page.locator(".campaigns .inner"), main)).toBeLessThan(40);
    await go(page, "Data", "Import");
    expect(await short(page.locator(".data .inner > .pane").first(), main)).toBeLessThan(40);
  });

  test("the compendium's results start below the sort, apart from it", { tag: ["@layout.tablet", "@android"] }, async ({ page }) => {
    await go(page, "Compendium");
    const sort = (await page.locator(".order select").boundingBox())!;
    const first = (await page.locator(".rows li").first().boundingBox())!;
    expect(first.y - (sort.y + sort.height)).toBeGreaterThan(4);
  });
});
