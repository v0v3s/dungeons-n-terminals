// The app's screens for the wiki, from the test party: a phone's portrait
// screen, or a desktop window (DNT_E2E_TARGET=windows).
import type { Page } from "@playwright/test";
import { fileURLToPath } from "node:url";
import { choose, expect, go, invoke, openSheet, pick, sheetTab, test } from "../e2e/fixtures";

const OUT = fileURLToPath(new URL("../../docs/wiki/images/", import.meta.url));

/// The screen as it is, once its transitions are done.
async function shot(page: Page, name: string) {
  await page.waitForTimeout(500);
  await page.screenshot({ path: `${OUT}${name}.png` });
}

test.describe("a new app", () => {
  test.use({ seed: "empty" });
  test("getting started", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Import a zip…" })).toBeVisible();
    await shot(page, "app-welcome");
  });
});

test("the screens", async ({ page }) => {
  await invoke(page, "note_create", { folder: "NPCs", title: "Old Miller" });
  await invoke(page, "note_create", { folder: "Places", title: "The Mill" });
  await invoke(page, "note_create", { folder: "Quests", title: "The Missing Grain" });

  await openSheet(page);
  await shot(page, "app-sheet");
  await page.locator("button.skill", { hasText: "Arcana" }).click();
  await expect(page.locator(".status.roll")).toBeVisible();
  await shot(page, "app-overview");

  await sheetTab(page, "Spells");
  for (const name of ["Frost Needle", "Glimmer", "Ember Bolt", "Ward of Thorns"]) {
    await page.getByRole("button", { name: "+ add" }).first().click();
    await pick(page, [name]);
    await expect(page.locator("button.spell", { hasText: name })).toBeVisible();
  }
  await shot(page, "app-spells");

  await sheetTab(page, "Inventory");
  await page.locator("button.line", { hasText: "Scribe's Pack" }).click();
  await choose(page, "Scribe's Pack", "Unpack");
  await shot(page, "app-inventory");

  await invoke(page, "close_character");
  await page.reload();
  await go(page, "Compendium");
  await shot(page, "app-compendium");

  await go(page, "Notes");
  await shot(page, "app-notes");

  await go(page, "Campaigns");
  await shot(page, "app-campaigns");

  await go(page, "Characters");
  await page.getByRole("button", { name: "+ New character" }).click();
  const name = page.getByRole("dialog", { name: "New character" }).getByRole("textbox");
  await name.fill("Nova");
  await name.press("Enter");
  await expect(page.locator(".creation")).toContainText("Choose a species");
  await shot(page, "app-creation");
  await invoke(page, "creation_cancel");
});
