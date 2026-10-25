import type { Locator, Page } from "@playwright/test";
import { CAMPAIGN, answer, dialog, expect, go, test } from "./fixtures";

const exact = (text: string) => new RegExp(`^${text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);

/// The campaign's books and homebrew, from its card.
async function sources(page: Page): Promise<Locator> {
  await go(page, "Campaigns");
  await page
    .locator("section.card")
    .filter({ has: page.locator(".name", { hasText: exact(CAMPAIGN) }) })
    .getByRole("button", { name: "Books & homebrew" })
    .click();
  const d = dialog(page, `Sources · ${CAMPAIGN}`);
  await expect(d.locator("button.source").first()).toBeVisible();
  return d;
}

/// A book's row in the sources dialog, by its id.
const book = (d: Locator, id: string) => d.locator("button.source").filter({ has: d.page().locator(".dim", { hasText: exact(id) }) });
/// A section of books, by its name.
const group = (d: Locator, name: string) => d.locator("button.group", { hasText: name });
/// A homebrew row, by its name.
const brew = (d: Locator, name: string) => d.locator("button.source.brew", { hasText: name });

async function save(d: Locator) {
  await d.getByRole("button", { name: "Save" }).click();
  await expect(d).toHaveCount(0);
}

/// Search the compendium.
async function search(page: Page, query: string) {
  await go(page, "Compendium");
  await page.locator(".compendium .prompt input").fill(query);
}

/// A result of the compendium's search, by its name and source.
const result = (page: Page, name: string, source: string) =>
  page
    .locator(".compendium ul.rows button")
    .filter({ has: page.locator(".name", { hasText: exact(name) }) })
    .filter({ has: page.locator(".src", { hasText: exact(source) }) });

test.describe("official books", () => {
  test.use({ seed: "campaign" });

  test("a book turned off leaves the compendium and the pickers", { tag: "@sources.toggle-book" }, async ({ page }) => {
    await search(page, "Mirefolk");
    await expect(result(page, "Mirefolk", "TSX")).toBeVisible();

    const d = await sources(page);
    await expect(book(d, "TSX")).toHaveClass(/\bon\b/);
    await book(d, "TSX").click();
    await expect(book(d, "TSX")).not.toHaveClass(/\bon\b/);
    await expect(book(d, "TST")).toHaveClass(/\bon\b/);
    await save(d);

    await search(page, "Mirefolk");
    await expect(page.locator(".compendium ul.rows")).toContainText("Nothing matches.");
    await search(page, "Kindred");
    await expect(result(page, "Kindred", "TST")).toBeVisible();

    // Character creation does not offer it either.
    await go(page, "Characters");
    await page.getByRole("button", { name: "+ New character" }).click();
    await answer(page, "New character", "Newcomer");
    await expect(page.getByText("Choose a species").first()).toBeVisible();
    const picker = page.locator("li button", { hasText: "Kindred" }).first();
    await expect(picker).toBeVisible();
    await expect(page.locator("li button", { hasText: "Mirefolk" })).toHaveCount(0);
  });

  test("a preset ticks its set of books", { tag: "@sources.preset" }, async ({ page }) => {
    const d = await sources(page);
    const presets = d.getByRole("combobox", { name: "Tick a preset" });
    await expect(presets).toHaveValue("all");
    await presets.selectOption("2024 Core");
    // None of the test books is in it.
    for (const id of ["TST", "TSX", "TSA"]) await expect(book(d, id)).not.toHaveClass(/\bon\b/);
    await expect(presets).toHaveValue("2024 Core");
    await save(d);
    await expect(page.locator("section.card", { hasText: CAMPAIGN }).locator(".books")).toContainText("XPHB XDMG XMM");
  });

  test("all books are enabled at once", { tag: "@sources.all" }, async ({ page }) => {
    let d = await sources(page);
    await book(d, "TSX").click();
    await book(d, "TSA").click();
    await save(d);
    // The card names the books (and the source of the campaign's own entries).
    await expect(page.locator("section.card", { hasText: CAMPAIGN }).locator(".books")).toHaveText(/^TST( ·|$)/);

    d = await sources(page);
    const presets = d.getByRole("combobox", { name: "Tick a preset" });
    await expect(presets).toHaveValue("custom");
    await presets.selectOption("all");
    for (const id of ["TST", "TSX", "TSA"]) await expect(book(d, id)).toHaveClass(/\bon\b/);
    await save(d);
    await expect(page.locator("section.card", { hasText: CAMPAIGN }).locator(".books")).toHaveText(/^all official( ·|$)/);
    await search(page, "Mirefolk");
    await expect(result(page, "Mirefolk", "TSX")).toBeVisible();
  });

  test("showing reprinted entries brings back the older version", { tag: "@sources.reprinted" }, async ({ page }) => {
    await search(page, "Spark");
    await expect(result(page, "Spark", "TST")).toBeVisible();
    await expect(result(page, "Spark", "TSX")).toHaveCount(0);

    const d = await sources(page);
    const newest = d.getByRole("checkbox", { name: "Only the newest printing" });
    await expect(newest).toBeChecked();
    await newest.click();
    await expect(newest).not.toBeChecked();
    await save(d);

    await search(page, "Spark");
    await expect(result(page, "Spark", "TSX")).toBeVisible();
    await expect(result(page, "Spark", "TST")).toBeVisible();
  });

  test("the list of books filters by name", { tag: "@sources.filter" }, async ({ page }) => {
    const d = await sources(page);
    await d.getByPlaceholder("filter").fill("outskirts");
    await expect(book(d, "TSX")).toBeVisible();
    await expect(book(d, "TST")).toHaveCount(0);
    await expect(book(d, "TSA")).toHaveCount(0);
    await d.getByPlaceholder("filter").fill("testing caves");
    await expect(book(d, "TSA")).toBeVisible();
    await expect(book(d, "TSX")).toHaveCount(0);
  });

  test("a whole section of books is ticked and unticked at once", { tag: "@sources.group" }, async ({ page }) => {
    const d = await sources(page);
    await expect(group(d, "Supplements")).toContainText("untick all");
    await group(d, "Supplements").click();
    await expect(book(d, "TSX")).not.toHaveClass(/\bon\b/);
    // The adventure is a section of its own.
    await expect(book(d, "TSA")).toHaveClass(/\bon\b/);
    await expect(book(d, "TST")).toHaveClass(/\bon\b/);
    await expect(group(d, "Supplements")).toContainText("tick all");
    await group(d, "Supplements").click();
    for (const id of ["TST", "TSX", "TSA"]) await expect(book(d, id)).toHaveClass(/\bon\b/);
    await group(d, "Core rulebooks").click();
    await expect(book(d, "TST")).not.toHaveClass(/\bon\b/);
    await save(d);
    await search(page, "Kindred");
    await expect(page.locator(".compendium ul.rows")).toContainText("Nothing matches.");
  });
});

test.describe("homebrew", () => {
  test.use({ seed: "campaign" });

  test("homebrew turned on shows its entries; all off removes them", { tag: "@sources.homebrew" }, async ({ page }) => {
    await search(page, "Pocket Sundial");
    await expect(page.locator(".compendium ul.rows")).toContainText("Nothing matches.");

    let d = await sources(page);
    await d.getByRole("button", { name: /^Homebrew/ }).click();
    await expect(brew(d, "Proving Grounds Brew")).not.toHaveClass(/\bon\b/);
    await brew(d, "Proving Grounds Brew").click();
    await expect(brew(d, "Proving Grounds Brew")).toHaveClass(/\bon\b/);
    await save(d);
    await search(page, "Pocket Sundial");
    await expect(result(page, "Pocket Sundial", "TSTB")).toBeVisible();

    d = await sources(page);
    await d.getByRole("button", { name: /^Homebrew/ }).click();
    await group(d, "Enabled").click();
    await expect(brew(d, "Proving Grounds Brew")).not.toHaveClass(/\bon\b/);
    await save(d);
    await search(page, "Pocket Sundial");
    await expect(page.locator(".compendium ul.rows")).toContainText("Nothing matches.");
  });

  test("the homebrew list filters by name and by author", { tag: "@sources.homebrew-filter" }, async ({ page }) => {
    const d = await sources(page);
    await d.getByRole("button", { name: /^Homebrew/ }).click();
    const filter = d.getByPlaceholder("name or author");
    await filter.fill("proving brew");
    await expect(brew(d, "Proving Grounds Brew")).toBeVisible();
    await filter.fill("nothing like it");
    await expect(brew(d, "Proving Grounds Brew")).toHaveCount(0);
    await filter.fill("test guild");
    await expect(brew(d, "Proving Grounds Brew")).toBeVisible();
  });

  test("the homebrew list filters by rules edition", { tag: "@sources.homebrew-edition" }, async ({ page }) => {
    const d = await sources(page);
    await d.getByRole("button", { name: /^Homebrew/ }).click();
    const rules = d.getByRole("combobox", { name: "Rules" });
    await rules.selectOption("2024");
    await expect(brew(d, "Proving Grounds Brew")).toContainText("2024");
    await rules.selectOption("2014");
    await expect(brew(d, "Proving Grounds Brew")).toHaveCount(0);
    await rules.selectOption("Any rules");
    await expect(brew(d, "Proving Grounds Brew")).toBeVisible();
  });
});
