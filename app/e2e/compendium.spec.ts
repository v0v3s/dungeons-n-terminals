import type { Page } from "@playwright/test";
import { answer, choose, dialog, entryAction, expect, go, phone, test } from "./fixtures";
import { drag, swipe } from "./touch";

test.use({ seed: "campaign" });

const exact = (name: string) => new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);

/// The compendium, its results in the best order (the order is remembered).
async function compendium(page: Page) {
  await go(page, "Compendium");
  const order = page.locator(".order select");
  await expect(order).toBeVisible();
  if ((await order.inputValue()) !== "0") await order.selectOption({ label: "best match" });
}

const searchBox = (page: Page) => page.locator(".compendium .prompt input").filter({ visible: true });
const names = (page: Page) => page.locator(".rows .name");
const sources = (page: Page) => page.locator(".rows .src");
const heading = (page: Page) => page.locator(".detail h1");
const doc = (page: Page) => page.locator(".detail .doc");

/// On a phone the entry hides the list: back to it.
async function toList(page: Page) {
  if (await phone(page)) {
    const backButton = page.getByRole("button", { name: "Back", exact: true });
    while (await backButton.isVisible()) await backButton.click();
  }
}

async function search(page: Page, query: string) {
  await toList(page);
  await searchBox(page).fill(query);
  // Done typing: on a phone the keyboard goes, and the list moves with it.
  await searchBox(page).blur();
}

/// Tap a result to read it (again while the list is still moving).
async function read(page: Page, name: string) {
  const row = page.locator(".rows button", { has: page.locator(".name", { hasText: exact(name) }) }).first();
  await expect(async () => {
    await row.click();
    await expect(heading(page).filter({ visible: true })).toContainText(name, { timeout: 1500 });
  }).toPass({ timeout: 10_000 });
}

/// One step back: the app bar's Back on a phone, the entry's own elsewhere.
async function back(page: Page) {
  if (await phone(page)) await page.getByRole("button", { name: "Back", exact: true }).click();
  else await page.locator(".detail button.back").click();
}

/// The category select's option for `label` ("Spells · 12"): its value.
async function categoryValue(page: Page, label: string) {
  const options = await page.locator(".categories select option").allTextContents();
  const i = options.findIndex((o) => o.startsWith(`${label} ·`));
  if (i < 0) throw new Error(`No category ${label} in ${options.join(", ")}`);
  return String(i);
}

async function category(page: Page, label: string) {
  await toList(page);
  const value = await categoryValue(page, label);
  await page.locator(".categories select").selectOption(value);
  await expect(page.locator(".categories select")).toHaveValue(value);
}

test("the results follow what is typed", { tag: "@compendium.search" }, async ({ page }) => {
  await compendium(page);
  await searchBox(page).pressSequentially("emb");
  await expect(names(page).first()).toBeVisible();
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Emberkin" }).first()).toBeVisible();
  await searchBox(page).pressSequentially("er bolt");
  await expect(names(page).first()).toHaveText("Ember Bolt");
  await search(page, "zzqxj");
  await expect(page.locator(".rows")).toContainText("Nothing matches.");
});

test("filters narrow a search", { tag: "@compendium.filters" }, async ({ page }) => {
  await compendium(page);
  // @source: only the supplement's entries.
  await search(page, "@tsx ");
  await expect(names(page).filter({ hasText: "Outskirt Gust" })).toHaveCount(1);
  for (const s of await sources(page).allTextContents()) expect(s.trim()).toBe("TSX");

  // #kind and ^level.
  await category(page, "Spells");
  await search(page, "#cantrip ");
  await expect(names(page).filter({ hasText: "Frost Needle" })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(0);
  await search(page, "^3");
  await expect(names(page)).toHaveText(["Storm Lance"]);

  // "text" in the entry, and ! not.
  await search(page, '"dim light"');
  await expect(names(page)).toHaveText(["Glimmer"]);
  await search(page, "!#cantrip ");
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Frost Needle" })).toHaveCount(0);
});

test("a category narrows the results to its kind", { tag: "@compendium.category" }, async ({ page }) => {
  await compendium(page);
  await category(page, "Spells");
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Thornknife" })).toHaveCount(0);
  await category(page, "Items");
  await expect(names(page).filter({ hasText: exact("Thornknife") })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(0);
  await category(page, "Rules");
  await expect(names(page).filter({ hasText: "Proving Rule" })).toHaveCount(1);
  await category(page, "All");
  await expect(names(page).filter({ hasText: exact("Thornknife") })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Ember Bolt" })).toHaveCount(1);
});

test("results are ordered by name, source or level", { tag: "@compendium.order" }, async ({ page }) => {
  await compendium(page);
  await category(page, "Spells");
  const order = page.locator(".order select");
  try {
    await order.selectOption({ label: "name A–Z" });
    await expect(names(page).first()).toHaveText("Ember Bolt");
    await order.selectOption({ label: "name Z–A" });
    await expect(names(page).first()).toHaveText("Ward of Thorns");
    await order.selectOption({ label: "level · rarity" });
    await expect(names(page).last()).toHaveText("Unmaking Word");
    await expect(names(page).first()).toHaveText("Frost Needle");
    // The supplement's own spell sorts after the core book's.
    await order.selectOption({ label: "source" });
    await expect(names(page).last()).toHaveText("Outskirt Gust");
  } finally {
    await order.selectOption({ label: "best match" });
  }
});

test("an entry shows its fields, text, tables and lists", { tag: "@compendium.read" }, async ({ page }) => {
  await compendium(page);
  await search(page, "proving rule");
  await read(page, "Proving Rule");
  await expect(page.locator(".detail .sub")).toContainText("The Proving Grounds");
  await expect(doc(page)).toContainText("Roll 1d20 and add your modifier.");
  const table = doc(page).locator("table");
  await expect(doc(page).locator(".caption")).toHaveText("Outcomes");
  await expect(table.locator("th")).toHaveText(["d6", "Result"]);
  await expect(table.locator("tbody tr").first().locator("td")).toHaveText(["1-3", "Fail"]);
  await expect(table.locator("tbody tr").last().locator("td")).toHaveText(["4-6", "Pass"]);
  await expect(doc(page).locator("li")).toHaveText(["First, look.", "Then, leap."]);

  // A spell's fields.
  await search(page, "glimmer");
  await read(page, "Glimmer");
  await expect(doc(page).locator("dl")).toContainText("Touch");
  await expect(doc(page).locator("dl")).toContainText("1 hour");
  await expect(doc(page)).toContainText("An object sheds dim light.");
});

test("a link opens its entry, and back returns", { tag: "@compendium.follow-link" }, async ({ page }) => {
  await compendium(page);
  await category(page, "Features");
  await search(page, "spellcasting #mage ");
  await read(page, "Spellcasting");
  await doc(page).getByRole("link", { name: "Proving Rule" }).click();
  await expect(heading(page)).toContainText("Proving Rule");
  await expect(doc(page).locator("table")).toBeVisible();
  await back(page);
  await expect(heading(page)).toContainText("Spellcasting");
  await doc(page).getByRole("link", { name: "Grimoire" }).click();
  await expect(heading(page)).toContainText("Grimoire");
  await back(page);
  await expect(heading(page)).toContainText("Spellcasting");
});

test("an entry is cloned as a campaign version or as a named copy", { tag: "@compendium.clone" }, async ({ page }) => {
  await compendium(page);
  await search(page, "glimmer");
  await read(page, "Glimmer");
  await entryAction(page, "Glimmer", "Clone into the campaign");
  await choose(page, "Clone Glimmer", /A campaign version/);
  const editor = dialog(page, "Edit · Glimmer");
  await expect(editor).toContainText("campaign version");
  await editor.getByRole("button", { name: "Done" }).click();
  // It replaces the book entry: still one Glimmer, with its book original.
  await toList(page);
  await expect(names(page).filter({ hasText: exact("Glimmer") })).toHaveCount(1);
  await read(page, "Glimmer");
  await expect(page.getByRole("button", { name: "Book original" })).toBeVisible();

  await toList(page);
  await search(page, "frost needle");
  await read(page, "Frost Needle");
  await entryAction(page, "Frost Needle", "Clone into the campaign");
  await choose(page, "Clone Frost Needle", /A separate copy/);
  await expect(dialog(page, "The copy's name").getByRole("textbox")).toHaveValue("Frost Needle (copy)");
  await answer(page, "The copy's name", "Frost Lance");
  const copy = dialog(page, "Edit · Frost Lance");
  await expect(copy).toContainText("campaign homebrew");
  await copy.getByRole("button", { name: "Done" }).click();
  await search(page, "frost");
  await expect(names(page).filter({ hasText: "Frost Needle" })).toHaveCount(1);
  await expect(names(page).filter({ hasText: "Frost Lance" })).toHaveCount(1);
});

test("a campaign version and its book original", { tag: "@compendium.original" }, async ({ page }) => {
  await compendium(page);
  await search(page, "glimmer");
  await read(page, "Glimmer");
  await entryAction(page, "Glimmer", "Clone into the campaign");
  await choose(page, "Clone Glimmer", /A campaign version/);
  const editor = dialog(page, "Edit · Glimmer");
  await editor.locator("button.field", { hasText: "Description" }).click();
  await dialog(page, "Description").getByRole("textbox").fill("A spark of moonlight hangs in the air.");
  await dialog(page, "Description").getByRole("button", { name: "Save" }).click();
  await editor.getByRole("button", { name: "Done" }).click();

  await toList(page);
  await read(page, "Glimmer");
  await expect(doc(page)).toContainText("A spark of moonlight");
  await page.getByRole("button", { name: "Book original" }).click();
  await expect(doc(page)).toContainText("An object sheds dim light.");
  await expect(doc(page)).not.toContainText("A spark of moonlight");
  await page.getByRole("button", { name: "The campaign version" }).click();
  await expect(doc(page)).toContainText("A spark of moonlight");
});

test("a new homebrew entry from a template", { tag: "@compendium.new-homebrew" }, async ({ page }) => {
  await compendium(page);
  await page
    .getByRole("button", { name: "New homebrew entry" })
    .or(page.getByRole("button", { name: "+ new" }))
    .filter({ visible: true })
    .first()
    .click();
  await choose(page, "New homebrew entry", "Spell");
  await answer(page, "New spell", "Moth Whisper");
  const editor = dialog(page, "Edit · Moth Whisper");
  await expect(editor).toContainText("campaign homebrew");
  await editor.getByRole("button", { name: "Done" }).click();
  await search(page, "moth whisper");
  await read(page, "Moth Whisper");
  await expect(page.locator(".detail .sub")).toContainText("Spell");
});

test("swiping the results changes the category, the gap naming the next one", { tag: ["@compendium.swipe-categories", "@android"] }, async ({ page }) => {
  await compendium(page);
  await toList(page);
  const select = page.locator(".categories select");
  const first = await select.inputValue();
  const second = String(Number(first) + 1);
  const results = page.locator(".swipe");
  const next = (await select.locator("option").allTextContents())[Number(second)];
  // Part of the way: the gap names the next category; it slides back.
  await drag(page, results, 0.8, [0.65, 0.5], async () => {
    await expect(page.locator(".compendium .swipe-hint")).toHaveText(next);
  });
  await expect(page.locator(".compendium .swipe-hint")).toHaveCount(0);
  await expect(select).toHaveValue(first);
  await swipe(page, results, 0.8, 0.2);
  await expect(select).toHaveValue(second);
  await swipe(page, results, 0.2, 0.8);
  await expect(select).toHaveValue(first);
});

test("an entry is exported and imported", { tag: "@export.entry" }, async ({ page }) => {
  await page.evaluate(() => ((window as any).dntE2eFiles = { open: null, save: "e2e:" }));
  // A homebrew entry, from a campaign with that homebrew on.
  await page.evaluate(() =>
    (window as any).__TAURI_INTERNALS__.invoke("set_campaign_sources", { slug: "e2e", sources: [], homebrew: ["TSTB"], showReprinted: false }),
  );
  await go(page, "Compendium");
  await search(page, "sun glint");
  await read(page, "Sun Glint");
  await entryAction(page, "Sun Glint", "Export");
  await expect(page.locator(".notice")).toContainText("Exported Sun Glint");
  // Off again; the import turns it on.
  await page.evaluate(() =>
    (window as any).__TAURI_INTERNALS__.invoke("set_campaign_sources", { slug: "e2e", sources: [], homebrew: [], showReprinted: false }),
  );
  await go(page, "Data");
  await page.evaluate(() => ((window as any).dntE2eFiles = { open: "e2e:Sun Glint.json", save: "e2e:" }));
  await page.getByRole("button", { name: "Import…" }).click();
  await expect(page.locator(".data p.ok")).toContainText("Imported Sun Glint");
  await go(page, "Compendium");
  await search(page, "sun glint");
  await read(page, "Sun Glint");
});
