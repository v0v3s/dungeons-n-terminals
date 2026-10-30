import type { Page } from "@playwright/test";
import { CAMPAIGN, campaignAction, choose, dialog, expect, go, invoke, openSheet, sheetMore, status, test } from "./fixtures";

/// The Data screen's backups, newest first.
const saved = (page: Page) => page.locator(".saved");
/// What the last import or backup did, on the Data screen.
const done = (page: Page) => page.locator(".data p.ok");

/// Answer the next open dialog with `path`, and the next save to e2e:.
async function files(page: Page, open: string | null) {
  await page.evaluate((open) => ((window as any).dntE2eFiles = { open, save: "e2e:" }), open);
}

/// Import a file saved before (e2e:<name>) from Data › Import.
async function importSaved(page: Page, name: string) {
  await go(page, "Data", "Import");
  await files(page, `e2e:${name}`);
  await page.getByRole("button", { name: "Import…" }).click();
}

/// A character using the test homebrew's feat, in a campaign with it on.
async function brewer(page: Page) {
  await invoke(page, "set_campaign_sources", { slug: "e2e", sources: [], homebrew: ["TSTB"], showReprinted: false });
  await invoke(page, "e2e_put_file", {
    path: "characters/brewer.toml",
    text: 'name = "Brewer"\nspecies = "race:Stoneling|TST"\nfeats = ["feat:Brewed Luck|TSTB"]\n\n[abilities]\nstr = 10\ndex = 10\ncon = 10\nint = 10\nwis = 10\ncha = 10\n\n[[levels]]\nclass = "class:Wizard|SRD52"\n',
  });
}

test("everything is backed up and listed", { tag: "@backup.now" }, async ({ page }) => {
  await go(page, "Data", "Backups");
  await expect(saved(page)).toHaveCount(0);
  await page.getByRole("button", { name: "Back up now" }).click();
  await expect(done(page)).toContainText("Backed up to dnt-backup-");
  await expect(saved(page)).toHaveCount(1);
  await expect(saved(page).first()).toContainText(/MB|kB/);
});

test("a backup is restored, and a broken one changes nothing", { tag: "@backup.restore" }, async ({ page }) => {
  await go(page, "Data", "Backups");
  await page.getByRole("button", { name: "Back up now" }).click();
  await expect(saved(page)).toHaveCount(1);
  // Someone new after the backup.
  await invoke(page, "e2e_put_file", { path: "characters/intruder.toml", text: 'name = "Intruder"\n' });
  // A broken file: refused, nothing changed.
  await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke("e2e_stash_zip", { bytes: [1, 2, 3] }));
  await files(page, "e2e:stash");
  await page.getByRole("button", { name: "Restore from a file…" }).click();
  await dialog(page, "Restore this backup?").getByRole("button", { name: "Restore" }).click();
  await expect(page.locator(".data .err")).toContainText("Not a zip");
  // The listed one: what was here is backed up first.
  await saved(page).first().getByRole("button", { name: "Restore" }).click();
  await dialog(page, "Restore this backup?").getByRole("button", { name: "Restore" }).click();
  await expect(done(page)).toContainText("Restored the backup");
  await expect(saved(page)).toHaveCount(2);
  await expect(saved(page).filter({ hasText: "before-restore" })).toHaveCount(1);
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
  await expect(page.locator("button.card", { hasText: "Intruder" })).toHaveCount(0);
});

test("backups are made on schedule, keeping the last few", { tag: "@backup.auto" }, async ({ page }) => {
  await go(page, "Data", "Backups");
  await page.getByRole("combobox", { name: "Automatic backups" }).selectOption("day");
  for (let i = 0; i < 4; i++) await page.getByRole("button", { name: "Keep fewer" }).click();
  await expect(page.locator(".keep b")).toHaveText("1");
  // Days go by (the app checks at start): one is kept.
  for (const day of ["2099-01-01", "2099-01-03", "2099-01-05"]) await invoke(page, "e2e_backup_auto", { now: `${day} 10:00` });
  await go(page, "Compendium");
  await go(page, "Data", "Backups");
  await expect(saved(page)).toHaveCount(1);
  await expect(saved(page).first()).toContainText("2099-01-05");
});

test("backups go to the folder chosen, and back to the default one", { tag: "@backup.folder" }, async ({ page }) => {
  await go(page, "Data", "Backups");
  // A folder of its own each run (the app's cache outlives a reset).
  const folder = `elsewhere-${Date.now()}`;
  await page.evaluate((folder) => ((window as any).dntE2eFiles = { folder: `e2e:${folder}` }), folder);
  await page.getByRole("button", { name: "Change…" }).click();
  await expect(page.locator(".data .folder")).toContainText(folder);
  await expect(done(page)).toContainText(`Backups go to`);
  await page.getByRole("button", { name: "Back up now" }).click();
  await expect(saved(page)).toHaveCount(1);
  // The automatic one too (the first start only notes the time).
  for (const day of ["2099-01-01", "2099-02-01"]) await invoke(page, "e2e_backup_auto", { now: `${day} 10:00` });
  await go(page, "Compendium");
  await go(page, "Data", "Backups");
  await expect(saved(page)).toHaveCount(2);
  // Back to the default folder: they stay where they are.
  await page.getByRole("button", { name: "Default" }).click();
  await expect(page.locator(".data .folder")).not.toContainText(folder);
  await expect(saved(page)).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Default" })).toHaveCount(0);
});

test("a character goes with what they use, and turns their homebrew on", { tag: "@export.character" }, async ({ page }) => {
  await brewer(page);
  await openSheet(page, "Overview", "Brewer");
  await files(page, null);
  await sheetMore(page);
  await choose(page, "Brewer", "Export for dnt (to import elsewhere)…");
  await expect(status(page)).toContainText("Exported Brewer");
  // The homebrew off again, then the import: here already, replaced; on again.
  await invoke(page, "set_campaign_sources", { slug: "e2e", sources: [], homebrew: [], showReprinted: false });
  await importSaved(page, "Brewer.dnt.zip");
  await dialog(page, "Import Brewer").getByRole("button", { name: "Replace" }).click();
  await expect(done(page)).toContainText("Imported Brewer, 1 homebrew source");
  // The feat shows only with its homebrew on.
  await openSheet(page, "Features", "Brewer");
  await expect(page.locator(".feature", { hasText: "Brewed Luck" })).toBeVisible();
});

test("a campaign is exported and imported", { tag: "@export.campaign" }, async ({ page }) => {
  await files(page, null);
  await go(page, "Campaigns");
  await campaignAction(page, CAMPAIGN, "Export");
  await expect(page.locator(".campaigns p.ok, p.ok").first()).toContainText("Exported the campaign");
  await importSaved(page, `${CAMPAIGN}.dnt.zip`);
  await dialog(page, `Import ${CAMPAIGN}`).getByRole("button", { name: "Keep both" }).click();
  await expect(done(page)).toContainText(`Imported the campaign ${CAMPAIGN} (2)`);
  await go(page, "Campaigns");
  await expect(page.locator(".card", { hasText: `${CAMPAIGN} (2)` })).toBeVisible();
});

test("a character here already is replaced, or both kept", { tag: "@import.clash" }, async ({ page }) => {
  await openSheet(page, "Overview", "Tester");
  await files(page, null);
  await sheetMore(page);
  await choose(page, "Tester", "Export for dnt (to import elsewhere)…");
  await expect(status(page)).toContainText("Exported Tester");
  await importSaved(page, "Tester.dnt.zip");
  const ask = dialog(page, "Import Tester");
  await expect(ask).toContainText("Tester is here already");
  await ask.getByRole("button", { name: "Keep both" }).click();
  await expect(done(page)).toContainText("Imported Tester (2)");
  await importSaved(page, "Tester.dnt.zip");
  await dialog(page, "Import Tester").getByRole("button", { name: "Replace" }).click();
  await expect(done(page)).toContainText("Imported Tester");
  // The list, not the open sheet.
  await invoke(page, "close_character");
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toHaveCount(2);
});
