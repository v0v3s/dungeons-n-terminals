import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { CAMPAIGN, choose, dialog, expect, fakeFilePicker, go, invoke, test } from "./fixtures";

/// A pane of the Data screen, by its title.
const pane = (page: Page, title: string) =>
  page.locator("section.pane").filter({ has: page.locator("h2.title", { hasText: new RegExp(`^${title}$`) }) });

/// The imported sources, as the Data screen lists them (source → entries).
async function listed(page: Page) {
  const sources = pane(page, "Imported sources");
  if (!(await sources.locator(".sources").isVisible())) await sources.locator("summary").click();
  const rows = await sources.locator(".sources > div").allInnerTexts();
  return Object.fromEntries(rows.map((r) => r.trim().split(/\s+/)).map(([s, n]) => [s, Number(n)]));
}

test.describe("with the data", () => {
  test.use({ seed: "data" });

  test("the imported sources are listed with their entry counts, and the homebrew", { tag: "@data.list" }, async ({ page }) => {
    await go(page, "Data");
    await expect(pane(page, "Imported sources").locator("summary")).toContainText(/\d+ sources, \d+ entries/);
    const counts = await listed(page);
    for (const source of ["TST", "TSX"]) expect(counts[source], source).toBeGreaterThan(0);
    await expect(pane(page, "Imported homebrew")).toContainText(/1 sources, \d+ entries/);
  });

  test("importing again updates the entries instead of doubling them", { tag: "@data.import-later" }, async ({ page }) => {
    await go(page, "Data");
    const before = await listed(page);
    const brew = await pane(page, "Imported homebrew").innerText();
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import a zip…" }).click();
    await expect(page.locator(".ok")).toContainText("✓");
    expect(await listed(page)).toEqual(before);
    await expect(pane(page, "Imported homebrew")).toHaveText(brew);
  });

  test("about tells the version, the licence, the repository and that no game content ships", { tag: "@data.about" }, async ({ page }) => {
    const version = readFileSync(new URL("../../Cargo.toml", import.meta.url), "utf8").match(/^version = "(.+)"/m)![1];
    await go(page, "Data");
    const about = pane(page, "About");
    await expect(about).toContainText("ships no game content");
    await expect(about).toContainText(version);
    await expect(about).toContainText("MIT");
    await expect(about).toContainText("github.com/v0v3s/dungeons-n-terminals");
  });
});

test.describe("with a campaign", () => {
  test.use({ seed: "campaign" });

  test("removing the imported homebrew keeps the data", { tag: "@data.forget-homebrew" }, async ({ page }) => {
    await go(page, "Data");
    const before = await listed(page);
    await page.getByRole("button", { name: "Remove the imported homebrew" }).click();
    await expect(dialog(page, "Remove the imported homebrew")).toContainText("every imported homebrew source");
    await choose(page, "Remove the imported homebrew", "Remove homebrew");
    await expect(pane(page, "Imported homebrew")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Remove the imported homebrew" })).toHaveCount(0);
    const after = await listed(page);
    for (const source of ["TST", "TSX", "TSA"]) expect(after[source], source).toBe(before[source]);
  });

  test("removing all the data keeps the campaigns", { tag: "@data.forget-all" }, async ({ page }) => {
    await go(page, "Data");
    await page.getByRole("button", { name: "Remove all imported data" }).click();
    await choose(page, "Remove all imported data", "Remove the data");
    // Back to getting started, at the data; the campaign is still there.
    await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import a zip…" }).click();
    await expect(page.getByText(`${CAMPAIGN} is ready`)).toBeVisible();
  });
});

test("starting over deletes everything, after confirming", { tag: "@data.reset" }, async ({ page }) => {
  await go(page, "Data");
  await page.getByRole("button", { name: "Reset the app" }).click();
  await expect(dialog(page, "Reset the app")).toContainText("cannot be undone");
  await choose(page, "Reset the app", "Cancel");
  await expect(dialog(page, "Reset the app")).toHaveCount(0);
  expect((await invoke<{ campaigns: unknown[] }>(page, "status")).campaigns).toHaveLength(1);

  await page.getByRole("button", { name: "Reset the app" }).click();
  await choose(page, "Reset the app", "Delete everything");
  await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
  const s = await invoke<{ loaded: boolean; campaigns: unknown[] }>(page, "status");
  expect(s.loaded).toBe(false);
  expect(s.campaigns).toHaveLength(0);
  // The characters and notes went with the campaign.
  await fakeFilePicker(page);
  await page.getByRole("button", { name: "Import a zip…" }).click();
  await page.getByPlaceholder("the campaign's name").fill(CAMPAIGN);
  await page.getByRole("button", { name: "Create the campaign" }).click();
  await page.getByRole("button", { name: "Create a character" }).click();
  await expect(page.locator("button.card")).toHaveCount(0);
});
