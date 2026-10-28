import type { Page } from "@playwright/test";
import { expect, expectTab, go, openSheet, sheetMore, test } from "./fixtures";
import { swipe } from "./touch";

const tip = (page: Page, id: string) => page.locator(`[data-tip="${id}"]`);

/// A frontend that has shown no tips yet.
async function fresh(page: Page) {
  await page.evaluate(() => localStorage.removeItem("dnt.tips"));
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
}

test("on a phone, a tip names each gesture once, until the menu brings them back", { tag: ["@tips.gestures", "@android"] }, async ({ page }) => {
  await fresh(page);
  await openSheet(page);
  await expect(tip(page, "sheet.swipe")).toContainText("swipe sideways for the next tab");
  // Using the gesture is reading the tip: the next one shows.
  await swipe(page, page.locator(".sheet > .body"), 0.8, 0.2);
  await expectTab(page, "Skills");
  await expect(tip(page, "sheet.swipe")).toHaveCount(0);
  await expect(tip(page, "sheet.hold")).toContainText("hold a number to see how it adds up");
  await tip(page, "sheet.hold").getByRole("button", { name: "Dismiss the tip" }).click();
  await expect(tip(page, "sheet.hold")).toHaveCount(0);

  // The compendium has its own.
  await go(page, "Compendium");
  await expect(tip(page, "compendium.swipe")).toContainText("swipe sideways for the next category");
  await tip(page, "compendium.swipe").getByRole("button", { name: "Dismiss the tip" }).click();
  await expect(tip(page, "compendium.swipe")).toHaveCount(0);

  // Gone for good on this device…
  await openSheet(page);
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await openSheet(page);
  await expect(page.locator(".sheet [data-tip]")).toHaveCount(0);
  // …until the sheet's menu shows them again.
  await sheetMore(page);
  await page.getByRole("button", { name: "Show the tips again" }).click();
  await expect(tip(page, "sheet.swipe")).toBeVisible();
});

test("on a phone, the search and the category share a row, the sort in the results' heading", { tag: ["@compendium.one-row", "@android"] }, async ({ page }) => {
  await go(page, "Compendium");
  const search = page.locator(".compendium .searchrow .searchbox input");
  const category = page.locator(".compendium .searchrow .categories select");
  await expect(search).toBeVisible();
  await expect(category).toBeVisible();
  const [a, b] = [await search.boundingBox(), await category.boundingBox()];
  expect(Math.abs(a!.y + a!.height / 2 - (b!.y + b!.height / 2))).toBeLessThan(8);
  // The sort sits on the results' border, above the first row.
  const sort = page.locator(".browse > .aside .order select");
  await expect(sort).toBeVisible();
  const first = page.locator(".browse .rows li").first();
  expect((await sort.boundingBox())!.y).toBeLessThan((await first.boundingBox())!.y);
  // It still sorts (and is kept: put it back).
  try {
    await sort.selectOption({ label: "name A–Z" });
    await expect(sort).not.toHaveValue("0");
  } finally {
    await sort.selectOption({ label: "best match" });
  }
});
