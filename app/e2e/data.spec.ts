import type { Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { CAMPAIGN, choose, dataTab, dialog, expect, fakeFilePicker, go, invoke, test } from "./fixtures";

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
    await go(page, "Data", "Import");
    await expect(pane(page, "Imported sources").locator("summary")).toContainText(/\d+ sources, \d+ entries/);
    const counts = await listed(page);
    for (const source of ["TST", "TSX"]) expect(counts[source], source).toBeGreaterThan(0);
    await expect(pane(page, "Imported homebrew")).toContainText(/1 sources, \d+ entries/);
  });

  test("importing again updates the entries instead of doubling them", { tag: "@data.import-later" }, async ({ page }) => {
    await go(page, "Data", "Import");
    const before = await listed(page);
    const brew = await pane(page, "Imported homebrew").innerText();
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import…" }).click();
    await expect(page.locator(".ok")).toContainText("✓");
    expect(await listed(page)).toEqual(before);
    await expect(pane(page, "Imported homebrew")).toHaveText(brew);
  });

  test("about tells the version, the licence, the repository and that the SRD is built in", { tag: "@data.about" }, async ({ page }) => {
    const version = readFileSync(new URL("../../Cargo.toml", import.meta.url), "utf8").match(/^version = "(.+)"/m)![1];
    await go(page, "Data", "About");
    const about = pane(page, "About");
    await expect(about).toContainText("It has the SRD 5.2.1 built in, and reads the books you import.");
    // The attribution the SRD's licence asks for.
    await expect(about).toContainText("This work includes material from the System Reference Document 5.2.1");
    await expect(about).toContainText("Creative Commons Attribution 4.0 International License");
    await expect(about).toContainText(version);
    await expect(about).toContainText("MIT");
    await expect(about).toContainText("github.com/v0v3s/dungeons-n-terminals");
  });
});

test.describe("with a campaign", () => {
  test.use({ seed: "campaign" });

  test("removing the imported homebrew keeps the data", { tag: "@data.forget-homebrew" }, async ({ page }) => {
    await go(page, "Data", "Import");
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
    const before = await invoke<{ entries: number }>(page, "status");
    await go(page, "Data", "Import");
    await page.getByRole("button", { name: "Remove all imported data" }).click();
    await choose(page, "Remove all imported data", "Remove the data");
    // Back to getting started: the built-in SRD is left, and the campaign is still there.
    await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
    await expect(page.getByText(`${CAMPAIGN} is ready`)).toBeVisible();
    const after = await invoke<{ loaded: boolean; entries: number; imported: unknown[]; campaigns: unknown[] }>(page, "status");
    expect(after.loaded).toBe(true);
    expect(after.imported).toHaveLength(0);
    expect(after.entries).toBeLessThan(before.entries);
    expect(after.campaigns).toHaveLength(1);
    // Imported again, it is all back.
    await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import a zip…" }).click();
    await expect(page.locator(".welcome")).toContainText(/Imported \d+ new/);
    expect((await invoke<{ entries: number }>(page, "status")).entries).toBe(before.entries);
    await expect(page.getByText(`${CAMPAIGN} is ready`)).toBeVisible();
  });
});

test("starting over deletes everything, after confirming", { tag: "@data.reset" }, async ({ page }) => {
  await go(page, "Data", "Backups");
  // A backups folder of your choosing: forgotten too.
  await page.evaluate((folder) => ((window as any).dntE2eFiles = { folder: `e2e:${folder}` }), `reset-${Date.now()}`);
  await page.getByRole("button", { name: "Change…" }).click();
  expect((await invoke<{ chosen: boolean }>(page, "backups")).chosen).toBe(true);
  await dataTab(page, "Import");
  await page.getByRole("button", { name: "Reset the app" }).click();
  await expect(dialog(page, "Reset the app")).toContainText("cannot be undone");
  await choose(page, "Reset the app", "Cancel");
  await expect(dialog(page, "Reset the app")).toHaveCount(0);
  expect((await invoke<{ campaigns: unknown[] }>(page, "status")).campaigns).toHaveLength(1);

  await page.getByRole("button", { name: "Reset the app" }).click();
  await choose(page, "Reset the app", "Delete everything");
  await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
  // Only the built-in SRD is left: no imported data, no campaign.
  const s = await invoke<{ loaded: boolean; imported: unknown[]; campaigns: unknown[] }>(page, "status");
  expect(s.loaded).toBe(true);
  expect(s.imported).toHaveLength(0);
  expect(s.campaigns).toHaveLength(0);
  expect((await invoke<{ chosen: boolean }>(page, "backups")).chosen).toBe(false);
  // The characters and notes went with the campaign.
  await page.getByPlaceholder("the campaign's name").fill(CAMPAIGN);
  await page.getByRole("button", { name: "Create the campaign" }).click();
  await page.getByRole("button", { name: "Create a character" }).click();
  await expect(page.locator("button.card")).toHaveCount(0);
});

test("a mistake in the rules is named on the Data screen", { tag: "@features.rules-problems" }, async ({ page }) => {
  await invoke(page, "e2e_put_file", { path: "overlays/mine.toml", text: '[[feature]]\nid = "feat:Nonesuch|TST"\n' });
  // Rules for imported homebrew the campaign does not turn on are fine.
  await invoke(page, "e2e_put_file", { path: "overlays/brew.toml", text: '[[feature]]\nid = "feat:Brewed Luck|TSTB"\n' });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Data", "Problems");
  await expect(pane(page, "Problems")).toContainText("Rules: mine.toml: not in the library: feat:Nonesuch|TST");
  await expect(pane(page, "Problems")).not.toContainText("brew.toml");
});

test("the data's parts are tabs; Problems only while there are some", { tag: "@data.tabs" }, async ({ page }) => {
  await go(page, "Data");
  const tabs = page.getByRole("combobox", { name: "Data" });
  await expect(tabs).toHaveValue("Import");
  await expect(tabs.locator("option")).toHaveText(["Import", "Rules", "Backups", "About"]);
  // One part at a time.
  await expect(page.getByRole("button", { name: "Import…" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Back up now" })).toHaveCount(0);
  await dataTab(page, "Backups");
  await expect(page.getByRole("button", { name: "Back up now" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Import…" })).toHaveCount(0);
  // A problem: its tab, with how many.
  await invoke(page, "e2e_put_file", { path: "overlays/mine.toml", text: '[[feature]]\nid = "feat:Nonesuch|TST"\n' });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Data", "Problems");
  await expect(tabs.locator("option", { hasText: "Problems (1)" })).toHaveCount(1);
  await expect(page.locator(".err")).toContainText("not in the library: feat:Nonesuch|TST");
});
