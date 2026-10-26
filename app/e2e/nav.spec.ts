import type { Page } from "@playwright/test";
import { CAMPAIGN, expect, go, openSheet, sheetTab, test } from "./fixtures";

test("the sections are reached from the menu or the header", { tag: "@nav.sections" }, async ({ page }) => {
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
  await go(page, "Notes");
  await expect(page.getByPlaceholder("search notes")).toBeVisible();
  await expect(page.getByText(`Notes · ${CAMPAIGN}`)).toBeVisible();
  await go(page, "Campaigns");
  await expect(page.locator("section.card", { hasText: CAMPAIGN })).toBeVisible();
  await go(page, "Data");
  await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
  await go(page, "Compendium");
  await expect(page.locator(".compendium .prompt input")).toBeVisible();
  await expect(page.locator(".compendium ul.rows button").first()).toBeVisible();
  // Back to where it was.
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
});

/// What does not fit on the screen: the page wider than the window,
/// buttons off its side (but for those in a row that scrolls sideways, like
/// the tabs), and buttons whose label wraps onto a second line.
function misfits(page: Page) {
  return page.evaluate(() => {
    const out: string[] = [];
    const width = document.documentElement.clientWidth;
    const wide = Math.max(document.documentElement.scrollWidth, document.body.scrollWidth);
    if (wide > width) out.push(`the page is ${wide}px wide in a ${width}px window`);
    for (const button of document.querySelectorAll("button")) {
      const box = button.getBoundingClientRect();
      if (!box.width || !box.height || getComputedStyle(button).visibility === "hidden") continue;
      const scrolls = (e: Element | null): boolean =>
        !!e && (["auto", "scroll"].includes(getComputedStyle(e).overflowX) || scrolls(e.parentElement));
      if (box.right > width + 1 && !scrolls(button.parentElement)) out.push(`button "${button.textContent?.trim()}" runs off the side (right ${Math.round(box.right)}px)`);
      // A label that wrapped has its characters on more than one line
      // (a button of several texts, a card or a row, may wrap them).
      const texts = [...button.childNodes].length && document.createTreeWalker(button, NodeFilter.SHOW_TEXT);
      let labels = 0;
      for (let n = texts && texts.nextNode(); n; n = texts && texts.nextNode()) if (n.textContent?.trim()) labels++;
      if (labels > 1) continue;
      const walker = document.createTreeWalker(button, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (!node.textContent?.trim()) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        const tops = new Set<number>();
        for (const r of range.getClientRects()) if (r.width > 0) tops.add(Math.round(r.top / 4));
        if (tops.size > 1) {
          out.push(`button "${button.textContent?.trim().replace(/\s+/g, " ")}" wraps its label`);
          break;
        }
      }
    }
    return out;
  });
}

/// Check the screen as it is now, saying where.
async function fits(page: Page, where: string) {
  // Let transitions and lazy content settle.
  await page.waitForTimeout(400);
  expect.soft(await misfits(page), where).toEqual([]);
}

test("every screen fits its window", { tag: "@layout.fits" }, async ({ page }) => {
  for (const section of ["Compendium", "Characters", "Notes", "Campaigns", "Data"] as const) {
    await go(page, section);
    await fits(page, section);
  }
  // An entry open in the compendium.
  await go(page, "Compendium");
  await page.locator(".compendium ul.rows button").first().click();
  await fits(page, "Compendium entry");

  await openSheet(page, "Overview");
  for (const tab of ["Overview", "Features", "Spells", "Inventory", "Notes"] as const) {
    await sheetTab(page, tab);
    await fits(page, `sheet: ${tab}`);
  }
});

test("the menu reaches inside a section: a category, the open character's tab", { tag: ["@nav.menu-inside", "@android"] }, async ({ page }) => {
  const menu = () => page.getByRole("button", { name: "Menu", exact: true }).click();
  const inside = (section: string) => page.locator(`.drawer .child[data-in="${section}"]`);
  await openSheet(page);
  await go(page, "Compendium");
  // Folded at first: a section's ▸ shows what is in it.
  await menu();
  await expect(inside("characters")).toHaveCount(0);
  await page.getByRole("button", { name: "What is in Characters" }).click();
  await page.getByRole("button", { name: "What is in Compendium" }).click();
  // From the compendium straight to the character's spells.
  await inside("characters").filter({ has: page.getByText("Spells", { exact: true }) }).click();
  await expect(page.locator(".sheet .tabs select")).toHaveValue("Spells");
  // And to a category of the compendium: its results, not an open entry.
  await menu();
  await inside("compendium").nth(2).click();
  await expect(page.locator(".categories select")).toHaveValue("2");
  // The character's note, from the menu too.
  await menu();
  await inside("characters").filter({ has: page.getByText("Notes", { exact: true }) }).click();
  await expect(page.locator(".note")).toContainText("Tester.md");
});
