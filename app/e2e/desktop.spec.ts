// The app on a computer: the sheet's tab row, the party beside it, the
// overview laid out across the window, and big screens filled.
import type { Page } from "@playwright/test";
import { answer, expect, expectTab, go, openSheet, pick, sheetTab, status, test, windowSize } from "./fixtures";

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

test("ⓘ shows how a number adds up beside it, with its roll", { tag: ["@sheet.explain-popup", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Skills");
  const info = page.getByRole("button", { name: "How Arcana adds up" });
  await info.click();
  const pop = page.locator(".pop");
  await expect(pop).toBeVisible();
  await expect(pop).toContainText("Intelligence");
  // Beside what it explains, not over the middle of the window.
  const at = (await info.boundingBox())!;
  const box = (await pop.boundingBox())!;
  expect(Math.abs(box.y - at.y)).toBeLessThan(at.height * 3);
  expect(box.x > at.x + at.width - 1 || box.x + box.width < at.x + 1).toBe(true);
  // Where each part comes from.
  await expect(pop.locator(".part").first()).not.toBeEmpty();
  await pop.getByRole("button", { name: "Advantage", exact: true }).click();
  await expect(pop).toBeHidden();
  await expect(status(page)).toContainText(/Arcana/);
  // The ability's name, too; Escape closes.
  await page.getByRole("button", { name: "Strength", exact: true }).click();
  await expect(pop).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();
});

test("the sheet's ⋯ menu is a row of buttons too", { tag: ["@sheet.actions-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const row = page.getByRole("toolbar", { name: "Character" });
  for (const name of ["Level up…", "Short rest", "Long rest", "Remove the latest level", "Open the character's note", "Edit the character file…", "Export as a printable page…", "Export as Markdown…", "Export for dnt (to import elsewhere)…", "Delete the character…"]) {
    await expect(row.getByRole("button", { name, exact: true })).toBeVisible();
  }
  await row.getByRole("button", { name: "Long rest", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("long rest");
  await page.keyboard.press("Escape");
  await row.getByRole("button", { name: "Write rolls into the session note" }).click();
  await expect(row.getByRole("button", { name: "Stop writing rolls into the session note" })).toBeVisible();
});

const staff = (page: Page) => page.locator(".attack").filter({ has: page.locator(".aname", { hasText: /^\s*Quarterstaff\s*$/ }) });

test("an attack's menu is buttons by it: the hands, how it adds up", { tag: ["@attacks.actions-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const acts = page.getByRole("group", { name: "Quarterstaff: actions" });
  await acts.getByRole("button", { name: "Hold in the main hand" }).click();
  await expect(staff(page).locator(".hand")).not.toBeEmpty();
  await acts.getByRole("button", { name: "How the damage is made up" }).click();
  await expect(page.locator(".pop")).toBeVisible();
  await page.keyboard.press("Escape");
  // No empty gutter before the names.
  const name = (await page.locator(".attack").filter({ hasText: "Dagger" }).locator(".aname").boundingBox())!;
  const row = (await page.locator(".attack").filter({ hasText: "Dagger" }).locator(".arow").boundingBox())!;
  expect(name.x - row.x).toBeLessThan(20);
});

test("trackers and the hit dice have their actions in sight", { tag: ["@trackers.actions-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const line = page.locator(".hpline");
  await expect(line).toContainText("Temp HP 0");
  const die = line.locator(".tracker.hitdie");
  await expect(die.locator(".count")).toHaveText("1/1");
  await die.getByRole("button", { name: "Use" }).click();
  await expect(die.locator(".count")).toHaveText("0/1");
  await die.getByRole("button", { name: "Restore" }).click();
  await expect(die.locator(".count")).toHaveText("1/1");
  await expect(die.getByRole("button", { name: /Roll d6 \+ Con, heal/ })).toHaveText("Roll one and heal");
  await die.getByRole("button", { name: "How the hit dice are made up" }).click();
  await expect(page.locator(".pop")).toBeVisible();
  await page.keyboard.press("Escape");
  // A feature's tracker: the feature it comes from, in one click.
  const acts = page.getByRole("group", { name: "Arcane Recovery: actions" });
  await acts.getByRole("button", { name: "Show the feature" }).click();
  await expect(page.getByRole("dialog", { name: /Arcane Recovery/ })).toBeVisible();
});

test("a click on a spell shows its actions under it", { tag: ["@spells.actions-panel", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Spells");
  await page.getByRole("button", { name: "+ add" }).first().click();
  await pick(page, ["Fire Bolt"]);
  const spell = page.locator("button.spell", { hasText: "Fire Bolt" });
  await spell.click();
  const acts = page.locator(".acts").filter({ has: page.getByRole("button", { name: "Roll it" }) });
  await expect(acts).toBeVisible();
  await expect(acts.getByRole("button", { name: "Read it" })).toBeVisible();
  await acts.getByRole("button", { name: "Make it a favorite" }).click();
  await expect(acts.getByRole("button", { name: "No longer a favorite" })).toBeVisible();
  // The menu is still there: ⋯.
  await page.locator(".srow", { has: spell }).first().getByRole("button", { name: "More" }).click();
  await expect(page.locator(".modal, [role=dialog]").filter({ hasText: "Roll it" }).last()).toBeVisible();
});

test("a click on an item shows its actions under it", { tag: ["@inventory.actions-panel", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Inventory");
  await page.locator("button.line", { hasText: "Dagger" }).click();
  const acts = page.locator(".acts").filter({ has: page.getByRole("button", { name: "One more" }) });
  await expect(acts).toBeVisible();
  for (const name of ["Wear it in…", "Hold in the main hand", "Put it in…", "Remove"]) await expect(acts.getByRole("button", { name, exact: true })).toBeVisible();
  await acts.getByRole("button", { name: "One more" }).click();
  await expect(page.locator("button.line", { hasText: "Dagger" }).locator(".qty")).toHaveText("×3");
});

test("the box by an item says Equip, Held or Worn", { tag: ["@inventory.equip-pills", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Inventory");
  const quarterstaff = page.getByRole("checkbox", { name: "Equipped: Quarterstaff", exact: true });
  await expect(quarterstaff).toHaveText("Equip");
  await quarterstaff.click();
  await expect(quarterstaff).toHaveText("Held · main hand");
  await quarterstaff.click();
  await expect(quarterstaff).toHaveText("Equip");
  const robe = page.getByRole("checkbox", { name: "Equipped: Robe", exact: true });
  await robe.click();
  await expect(robe).toHaveText(/^Worn · /);
});
