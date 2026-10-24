import type { Page } from "@playwright/test";
import { dialog, expect, openSheet, pick, test } from "./fixtures";

/// Add spells to the first class on the Spells tab, one at a time.
async function addSpells(page: Page, names: string[]) {
  for (const name of names) {
    await page.getByRole("button", { name: "+ add" }).first().click();
    await pick(page, [name]);
    await expect(page.locator("button.spell", { hasText: name })).toBeVisible();
  }
}

test("a new mage adds cantrips and spells to the grimoire", { tag: "@spells.add" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle", "Spark", "Glimmer"]);
  await expect(page.locator("button.spell")).toHaveCount(3);
  await addSpells(page, ["Ember Bolt"]);
  await expect(page.locator("button.spell", { hasText: "Ember Bolt" })).toBeVisible();
});

test("a cantrip rolls with the mage's numbers", { tag: "@spells.roll" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle"]);
  await page.locator("button.spell", { hasText: "Frost Needle" }).click();
  await dialog(page, "Frost Needle").getByRole("button", { name: "Roll it" }).click();
  await expect(page.locator(".status.roll")).toContainText("Frost Needle");
});
