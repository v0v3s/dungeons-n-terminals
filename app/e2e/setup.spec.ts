import type { Page } from "@playwright/test";
import { expect, fakeFilePicker, go, invoke, test } from "./fixtures";

/// A step of getting started, by its title.
const step = (page: Page, title: string) =>
  page.locator("section.pane").filter({ has: page.locator("h2.title", { hasText: title }) });

/// Create a campaign from getting started, and browse the compendium.
async function createAndBrowse(page: Page, name: string) {
  await page.getByPlaceholder("the campaign's name").fill(name);
  await page.getByRole("button", { name: "Create the campaign" }).click();
  await expect(step(page, "Create a campaign")).toContainText(`${name} is ready`);
  await page.getByRole("button", { name: "Browse the compendium" }).click();
}

/// The compendium searched for `query`: a result named `name`.
async function finds(page: Page, query: string, name: string) {
  await page.locator(".compendium .prompt input").filter({ visible: true }).fill(query);
  await expect(page.locator(".rows .name", { hasText: new RegExp(`^${name}$`) }).first()).toBeVisible();
}

test.describe("a new app", () => {
  test.use({ seed: "empty" });

  test("the first start has the SRD built in, and asks for a campaign", { tag: "@setup.srd-built-in" }, async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
    // Step 1 is done: nothing to import.
    await expect(step(page, "✓ 1 · The rules")).toContainText("System Reference Document 5.2.1 are built in");
    await expect(step(page, "▸ 2 · Create a campaign").getByPlaceholder("the campaign's name")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create the campaign" })).toBeDisabled();
    await createAndBrowse(page, "Westmarch");
    await finds(page, "fireball", "Fireball");
  });

  test("a zip imported at getting started joins the SRD", { tag: "@setup.import-first-run" }, async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
    const before = (await invoke<{ entries: number }>(page, "status")).entries;
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import a zip…" }).click();
    await expect(step(page, "The rules")).toContainText(/Imported \d+ new/);
    await expect(step(page, "The rules")).toContainText("System Reference Document 5.2.1 are built in");
    expect((await invoke<{ entries: number }>(page, "status")).entries).toBeGreaterThan(before);
    await expect(step(page, "Create a campaign").getByPlaceholder("the campaign's name")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create the campaign" })).toBeDisabled();
    // The compendium has both: the imported books and the SRD.
    await createAndBrowse(page, "Westmarch");
    await finds(page, "mirefolk", "Mirefolk");
    await finds(page, "fireball", "Fireball");
  });
});

test.describe("with the data", () => {
  test.use({ seed: "data" });

  test("the first campaign is created and becomes the active one", { tag: "@setup.first-campaign" }, async ({ page }) => {
    await page.getByPlaceholder("the campaign's name").fill("E2E First");
    await page.getByRole("button", { name: "Create the campaign" }).click();
    await expect(step(page, "Create a campaign")).toContainText("E2E First is ready");
    await page.getByRole("button", { name: "Create a character" }).click();
    await expect(page.getByRole("button", { name: "+ New character" })).toBeVisible();
    await go(page, "Campaigns");
    const card = page.locator("section.card", { hasText: "E2E First" });
    await expect(card.locator(".badge")).toHaveText("active");
    expect((await invoke<{ campaign: string }>(page, "status")).campaign).toBe("E2E First");
  });
});
