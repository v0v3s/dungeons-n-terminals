import { answer, CAMPAIGN, expect, expectTab, fits, go, invoke, openSheet, sheetTab, test, windowSize } from "./fixtures";

test("the sections are reached from the menu or the header", { tag: "@nav.sections" }, async ({ page }) => {
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
  await go(page, "Notes");
  await expect(page.getByPlaceholder("search notes")).toBeVisible();
  await expect(page.getByText(`Notes · ${CAMPAIGN}`)).toBeVisible();
  await go(page, "Campaigns");
  await expect(page.locator("section.card", { hasText: CAMPAIGN })).toBeVisible();
  await go(page, "Data");
  await expect(page.getByRole("button", { name: "Import…" })).toBeVisible();
  await go(page, "Compendium");
  await expect(page.locator(".compendium .prompt input")).toBeVisible();
  await expect(page.locator(".compendium ul.rows button").first()).toBeVisible();
  // Back to where it was.
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
});

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
  for (const tab of ["Overview", "Skills", "Spells", "Inventory", "Features", "Notes"] as const) {
    await sheetTab(page, tab);
    await fits(page, `sheet: ${tab}`);
  }
});

test("every step of a new character fits its window", { tag: "@layout.fits" }, async ({ page }) => {
  test.setTimeout(120_000);
  await go(page, "Characters");
  await page.getByRole("button", { name: "+ New character" }).click();
  await answer(page, "New character", "Fitter");
  const ask = page.locator(".creation .ask");
  await expect(ask.locator(".title")).toContainText("Choose a species");
  // A Dragonborn Sage Ranger (long names, long lists), else the first choices.
  const prefer = ["Dragonborn", "Sage", "Ranger"];
  for (let i = 0; i < 30 && !(await ask.getByText("Everything is chosen").isVisible()); i++) {
    const where = (await ask.locator(".title").first().textContent().catch(() => null)) ?? "the scores";
    await fits(page, `creation: ${where}`);
    const nav = page.locator(".creation .nav");
    if (await nav.getByRole("button", { name: "Next" }).isVisible()) {
      await nav.getByRole("button", { name: "Next" }).click();
      continue;
    }
    const rows = ask.locator("li button:not(.info)");
    const many = ask.locator(".count");
    const count = (await many.count()) ? Number(((await many.textContent()) ?? "").match(/of (\d+)/)?.[1] ?? 1) : 1;
    const labels = await rows.locator(".label").allTextContents();
    const wanted = labels.findIndex((l) => prefer.includes(l.trim()));
    if (count > 1) for (let n = 0; n < count; n++) await rows.nth(n).click();
    else await rows.nth(wanted >= 0 ? wanted : 0).click();
    const confirm = nav.locator(".confirm");
    if (await confirm.isVisible()) await confirm.click();
    await page.waitForTimeout(150);
  }
  await expect(ask.getByText("Everything is chosen")).toBeVisible();
  await fits(page, "creation: the review");
  await invoke(page, "creation_cancel");
});

test("the menu reaches inside a section: a category, the open character's tab, a tab of Data", { tag: ["@nav.menu-inside", "@android"] }, async ({ page }) => {
  const menu = () => page.getByRole("button", { name: "Menu", exact: true }).click();
  const inside = (section: string) => page.locator(`.drawer .child[data-in="${section}"]`);
  await openSheet(page);
  await go(page, "Compendium");
  // Folded at first: a section's ▸ shows what is in it.
  await menu();
  await expect(inside("characters")).toHaveCount(0);
  await page.getByRole("button", { name: "What is in Characters" }).click();
  await page.getByRole("button", { name: "What is in Compendium" }).click();
  // Each category with its kind's mark, as the compendium's rows.
  await expect(inside("compendium").filter({ hasText: "Spells" }).locator(".mark")).toHaveText("✦");
  // From the compendium straight to the character's spells.
  await inside("characters").filter({ has: page.getByText("Spells", { exact: true }) }).click();
  await expectTab(page, "Spells");
  // And to a category of the compendium: its results, not an open entry.
  await menu();
  await inside("compendium").nth(2).click();
  await expect(page.locator(".categories select")).toHaveValue("2");
  // The character's note, from the menu too: past the first few tabs.
  await menu();
  await expect(inside("characters").filter({ has: page.getByText("Notes", { exact: true }) })).toHaveCount(0);
  await page.getByRole("button", { name: "More in Characters" }).click();
  await inside("characters").filter({ has: page.getByText("Notes", { exact: true }) }).click();
  await expect(page.locator(".note")).toContainText("Tester.md");
  // A tab of Data.
  await menu();
  await page.getByRole("button", { name: "What is in Data" }).click();
  await inside("data").filter({ has: page.getByText("Backups", { exact: true }) }).click();
  await expect(page.getByRole("combobox", { name: "Data" })).toHaveValue("Backups");
});

test("the header's search, or Ctrl+K, puts the cursor in the compendium's search", { tag: ["@nav.search", "@windows"] }, async ({ page }) => {
  const search = page.locator(".compendium .prompt input");
  await go(page, "Characters");
  await page.getByRole("button", { name: /Search the compendium/ }).click();
  await expect(search).toBeFocused();
  // From anywhere, with the keys.
  await go(page, "Notes");
  await page.keyboard.press("Control+k");
  await expect(search).toBeFocused();
  // Again with a search typed: it is selected, to type over.
  await search.fill("fire");
  await page.locator(".compendium").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("Control+k");
  await expect(search).toBeFocused();
  await expect.poll(() => search.evaluate((i: HTMLInputElement) => [i.selectionStart, i.selectionEnd])).toEqual([0, 4]);
});

test("a narrow window has the app bar and its menu, and every section fits it", { tag: ["@layout.narrow-window", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 360, height: 480 });
  await expect(page.getByRole("navigation", { name: "Sections" })).toBeHidden();
  await expect(page.getByRole("button", { name: "Menu", exact: true })).toBeVisible();
  for (const section of ["Compendium", "Characters", "Notes", "Campaigns", "Data"] as const) {
    await go(page, section);
    await fits(page, `${section} at 360px`);
  }
  await openSheet(page, "Overview");
  await fits(page, "the sheet at 360px");
});

test("☰ hides and shows the notes' folders, hidden at first in a narrow window", { tag: ["@layout.sidebar", "@windows"] }, async ({ page }) => {
  const folders = page.getByPlaceholder("search notes");
  await windowSize(page, { width: 1280, height: 820 });
  await go(page, "Notes");
  await expect(folders).toBeVisible();
  await page.getByRole("button", { name: "Hide the sidebar" }).click();
  await expect(folders).toBeHidden();
  await page.getByRole("button", { name: "Show the sidebar" }).click();
  await expect(folders).toBeVisible();
  // A window with no room for it beside a page: hidden until asked for.
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await windowSize(page, { width: 1100, height: 760 });
  await go(page, "Notes");
  await expect(folders).toBeHidden();
  await page.getByRole("button", { name: "Show the sidebar" }).click();
  await expect(folders).toBeVisible();
});

test("Tab outlines the control the keyboard is on", { tag: ["@layout.focus", "@windows"] }, async ({ page }) => {
  await go(page, "Campaigns");
  await page.locator("body").click({ position: { x: 1, y: 1 } });
  await page.keyboard.press("Tab");
  const outline = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement;
    const s = getComputedStyle(el);
    return { tag: el.tagName, style: s.outlineStyle, width: parseFloat(s.outlineWidth) };
  });
  expect(outline.tag).toBe("BUTTON");
  expect(outline.style).not.toBe("none");
  expect(outline.width).toBeGreaterThanOrEqual(2);
});
