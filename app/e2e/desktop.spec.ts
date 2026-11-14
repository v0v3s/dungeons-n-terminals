// The app on a computer: the sheet's tab row, the party beside it, the
// overview laid out across the window, and big screens filled.
import type { Page } from "@playwright/test";
import { answer, expect, expectTab, go, openSheet, sheetTab, test, windowSize } from "./fixtures";

const party = (page: Page) => page.getByRole("navigation", { name: "Party" });

test("the sheet's tabs are a row of names; Spells shows the spells to choose", { tag: ["@sheet.tab-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const row = page.locator(".sheet .tabrow");
  await expect(row.getByRole("tab")).toHaveCount(6);
  await expect(row.getByRole("tab", { selected: true })).toHaveText("Overview");
  // Tester, a Wizard at level 1, has spells to choose.
  await expect(row.getByRole("tab", { name: /^Spells/ })).toHaveText(/^Spells\s*\d+$/);
  await sheetTab(page, "Skills");
  await expectTab(page, "Skills");
  await expect(row.getByRole("tab", { selected: true })).toHaveText("Skills");
});

test("the party is beside the sheet: another character in one click, ☰ hides it", { tag: ["@sheet.party-sidebar", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  await expect(party(page).locator('[aria-current="true"]')).toContainText("Tester");
  await party(page).getByRole("button", { name: /^Brawler/ }).click();
  await expect(page.locator(".sheet h1")).toContainText("Brawler");
  await expect(party(page).locator('[aria-current="true"]')).toContainText("Brawler");
  await page.getByRole("button", { name: "Hide the sidebar" }).click();
  await expect(party(page)).toBeHidden();
  await page.getByRole("button", { name: "Show the sidebar" }).click();
  await expect(party(page)).toBeVisible();
});

test("the hit points run across the overview; damage keeps to one line", { tag: ["@sheet.overview-desktop", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const pane = page.locator(".pane.hp");
  await pane.getByRole("button", { name: "Temp", exact: true }).click();
  await answer(page, "Temporary hit points", "5");
  await expect(page.locator(".hpnum")).toHaveText(/8\/8\s*\+5 temp/);
  // The temporary hit points: a share of the bar after the hit points.
  const temp = await pane.locator(".tempfill").boundingBox();
  const fill = await pane.locator(".hpbar .fill").boundingBox();
  expect(temp!.width).toBeGreaterThan(0);
  expect(temp!.x).toBeGreaterThan(fill!.x + fill!.width - 1);
  // The number, the bar and the buttons side by side, on one band.
  const num = (await page.locator(".hpnum").boundingBox())!;
  const damage = (await pane.getByRole("button", { name: "Damage", exact: true }).boundingBox())!;
  expect(fill!.x).toBeGreaterThan(num.x + num.width);
  expect(damage.x).toBeGreaterThan(fill!.x + fill!.width);
  expect(Math.abs(damage.y + damage.height / 2 - (num.y + num.height / 2))).toBeLessThan(num.height);
  // The Quarterstaff (versatile): its damage on one line, the two-handed
  // damage on a line under it.
  const staff = page.locator(".attack").filter({ has: page.locator(".aname", { hasText: /^\s*Quarterstaff\s*$/ }) });
  const lines = await staff.locator(".dmg").evaluate((d) => {
    const main = d.firstChild!;
    const range = document.createRange();
    range.selectNodeContents(main);
    const tops = new Set([...range.getClientRects()].map((r) => Math.round(r.top)));
    const alt = d.querySelector(".alt")!.getBoundingClientRect();
    return { mainLines: tops.size, altBelow: alt.top >= range.getBoundingClientRect().bottom - 1, label: getComputedStyle(d.querySelector(".alt")!, "::before").content };
  });
  expect(lines).toEqual({ mainLines: 1, altBelow: true, label: '"Two-handed: "' });
});

/// How the window is used: its width, the root font's size, how far right
/// the shown content reaches, and the widest paragraph in characters.
function usage(page: Page) {
  return page.evaluate(() => {
    const width = document.documentElement.clientWidth;
    let right = 0;
    for (const el of document.querySelectorAll("main *")) {
      const box = el.getBoundingClientRect();
      if (box.width && box.height && getComputedStyle(el).visibility !== "hidden") right = Math.max(right, box.right);
    }
    let widest = 0;
    for (const p of document.querySelectorAll<HTMLElement>("main p")) {
      const box = p.getBoundingClientRect();
      if (!box.width) continue;
      const zero = document.createElement("span");
      zero.textContent = "0";
      zero.style.cssText = "position:absolute;visibility:hidden";
      p.appendChild(zero);
      widest = Math.max(widest, box.width / zero.getBoundingClientRect().width);
      zero.remove();
    }
    return { width, font: parseFloat(getComputedStyle(document.documentElement).fontSize), right, widest };
  });
}

test("a big screen is filled: wider, bigger, more columns, readable lines", { tag: ["@layout.wide", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await go(page, "Compendium");
  const small = await usage(page);
  for (const size of [
    { width: 2560, height: 1440 },
    { width: 3840, height: 2160 },
  ]) {
    await windowSize(page, size);
    for (const section of ["Compendium", "Characters", "Notes", "Campaigns", "Data"] as const) {
      await go(page, section);
      await page.waitForTimeout(200);
      const u = await usage(page);
      expect.soft(u.right, `${section} at ${size.width}px reaches across`).toBeGreaterThan(u.width * 0.9);
      expect.soft(u.font, `${section} at ${size.width}px: bigger text`).toBeGreaterThan(small.font);
    }
    // An entry open: its text wraps at a readable length.
    await go(page, "Compendium");
    await page.locator(".compendium .prompt input").fill("fireball");
    await page.locator(".compendium ul.rows button").first().click();
    await page.waitForTimeout(200);
    expect.soft((await usage(page)).widest, `entry text at ${size.width}px`).toBeLessThanOrEqual(90);
    // The sheet spans the window, the skills in more than two columns.
    await openSheet(page, "Skills");
    const u = await usage(page);
    expect.soft(u.right, `sheet at ${size.width}px reaches across`).toBeGreaterThan(u.width * 0.9);
    const columns = await page.locator(".skills-tab .skills .body").evaluate((b) => new Set([...b.querySelectorAll("button.skill")].map((s) => Math.round(s.getBoundingClientRect().left))).size);
    expect.soft(columns, `skill columns at ${size.width}px`).toBeGreaterThanOrEqual(2);
  }
});
