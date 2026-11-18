// The app on a computer: the compendium, the notes, the campaigns and the
// characters, every action in sight.
import type { Page } from "@playwright/test";
import { CAMPAIGN, expect, fits, go, invoke, openSheet, pick, test, windowSize } from "./fixtures";

const search = (page: Page) => page.locator(".compendium .prompt input");

test("the compendium's search runs across the top, its categories and facets as chips", { tag: ["@compendium.top-search", "@windows"] }, async ({ page }) => {
  await go(page, "Compendium");
  const top = (await search(page).boundingBox())!;
  const results = (await page.locator(".compendium ul.rows").boundingBox())!;
  expect(top.y).toBeLessThan(results.y);
  // A category by its chip.
  const categories = page.getByRole("group", { name: "Categories" });
  const spells = categories.getByRole("button", { name: /^Spells \d/ });
  await spells.click();
  await expect(spells).toHaveAttribute("aria-pressed", "true");
  // A facet's chip adds it to the search.
  await search(page).fill("fire ");
  const facets = page.getByRole("group", { name: "Search by" });
  await facets.getByRole("button", { name: /^\^/ }).click();
  await expect(search(page)).toHaveValue(/fire \^$/);
  // While typing, the suggestions are chips that complete it.
  await search(page).fill("#");
  const suggestion = facets.getByRole("button").nth(1);
  const value = (await suggestion.textContent())!.trim().split(/\s+/)[0];
  await suggestion.click();
  await expect(search(page)).toHaveValue(new RegExp(`^#${value.replace(/^#/, "")}`));
});

test("an entry's actions are buttons by its name", { tag: ["@compendium.entry-actions", "@windows"] }, async ({ page }) => {
  await go(page, "Compendium");
  await search(page).fill("fireball");
  await page.locator(".compendium ul.rows button").first().click();
  const acts = page.getByRole("group", { name: "Fireball: actions" });
  await expect(acts.getByRole("button", { name: "Export" })).toBeVisible();
  await acts.getByRole("button", { name: "Clone into the campaign" }).click();
  await expect(page.getByRole("dialog", { name: "Clone Fireball" })).toBeVisible();
});

test("the notes' actions are in sight, the links to a note beside it", { tag: ["@notes.visible-actions", "@windows"] }, async ({ page }) => {
  for (const [path, text] of Object.entries({ "Bridge.md": "# Bridge\n\nAsk [[Old Miller]].\n", "NPCs/Old Miller.md": "# Old Miller\n\nGrumpy.\n" })) await invoke(page, "note_write", { path, text });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Notes");
  const top = page.getByRole("group", { name: "Notes: actions" });
  await expect(top.getByRole("button", { name: "New folder" })).toBeVisible();
  await expect(top.getByRole("button", { name: "Today's session note" })).toBeVisible();
  // A folder chosen: its own actions, on its row in the tree.
  await page.locator("button.row.dir", { hasText: "NPCs" }).click();
  const folder = page.getByRole("group", { name: "NPCs: actions" });
  for (const name of ["Rename NPCs", "Delete NPCs…", "Move NPCs up", "Move NPCs down"]) await expect(folder.getByRole("button", { name })).toBeVisible();
  await expect(page.locator(".rows .chosen")).toContainText("NPCs");
  await expect(page.locator(".rows .chosen").getByRole("group", { name: "NPCs: actions" })).toBeVisible();
  // The toolbar: + Note (in NPCs), Quick note, new folder, session note and
  // ⋯, one size, one row, at the top of the note's side (not the folders').
  const bar = page.getByRole("toolbar", { name: "Notes" });
  await expect(bar.locator("button.new")).toHaveText("+ Note · NPCs");
  await expect(page.locator(".tree").getByRole("toolbar", { name: "Notes" })).toHaveCount(0);
  await expect(page.locator(".note").getByRole("toolbar", { name: "Notes" })).toBeVisible();
  const rows = await bar.locator("button").evaluateAll((bs) => new Set(bs.map((b) => Math.round(b.getBoundingClientRect().top))).size);
  expect(rows).toBe(1);
  const heights = await bar.locator("button").evaluateAll((bs) => new Set(bs.map((b) => Math.round(b.getBoundingClientRect().height))).size);
  expect(heights).toBe(1);
  // A note open: its actions, and what links to it beside the text.
  await page.locator("button.row:not(.dir)", { hasText: "Old Miller" }).click();
  const acts = page.getByRole("group", { name: "Old Miller: actions" });
  await expect(acts.getByRole("button", { name: "Delete…" })).toBeVisible();
  // The toolbar above the note's own actions.
  const note = (await page.locator(".note .ntools").boundingBox())!;
  expect((await bar.boundingBox())!.y + (await bar.boundingBox())!.height).toBeLessThanOrEqual(note.y + 1);
  const linked = page.locator(".note .backlinks");
  await expect(linked).toContainText("Bridge");
  const text = (await page.locator(".note .read").boundingBox())!;
  const beside = (await linked.boundingBox())!;
  expect(beside.x).toBeGreaterThan(text.x + text.width - 1);
  // The note's toolbar: one size too.
  const sizes = await page.locator(".note .ntools button").evaluateAll((bs) => new Set(bs.map((b) => Math.round(b.getBoundingClientRect().height))).size);
  expect(sizes).toBe(1);
  await acts.getByRole("button", { name: "Rename" }).click();
  await expect(page.getByRole("dialog", { name: "Rename" })).toBeVisible();
});

test("the campaigns are cards side by side, their actions in sight", { tag: ["@campaigns.cards", "@windows"] }, async ({ page }) => {
  await invoke(page, "create_campaign", { name: `${CAMPAIGN} Two`, sources: [] });
  await invoke(page, "activate_campaign", { slug: (await invoke<{ campaigns: { name: string; slug: string }[] }>(page, "status")).campaigns.find((c) => c.name === CAMPAIGN)!.slug });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Campaigns");
  const cards = page.locator("section.card");
  await expect(cards).toHaveCount(2);
  const [a, b] = [(await cards.nth(0).boundingBox())!, (await cards.nth(1).boundingBox())!];
  expect(Math.abs(a.y - b.y)).toBeLessThan(2);
  const two = page.getByRole("group", { name: `${CAMPAIGN} Two: actions` });
  for (const name of ["Books & homebrew", "Rename", "Export", "Delete"]) await expect(two.getByRole("button", { name, exact: true })).toBeVisible();
  await expect(cards.filter({ hasText: `${CAMPAIGN} Two` }).getByRole("button", { name: "Make active" })).toBeVisible();
  await two.getByRole("button", { name: "Rename", exact: true }).click();
  await expect(page.getByRole("dialog", { name: `Rename ${CAMPAIGN} Two` })).toBeVisible();
});

test("the characters are cards side by side", { tag: ["@characters.cards", "@windows"] }, async ({ page }) => {
  await go(page, "Characters");
  const cards = page.locator("button.card");
  await expect(cards).toHaveCount(4);
  const tops = await cards.evaluateAll((cs) => new Set(cs.map((c) => Math.round(c.getBoundingClientRect().top))).size);
  expect(tops).toBe(1);
  // They fill the row, however few, on any window.
  for (const width of [1280, 2560, 3840]) {
    await windowSize(page, { width, height: 1000 });
    const fill = await page.locator(".cards").evaluate((g) => {
      const box = g.getBoundingClientRect();
      const right = Math.max(...[...g.children].map((c) => c.getBoundingClientRect().right));
      return (right - box.left) / box.width;
    });
    expect(fill, `at ${width}px`).toBeGreaterThan(0.97);
  }
});

test("a dialog is a card with a ✕ that closes it", { tag: ["@layout.dialogs", "@windows"] }, async ({ page }) => {
  await go(page, "Campaigns");
  await page.getByRole("button", { name: "+ New campaign" }).click();
  const box = page.getByRole("dialog", { name: "New campaign" });
  await expect(box).toBeVisible();
  const title = (await box.getByRole("heading", { name: "New campaign" }).boundingBox())!;
  const frame = (await box.boundingBox())!;
  expect(title.y).toBeGreaterThan(frame.y);
  await box.getByRole("button", { name: "Close the dialog" }).click();
  await expect(box).toBeHidden();
});

test("a dialog opens and closes smoothly, and an entry's reads cleanly at any size", { tag: ["@layout.dialogs", "@windows"] }, async ({ page }) => {
  await openSheet(page, "Spells");
  await page.getByRole("button", { name: "+ add" }).first().click();
  await pick(page, ["Fire Bolt"]);
  await page.locator("button.spell", { hasText: "Fire Bolt" }).click();
  const read = page.getByRole("button", { name: "Read it" });
  // Opening: it fades in from a little smaller (a short animation running
  // right after the click), then stands still at full size.
  const opening = await read.evaluate(async (b: HTMLElement) => {
    b.click();
    for (let i = 0; i < 30 && !document.querySelector(".modal"); i++) await new Promise(requestAnimationFrame);
    const m = document.querySelector(".modal")!;
    await new Promise(requestAnimationFrame);
    return { running: m.getAnimations().length > 0, opacity: Number(getComputedStyle(m).opacity) };
  });
  expect(opening.running).toBe(true);
  expect(opening.opacity).toBeLessThan(1);
  const entry = page.locator(".modal").and(page.getByRole("dialog", { name: "Fire Bolt" }));
  await expect(entry).toContainText("Evocation");
  await expect.poll(() => entry.evaluate((m) => m.getAnimations().length)).toBe(0);
  expect(await entry.evaluate((m) => getComputedStyle(m).opacity)).toBe("1");
  // Inside the window, its text inside it, at every size.
  for (const size of [
    { width: 1280, height: 820 },
    { width: 900, height: 600 },
    { width: 2560, height: 1440 },
  ]) {
    await windowSize(page, size);
    const box = (await entry.boundingBox())!;
    expect(box.x, `at ${size.width}px`).toBeGreaterThanOrEqual(0);
    expect(box.y, `at ${size.width}px`).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width, `at ${size.width}px`).toBeLessThanOrEqual(size.width);
    expect(box.y + box.height, `at ${size.width}px`).toBeLessThanOrEqual(size.height);
    await fits(page, `the entry dialog at ${size.width}px`);
  }
  // Closing: it fades out the same way, then is gone.
  await page.keyboard.press("Escape");
  await expect(entry).toBeHidden();
});

test("on a big screen the characters and campaigns take more columns", { tag: ["@layout.wide", "@windows"] }, async ({ page }) => {
  const columns = (sel: string) => page.locator(sel).evaluateAll((cs) => new Set(cs.map((c) => Math.round(c.getBoundingClientRect().left))).size);
  await go(page, "Characters");
  const narrow = await columns("button.card");
  await windowSize(page, { width: 2560, height: 1440 });
  await go(page, "Compendium");
  await go(page, "Characters");
  await expect(page.locator("button.card")).toHaveCount(4);
  expect(await columns("button.card")).toBeGreaterThanOrEqual(narrow);
  // The compendium: the search spans the window above its two columns.
  await go(page, "Compendium");
  const bar = (await page.locator(".compendium .searchwide").boundingBox())!;
  expect(bar.width).toBeGreaterThan(2560 * 0.6);
});
