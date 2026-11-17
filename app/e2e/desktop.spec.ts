// The app on a computer: the sheet's tab row, the party beside it, the
// overview laid out across the window, and big screens filled.
import type { Page } from "@playwright/test";
import { answer, expect, expectTab, fits, go, openSheet, pick, sheetMore, sheetTab, status, test, windowSize } from "./fixtures";

const party = (page: Page) => page.getByRole("navigation", { name: "Party" });

test("the sheet's tabs are icons, the open one named; a tab's name opens out on hover or focus", { tag: ["@sheet.tab-row", "@windows"] }, async ({ page }) => {
  for (const size of [
    { width: 1280, height: 820 },
    { width: 2560, height: 1440 },
  ]) {
    await windowSize(page, size);
    await openSheet(page);
    const row = page.locator(".sheet .tabrow");
    const tabs = row.getByRole("tab");
    await expect(tabs).toHaveCount(6);
    await expect(tabs.locator("svg.icon").first()).toBeVisible();
    // Left-aligned, not stretched across the sheet.
    const sheet = (await page.locator(".sheet > .head").boundingBox())!;
    const box = (await row.boundingBox())!;
    expect(box.width, `at ${size.width}px`).toBeLessThan(sheet.width * 0.6);
    expect(Math.abs(box.x - sheet.x), `at ${size.width}px`).toBeLessThan(4);
    // The open tab shows its name; the others only their icon.
    await expect(row.getByRole("tab", { selected: true })).toHaveAccessibleName("Overview");
    await expect(row.getByRole("tab", { selected: true }).locator(".tname")).toBeVisible();
    const skills = row.getByRole("tab", { name: "Skills" });
    const name = skills.locator(".flyout");
    const shown = () => name.evaluate((n) => Number(getComputedStyle(n).opacity));
    expect(await shown()).toBe(0);
    // The pointer over it: its name opens out, centred on its icon; no tab moves.
    const before = await tabs.evaluateAll((ts) => ts.map((t) => Math.round(t.getBoundingClientRect().left)));
    await skills.hover();
    await expect.poll(shown).toBe(1);
    await expect(name).toContainText("Skills");
    const tab = (await skills.boundingBox())!;
    const out = (await name.boundingBox())!;
    expect(Math.abs(out.x + out.width / 2 - (tab.x + tab.width / 2))).toBeLessThan(3);
    expect(await tabs.evaluateAll((ts) => ts.map((t) => Math.round(t.getBoundingClientRect().left)))).toEqual(before);
    // The keyboard on a tab does the same.
    await page.mouse.move(1, 1);
    await expect.poll(shown).toBe(0);
    await skills.focus();
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Tab");
    await expect.poll(shown).toBe(1);
    // Tester, a Wizard at level 1, has spells to choose: the count on the icon.
    await expect(row.getByRole("tab", { name: /^Spells/ })).toHaveAccessibleName(/^Spells, \d+ to choose$/);
    await expect(row.getByRole("tab", { name: /^Spells/ }).locator(".badge")).toBeVisible();
    await sheetTab(page, "Skills");
    await expectTab(page, "Skills");
    await expect(row.getByRole("tab", { selected: true }).locator(".tname")).toHaveText("Skills");
  }
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
    // Nothing runs out of its box on a big screen either.
    for (const tab of ["Overview", "Skills", "Spells", "Inventory", "Features"] as const) {
      await sheetTab(page, tab);
      await fits(page, `sheet: ${tab} at ${size.width}px`);
    }
    for (const section of ["Compendium", "Characters", "Notes", "Campaigns", "Data"] as const) {
      await go(page, section);
      await fits(page, `${section} at ${size.width}px`);
    }
  }
});

test("ⓘ shows how a number adds up beside it, with its roll", { tag: ["@sheet.explain-popup", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Skills");
  const info = page.getByRole("button", { name: "How Arcana adds up" });
  await info.click();
  const pop = page.locator(".pop");
  await expect(pop).toBeVisible();
  // The total with its sign; a line for each part, with where it comes from:
  // Tester's Arcana is Intelligence +1 and a proficiency bonus +2.
  await expect(pop.locator(".pvalue")).toHaveText("+3");
  await expect(pop.locator(".pline")).toHaveCount(2);
  await expect(pop.locator(".pline").nth(0)).toContainText(/\+1\s*Intelligence modifier/);
  await expect(pop.locator(".pline").nth(1)).toContainText(/\+2\s*Proficiency bonus/);
  // Beside what it explains, not over the middle of the window.
  const at = (await info.boundingBox())!;
  const box = (await pop.boundingBox())!;
  expect(Math.abs(box.y - at.y)).toBeLessThan(at.height * 3);
  expect(box.x > at.x + at.width - 1 || box.x + box.width < at.x + 1).toBe(true);
  // Only how it adds up: rolling is the number's own (and the Roll control's).
  await expect(pop.getByRole("button", { name: /Roll|Advantage|Disadvantage/ })).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();
  // The ability's name, too; Escape closes.
  await page.getByRole("button", { name: "Strength", exact: true }).click();
  await expect(pop).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(pop).toBeHidden();
});

test("the next roll is chosen in sight, and Shift or Ctrl with a click rolls with advantage or disadvantage", { tag: ["@roll.mode-control", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Skills");
  const modes = page.getByRole("group", { name: "Next roll" });
  const pressed = modes.locator('[aria-pressed="true"]');
  await expect(pressed).toHaveText("Normal");
  await expect(page.locator(".rollmode .hint")).toContainText("Shift+click advantage");
  await expect(page.locator(".rollmode .hint")).toContainText("Ctrl+click disadvantage");
  const arcana = page.locator("button.skill", { hasText: "Arcana" });
  /// Roll, read the roll, and put it away (the next one is its own).
  const rolled = async (click: () => Promise<void>) => {
    await click();
    await expect(page.locator(".status.roll")).toBeVisible();
    const text = await page.locator(".status.roll").innerText();
    await page.locator(".status.roll").click();
    await expect(page.locator(".status")).toHaveCount(0);
    return text;
  };
  // A plain click rolls as the control says; then it is Normal again.
  await modes.getByRole("button", { name: "Advantage", exact: true }).click();
  await expect(pressed).toHaveText("Advantage");
  expect(await rolled(() => arcana.click())).toMatch(/(?<!dis)advantage \(chosen\)/);
  await expect(pressed).toHaveText("Normal");
  // Shift+click: advantage; Ctrl+click: disadvantage; the control stays Normal.
  expect(await rolled(() => arcana.click({ modifiers: ["Shift"] }))).toMatch(/Arcana[\s\S]*(?<!dis)advantage \(chosen\)/);
  await expect(pressed).toHaveText("Normal");
  expect(await rolled(() => arcana.click({ modifiers: ["Control"] }))).toMatch(/Arcana[\s\S]*disadvantage \(chosen\)/);
  await expect(pressed).toHaveText("Normal");
  expect(await rolled(() => arcana.click())).not.toContain("chosen");
  // The same control on the overview, instead of the phone's chip.
  await sheetTab(page, "Overview");
  await expect(page.getByRole("group", { name: "Next roll" })).toBeVisible();
  await expect(page.locator(".state button.mode")).toHaveCount(0);
  await page.getByRole("group", { name: "Next roll" }).getByRole("button", { name: "Disadvantage" }).click();
  await page.locator("button.tile", { hasText: "Initiative" }).click();
  await expect(status(page)).toContainText("disadvantage (chosen)");
});

test("the latest rolls fold under the numbers, never a column of their own", { tag: ["@sheet.rolls-folded", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 2560, height: 1440 });
  await openSheet(page);
  await page.locator("button.tile", { hasText: "Initiative" }).click();
  await page.locator("button.tile", { hasText: "Initiative" }).click();
  const rolls = page.locator("details.rolled");
  await expect(rolls.locator("summary")).toContainText("Rolls (2)");
  // Folded at first, under the numbers, as wide as they are.
  await expect(rolls).not.toHaveAttribute("open");
  const tiles = (await page.locator(".tiles").boundingBox())!;
  const box = (await rolls.boundingBox())!;
  expect(box.y).toBeGreaterThan(tiles.y + tiles.height - 1);
  expect(Math.abs(box.x - tiles.x)).toBeLessThan(4);
  expect(box.width).toBeGreaterThan(tiles.width * 0.95);
  await rolls.locator("summary").click();
  await expect(rolls.locator(".roll")).toHaveCount(2);
});

test("the sheet's ⋯ menu is one toolbar of groups too, the exports in ⋯ only", { tag: ["@sheet.actions-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const bar = page.getByRole("toolbar", { name: "Character actions" });
  const group = (name: string) => bar.getByRole("group", { name, exact: true });
  const groups: [string, string[]][] = [
    ["Play", ["Short rest", "Long rest", "Rolls to the session note"]],
    ["Character", ["Level up…", "Undo the last level", "Note", "File…"]],
  ];
  for (const [g, names] of groups) for (const name of names) await expect(group(g).getByRole("button", { name, exact: true })).toBeVisible();
  await expect(bar.getByRole("button", { name: "Delete the character…" })).toBeVisible();
  // Each button's full name on hover.
  await expect(group("Character").getByRole("button", { name: "Note", exact: true })).toHaveAttribute("title", "Open the character's note");
  // The exports are in the ⋯ menu, not the toolbar: all of it one line.
  await expect(group("Export")).toHaveCount(0);
  await expect(bar.getByRole("button", { name: /Markdown|Printable|For dnt/ })).toHaveCount(0);
  const lines = (where: typeof bar) => where.locator("button").evaluateAll((bs) => new Set(bs.map((b) => Math.round(b.getBoundingClientRect().top))).size);
  await expect.poll(() => lines(bar)).toBe(1);
  await sheetMore(page);
  const more = page.getByRole("dialog", { name: "Tester" });
  for (const name of ["Export as a printable page…", "Export as Markdown…", "Export for dnt (to import elsewhere)…"]) await expect(more.getByRole("button", { name, exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await windowSize(page, { width: 2560, height: 1440 });
  await expect.poll(() => lines(bar)).toBe(1);
  await windowSize(page, { width: 1280, height: 820 });
  await group("Play").getByRole("button", { name: "Long rest", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Long rest" })).toContainText("long rest");
  await page.keyboard.press("Escape");
  await group("Play").getByRole("button", { name: "Rolls to the session note" }).click();
  await expect(group("Play").getByRole("button", { name: "Stop rolls to the session note" })).toBeVisible();
});

test("what happened shows in a toast that fades, never over the sheet", { tag: ["@sheet.toast", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  await page.locator(".pane.hp").getByRole("button", { name: "Temp", exact: true }).click();
  await answer(page, "Temporary hit points", "5");
  const toast = page.getByRole("status").filter({ hasText: "Temporary hit points" });
  await expect(toast).toBeVisible();
  // In a corner, out of the page's flow: the sheet keeps room to scroll its
  // last row clear of it.
  expect(await toast.evaluate((t) => getComputedStyle(t).position)).toBe("fixed");
  const body = page.locator(".sheet > .body");
  expect(await body.evaluate((b) => parseFloat(getComputedStyle(b).paddingBottom))).toBeGreaterThan((await toast.boundingBox())!.height);
  await expect(page.locator(".status")).toHaveCount(0, { timeout: 8000 });
});

const staff = (page: Page) => page.locator(".attack").filter({ has: page.locator(".aname", { hasText: /^\s*Quarterstaff\s*$/ }) });

test("an attack's menu is buttons by it: the hands, how it adds up", { tag: ["@attacks.actions-row", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page);
  const acts = page.getByRole("group", { name: "Quarterstaff: actions" });
  // Both hands for a Versatile weapon, not for a Dagger.
  await expect(acts.getByRole("button", { name: "Hold with both hands" })).toBeVisible();
  const dagger = page.getByRole("group", { name: "Dagger: actions" }).first();
  await expect(dagger.getByRole("button", { name: "Hold in the off hand" })).toBeVisible();
  await expect(dagger.getByRole("button", { name: "Hold with both hands" })).toHaveCount(0);
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
  await acts.getByRole("button", { name: "The feature" }).click();
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
  await expect(page.locator(".lrow").filter({ has: page.locator("button.line", { hasText: "Dagger" }) }).locator(".qty")).toHaveText("×3");
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

test("the features flow in columns of their own, and the swap buttons keep apart", { tag: ["@features.desktop-columns", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 1280, height: 820 });
  await openSheet(page, "Overview", "Ranger");
  await sheetTab(page, "Features");
  const panes = page.locator(".features > .pane");
  const boxes = await panes.evaluateAll((ps) => ps.map((p) => p.getBoundingClientRect()).map((r) => ({ x: Math.round(r.left), top: r.top, bottom: r.bottom })));
  // Two columns or more; in each, a pane follows the one above it closely,
  // whatever stands beside it.
  const columns = new Map<number, { top: number; bottom: number }[]>();
  for (const b of boxes) columns.set(b.x, [...(columns.get(b.x) ?? []), b]);
  expect(columns.size).toBeGreaterThanOrEqual(2);
  const rem = await page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize));
  for (const col of columns.values()) {
    const sorted = [...col].sort((a, b) => a.top - b.top);
    for (let i = 1; i < sorted.length; i++) expect(sorted[i].top - sorted[i - 1].bottom).toBeLessThan(2 * rem);
  }
  // The Weapon Mastery swaps, one per row, apart from each other.
  const swaps = await page.getByRole("button", { name: "swap", exact: true }).evaluateAll((bs) => bs.map((b) => b.getBoundingClientRect()).map((r) => ({ top: r.top, bottom: r.bottom })));
  expect(swaps.length).toBeGreaterThanOrEqual(2);
  for (let i = 1; i < swaps.length; i++) expect(swaps[i].top - swaps[i - 1].bottom).toBeGreaterThanOrEqual(2);
});
