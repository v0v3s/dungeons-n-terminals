import type { Locator, Page } from "@playwright/test";
import { answer, choose, dialog, expect, go, mira, openSheet, sheetTab, status, test } from "./fixtures";

/// A companion's pane, by its name.
const card = (page: Page, name: string): Locator =>
  page.locator(".companion").filter({ has: page.locator("button.fold, .title", { hasText: name }) });
const roll = (page: Page) => status(page).and(page.locator(".roll"));

/// A creature in the picker, by its exact name ("Wolf", not "Dire Wolf").
const offered = (page: Page, name: string) =>
  dialog(page, "Which creature?").locator("li button").filter({ has: page.locator(".label", { hasText: new RegExp(`^${name}$`) }) });

async function creature(page: Page, name: string) {
  const filter = dialog(page, "Which creature?").getByRole("textbox");
  if (await filter.count()) await filter.fill(name);
  await offered(page, name).click();
}

async function menu(page: Page, name: string, action: string) {
  await card(page, name).getByRole("button", { name: `More for ${name}` }).click();
  await choose(page, name, action);
}

/// Cast a spell from the Spells tab with its cast button, the way picked
/// when it asks.
async function cast(page: Page, spell: string, way: string | null) {
  await openSheet(page, "Spells", "Mira");
  await page.getByRole("button", { name: `Cast ${spell}` }).click();
  if (way) await dialog(page, `Cast ${spell}`).locator("li button", { hasText: way }).click();
}

test("casting a spell gives a familiar with a card of its own that rolls", { tag: "@companions.familiar" }, async ({ page }) => {
  await mira(page);
  await cast(page, "Find Familiar", "As a ritual");
  // Only what the rules allow: the Owl, not the Wolf (CR 1/4).
  await expect(offered(page, "Owl")).toHaveCount(1);
  await expect(offered(page, "Wolf")).toHaveCount(0);
  await creature(page, "Owl");
  await choose(page, "Familiar: which type?", "Fey");

  await sheetTab(page, "Companions");
  const owl = card(page, "Owl");
  await expect(owl).toContainText("Familiar");
  await expect(owl).toContainText("Tiny Fey");
  await expect(owl.locator(".hp")).toContainText("1/1");
  await expect(owl).toContainText("AC 11");
  await expect(owl).toContainText("Fly 60 ft.");
  await expect(owl).toContainText("Passive Perception 15");
  await expect(owl.locator("button.chip", { hasText: "Perception" })).toContainText("+5");

  await menu(page, "Owl", "Rename…");
  await answer(page, "Rename Owl", "Pip");
  const pip = card(page, "Pip");
  await pip.locator("button.chip", { hasText: "Perception" }).click();
  await expect(roll(page)).toContainText("Pip: Perception");

  // One familiar at a time.
  await cast(page, "Find Familiar", "As a ritual");
  await creature(page, "Owl");
  await choose(page, "Familiar: which type?", "Fey");
  await expect(status(page)).toContainText("Find Familiar gives 1 at once");
});

test("a steed cast with a higher slot follows the slot's level and its owner", { tag: "@companions.summon" }, async ({ page }) => {
  await mira(page);
  await cast(page, "Call Steed", "Level 4 slot");
  await choose(page, "Steed: which type?", "Fey");

  await sheetTab(page, "Companions");
  const steed = card(page, "Otherworldly Steed");
  await expect(steed.locator(".hp")).toContainText("45/45");
  await expect(steed).toContainText("AC 14");
  await expect(steed).toContainText("Fly 60 ft.");
  const slam = steed.locator("button.act", { hasText: "Otherworldly Slam (Fey)" });
  await expect(slam).toContainText("+6");
  // Short on a phone (psych.), whole on a computer.
  await expect(slam).toContainText(/1d8 \+ 4 psych/);
  await expect(steed.locator("button.act", { hasText: "Otherworldly Slam" })).toHaveCount(1);

  await slam.click();
  await expect(roll(page)).toContainText("Otherworldly Steed: Otherworldly Slam (Fey)");
  await expect(roll(page)).toContainText("psychic damage");
});

test("any creature joins as a follower", { tag: "@companions.follower" }, async ({ page }) => {
  test.setTimeout(90_000);
  await mira(page);
  await openSheet(page, "Companions", "Mira");
  await page.getByRole("button", { name: "Add a companion" }).click();
  await choose(page, "Add a companion", "A follower…");
  await creature(page, "Wolf");

  const wolf = card(page, "Wolf");
  await expect(wolf).toContainText("Follower");
  await expect(wolf.locator(".hp")).toContainText("11/11");
  const bite = wolf.locator("button.act", { hasText: "Bite" });
  await expect(bite).toContainText("+4");
  await expect(bite).toContainText(/1d6 \+ 2 pierc/);
  await bite.click();
  await expect(roll(page)).toContainText("Wolf: Bite");
  await expect(roll(page)).toContainText("piercing damage");

  await wolf.getByRole("button", { name: "Damage" }).click();
  await answer(page, "Damage to Wolf", "5");
  await expect(wolf.locator(".hp")).toContainText("6/11");
  await wolf.getByRole("button", { name: "Heal" }).click();
  await answer(page, "Heal Wolf", "3");
  await expect(wolf.locator(".hp")).toContainText("9/11");

  await menu(page, "Wolf", "Rename…");
  await answer(page, "Rename Wolf", "Grey");
  const grey = card(page, "Grey");
  await menu(page, "Grey", "Conditions…");
  await choose(page, "Grey: conditions", "Prone");
  await expect(grey.locator(".cond")).toHaveText(["Prone"]);
  await menu(page, "Grey", "Away (dismissed)");
  await expect(grey).toContainText("away");
  await menu(page, "Grey", "Back");
  await expect(grey).not.toContainText("away");
  await menu(page, "Grey", "Remove…");
  await dialog(page, "Remove Grey?").getByRole("button", { name: "Remove" }).click();
  await expect(card(page, "Grey")).toHaveCount(0);

  // One of your own: a homebrew creature from the template.
  await go(page, "Compendium");
  await page.getByRole("button", { name: "New homebrew entry" }).or(page.getByRole("button", { name: "+ new" })).filter({ visible: true }).first().click();
  await choose(page, "New homebrew entry", "Creature");
  await answer(page, "New creature", "Hireling");
  await dialog(page, "Edit · Hireling").getByRole("button", { name: "Done" }).click();
  await openSheet(page, "Companions", "Mira");
  await page.getByRole("button", { name: "Add a companion" }).click();
  await choose(page, "Add a companion", "A follower…");
  await creature(page, "Hireling");
  const hireling = card(page, "Hireling");
  await expect(hireling.locator(".hp")).toContainText("11/11");
  await expect(hireling.locator("button.act", { hasText: "Strike" })).toContainText("+3");
});

test("on a phone a companion is the Character tab, smaller", { tag: ["@companions.phone", "@android"] }, async ({ page }) => {
  await mira(page);
  await openSheet(page, "Companions", "Mira");
  await page.getByRole("button", { name: "Add a companion" }).click();
  await choose(page, "Add a companion", "A follower…");
  await creature(page, "Wolf");
  const wolf = card(page, "Wolf");
  await expect(wolf).toHaveClass(/\bsection\b/);
  // Damage and Heal on its title line; the numbers as boxes.
  await expect(wolf.locator(".aside").getByRole("button", { name: "Damage" })).toBeVisible();
  await expect(wolf.locator(".tile", { hasText: "AC" })).toContainText("12");
  await expect(wolf.locator(".abil6 .tile")).toHaveCount(6);
  // The attack: its roll at the right of its row.
  const bite = wolf.locator("button.act", { hasText: "Bite" });
  const [row, box] = [(await bite.boundingBox())!, (await bite.locator(".rollbox").boundingBox())!];
  expect(row.x + row.width - (box.x + box.width)).toBeLessThan(4);
});
