import type { Page } from "@playwright/test";
import { answer, expect, go, invoke, sheetTab, test } from "./fixtures";

const ask = (page: Page) => page.locator(".creation .ask");
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/// Start a new character from the roster.
async function start(page: Page, name: string) {
  await go(page, "Characters");
  await page.getByRole("button", { name: "+ New character" }).click();
  await answer(page, "New character", name);
  await expect(ask(page).locator(".title")).toContainText("Choose a species");
}

/// Answer the question titled `title` with these choices (several for a
/// multiple pick, which goes on by itself once all are chosen).
async function step(page: Page, title: string, ...labels: string[]) {
  await expect(ask(page).locator(".title")).toContainText(title);
  for (const label of labels) {
    // A long list has a filter.
    const filter = ask(page).getByPlaceholder("type to filter");
    if (await filter.count()) await filter.fill(label);
    await ask(page)
      .locator("li button", {
        has: page.locator(".label", {
          hasText: new RegExp(`^${escape(label)}`),
        }),
      })
      .first()
      .click();
  }
}

const row = (page: Page, ability: string) => ask(page).locator(".score", { hasText: ability });
const value = (page: Page, ability: string) => row(page, ability).locator(".value");
async function raise(page: Page, ability: string, times = 1) {
  for (let i = 0; i < times; i++) await row(page, ability).getByRole("button", { name: "Raise" }).click();
}

async function create(page: Page) {
  await expect(ask(page)).toContainText("Everything is chosen");
  await ask(page).getByRole("button", { name: "Create the character" }).click();
}

const identity = (page: Page) => page.locator(".sheet .id");
const tab = (page: Page, name: "Features" | "Inventory") => sheetTab(page, name);

test("a character is made step by step", { tag: "@creation.full" }, async ({ page }) => {
  await start(page, "Nova");
  await step(page, "Choose a species", "Kindred");
  await step(page, "Choose a background", "Archivist");
  // The scores start as the standard array: raising one swaps it with the next value.
  await expect(ask(page).getByRole("button", { name: "Standard array" })).toHaveAttribute("aria-pressed", "true");
  await expect(value(page, "Intelligence")).toHaveText("12");
  await raise(page, "Intelligence");
  await expect(value(page, "Intelligence")).toHaveText("13");
  await expect(value(page, "Constitution")).toHaveText("12");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "Background (Archivist): ability increases", "+2 / +1");
  await step(page, "+2 to which ability?", "Intelligence");
  await step(page, "+1 to which ability?", "Wisdom");
  await step(page, "Choose a feat", "Lucky Charm");
  await step(page, "Choose your starting class", "Mage");
  await step(page, "Mage: choose 2 skill(s)", "Arcana", "Nature");
  await step(page, "Kindred: choose 1 skill(s)", "Stealth");
  await step(page, "Class (Mage): starting equipment", "A:");
  await step(page, "Background (Archivist): starting equipment", "B:");
  await create(page);

  await expect(page.locator(".sheet h1")).toContainText("Nova");
  await expect(identity(page)).toContainText("Kindred · Archivist · Mage 1");
  // Intelligence 13 + 2.
  await sheetTab(page, "Skills");
  await expect(page.locator('.abil .score[data-of="Intelligence"]')).toHaveText("15");
  await expect(page.locator("button.skill", { hasText: "Stealth" }).locator(".mark")).toHaveClass(/on/);
  await expect(page.locator("button.skill", { hasText: "Survival" }).locator(".mark")).not.toHaveClass(/on/);
  await tab(page, "Features");
  await expect(page.locator(".feature", { hasText: "Lucky Charm" })).toBeVisible();
  await expect(page.locator(".feature", { hasText: "Quick Wits" })).toBeVisible();
  await tab(page, "Inventory");
  await expect(page.locator("button.line", { hasText: "Scribe's Pack" })).toBeVisible();

  await invoke(page, "close_character");
  await page.reload();
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Nova" })).toContainText("Mage 1");
});

test(
  "a lineage, a subclass at level 1 and an arcane focus are chosen",
  {
    tag: ["@creation.lineage", "@creation.subclass", "@creation.equipment-pick"],
  },
  async ({ page }) => {
    await start(page, "Ember");
    await step(page, "Choose a species", "Emberkin");
    await step(page, "Emberkin: choose a lineage", "Emberkin; Cinder Lineage");
    await step(page, "Choose a background", "Archivist");
    await ask(page).getByRole("button", { name: "Next" }).click();
    await step(page, "ability increases", "Skip");
    await step(page, "Choose your starting class", "Hexer");
    await step(page, "Choose a subclass", "The Tide");
    // History comes with the Archivist, so the Hexer does not offer it.
    await step(page, "Hexer: choose 2 skill(s)", "Arcana", "Nature");
    await step(page, "Class (Hexer): starting equipment", "A: an arcane focus");
    await step(page, "Choose an arcane focus", "Crystal Orb");
    await step(page, "Background (Archivist): starting equipment", "B:");
    await create(page);

    await expect(identity(page)).toContainText("Emberkin; Cinder Lineage · Archivist · Hexer 1");
    await tab(page, "Features");
    await expect(page.locator(".feature", { hasText: "The Tide" }).first()).toBeVisible();
    await tab(page, "Inventory");
    await expect(page.locator("button.line", { hasText: "Crystal Orb" })).toBeVisible();
    await expect(page.locator("button.line", { hasText: "Hex Pouch" })).toHaveCount(0);
  },
);

test("a subspecies is chosen after the species", { tag: "@creation.lineage" }, async ({ page }) => {
  await start(page, "Pebble");
  await step(page, "Choose a species", "Stoneling");
  await step(page, "Stoneling: choose a subrace", "Stoneling (Hill)");
  await expect(ask(page).locator(".title")).toContainText("Choose a background");
  const now = await invoke<{ summary: [string, string][] }>(page, "creation_current");
  expect(now.summary).toContainEqual(["Subrace", "Stoneling (Hill)"]);
});

test(
  "a class feature's option, weapon masteries and an instrument are chosen at level 1",
  { tag: ["@creation.feature-choice", "@creation.equipment-pick"] },
  async ({ page }) => {
    await start(page, "Scout");
    await step(page, "Choose a species", "Emberkin");
    await step(page, "Emberkin: choose a lineage", "Emberkin; Ash Lineage");
    await step(page, "Choose a background", "Drifter");
    await ask(page).getByRole("button", { name: "Next" }).click();
    await step(page, "ability increases", "Skip");
    await step(page, "Choose your starting class", "Warden");
    // The Drifter's choice comes after: it offers what the Warden left.
    await step(page, "Warden: choose 3 skill(s)", "Athletics", "Nature", "Perception");
    await step(page, "Drifter: choose 2 skill(s)", "Stealth", "Survival");
    await step(page, "Favored Ground: choose one", "Mountain Ground");
    await step(page, "Weapon Mastery: choose 2", "Longfang", "Thunderbow");
    await step(page, "Class (Warden): starting equipment", "A:");
    await step(page, "Background (Drifter): starting equipment", "A:");
    await step(page, "Choose a musical instrument", "Hand Drum");
    await create(page);

    await expect(identity(page)).toContainText("Warden 1");
    await tab(page, "Features");
    await expect(page.locator(".feature", { hasText: "Mountain Ground" })).toBeVisible();
    await expect(page.locator(".feature", { hasText: "Forest Ground" })).toHaveCount(0);
    const masteries = page
      .locator("section.pane", {
        has: page.locator(".title", { hasText: "Weapon Mastery" }),
      })
      .last();
    await expect(masteries).toContainText("Longfang");
    await expect(masteries).toContainText("Thunderbow");
    await tab(page, "Inventory");
    await expect(page.locator("button.line", { hasText: "Hand Drum" })).toBeVisible();
    await expect(page.locator("button.line", { hasText: "Reed Pipe" })).toHaveCount(0);
  },
);

test("ability scores are bought with 27 points", { tag: "@creation.point-buy" }, async ({ page }) => {
  await start(page, "Thrifty");
  await step(page, "Choose a species", "Emberkin");
  await step(page, "Emberkin: choose a lineage", "Emberkin; Ash Lineage");
  await step(page, "Choose a background", "Archivist");
  // The method changes on the same screen.
  await expect(value(page, "Strength")).toHaveText("15");
  await ask(page).getByRole("button", { name: "Point buy" }).click();
  await expect(ask(page).getByRole("button", { name: "Point buy" })).toHaveAttribute("aria-pressed", "true");
  const left = ask(page).locator(".budget");
  await expect(left).toContainText("27 / 27");
  await expect(value(page, "Strength")).toHaveText("8");
  await raise(page, "Strength", 7);
  await expect(value(page, "Strength")).toHaveText("15");
  await expect(left).toContainText("18 / 27");
  // 15 is the most a point buy goes to.
  await raise(page, "Strength");
  await expect(value(page, "Strength")).toHaveText("15");
  await raise(page, "Dexterity", 7);
  await raise(page, "Constitution", 7);
  await expect(left).toContainText("0 / 27");
  // No points left: nothing more is bought.
  await raise(page, "Intelligence");
  await expect(value(page, "Intelligence")).toHaveText("8");
  await row(page, "Constitution").getByRole("button", { name: "Lower" }).click();
  await expect(left).toContainText("2 / 27");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await expect(ask(page).locator(".title")).toContainText("ability increases");
  const now = await invoke<{ summary: [string, string][] }>(page, "creation_current");
  expect(now.summary).toContainEqual(["Abilities", "STR 15 DEX 15 CON 14 INT 8 WIS 8 CHA 8 (Point buy)"]);
});

test("going back changes an answer; going back from the start cancels", { tag: "@creation.back" }, async ({ page }) => {
  await start(page, "Waverer");
  await step(page, "Choose a species", "Kindred");
  await expect(ask(page).locator(".title")).toContainText("Choose a background");
  await page.getByRole("button", { name: "‹ Back" }).click();
  await step(page, "Choose a species", "Stoneling");
  await expect(ask(page).locator(".title")).toContainText("Stoneling: choose a subrace");
  const now = await invoke<{ summary: [string, string][] }>(page, "creation_current");
  expect(now.summary).toContainEqual(["Species", "Stoneling"]);

  await page.getByRole("button", { name: "‹ Back" }).click();
  await expect(ask(page).locator(".title")).toContainText("Choose a species");
  await page.getByRole("button", { name: "‹ Back" }).click();
  await expect(page.locator(".creation")).toHaveCount(0);
  await expect(page.locator("button.card")).toHaveCount(4);
  await expect(page.locator("button.card", { hasText: "Waverer" })).toHaveCount(0);

  // Cancel, from any step.
  await page.getByRole("button", { name: "+ New character" }).click();
  await answer(page, "New character", "Quitter");
  await step(page, "Choose a species", "Kindred");
  await page.locator(".creation").getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator(".creation")).toHaveCount(0);
  await expect(page.locator("button.card", { hasText: "Quitter" })).toHaveCount(0);
  expect(await invoke(page, "creation_current")).toBeNull();
});

const hint = (page: Page, label: string) => ask(page).locator("li button", { has: page.locator(".label", { hasText: new RegExp(`^${escape(label)}$`) }) }).locator(".hint");

test("each choice says what it gives", { tag: "@creation.row-hints" }, async ({ page }) => {
  await start(page, "Reader");
  await expect(hint(page, "Kindred")).toHaveText("speed 30 ft · any 1 skill · TST");
  await step(page, "Choose a species", "Kindred");
  await expect(hint(page, "Archivist")).toHaveText("History, Investigation · +2/+1 INT/WIS/CHA · Quick Wits · TST");
  await step(page, "Choose a background", "Archivist");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "ability increases", "Skip");
  await step(page, "Choose a feat", "Lucky Charm");
  await expect(ask(page).locator(".title")).toContainText("Choose your starting class");
  await expect(hint(page, "Warden")).toHaveText("d10 · DEX & WIS · saves STR, DEX · TST");
});

/// The checklist: beside the question, or folded above it on a phone.
async function checklist(page: Page) {
  if (await page.locator(".creation .steps").isVisible()) return page.locator(".creation .steps .checklist");
  const where = page.locator(".creation .where");
  if ((await where.count()) && (await where.getAttribute("aria-expanded")) !== "true") await where.click();
  return ask(page).locator(".checklist");
}
const item = (list: Awaited<ReturnType<typeof checklist>>, label: string) => list.locator("li", { has: list.page().locator(".slabel", { hasText: new RegExp(`^${escape(label)}$`) }) });

test("the steps are a checklist of what was chosen", { tag: "@creation.checklist" }, async ({ page }) => {
  await start(page, "Lister");
  await step(page, "Choose a species", "Kindred");
  await step(page, "Choose a background", "Archivist");
  const list = await checklist(page);
  await expect(item(list, "Species")).toContainText("Kindred");
  await expect(item(list, "Background")).toContainText("Archivist");
  await expect(item(list, "Ability scores")).toHaveClass(/current/);
  await expect(item(list, "Class")).not.toHaveClass(/done|current/);

  // The review is the whole checklist.
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "ability increases", "Skip");
  await step(page, "Choose a feat", "Lucky Charm");
  await step(page, "Choose your starting class", "Mage");
  await step(page, "Mage: choose 2 skill(s)", "Arcana", "Nature");
  await step(page, "Kindred: choose 1 skill(s)", "Stealth");
  await step(page, "Class (Mage): starting equipment", "A:");
  await step(page, "Background (Archivist): starting equipment", "B:");
  await expect(ask(page)).toContainText("Everything is chosen");
  const review = await checklist(page);
  await expect(item(review, "Class")).toContainText("Mage 1");
  await expect(item(review, "Skills & features")).toContainText("Arcana, Nature");
  await expect(item(review, "Equipment")).toContainText("Class (Mage)");
});

test("a finished step is chosen again from the checklist", { tag: "@creation.reopen" }, async ({ page }) => {
  await start(page, "Changer");
  await step(page, "Choose a species", "Kindred");
  await step(page, "Choose a background", "Archivist");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "ability increases", "Skip");
  await expect(ask(page).locator(".title")).toContainText("Choose a feat");

  // The species again: what came after is undone.
  let list = await checklist(page);
  await item(list, "Species").getByRole("button").click();
  await expect(ask(page).locator(".title")).toContainText("Choose a species");
  await step(page, "Choose a species", "Stoneling");
  await step(page, "Stoneling: choose a subrace", "Stoneling (Hill)");
  await expect(ask(page).locator(".title")).toContainText("Choose a background");
  const now = await invoke<{ summary: [string, string][] }>(page, "creation_current");
  expect(now.summary).toEqual([
    ["Species", "Stoneling"],
    ["Subrace", "Stoneling (Hill)"],
  ]);
  // Steps not reached yet cannot be opened.
  list = await checklist(page);
  await expect(item(list, "Class").getByRole("button")).toHaveCount(0);
});

test("on a phone a choice's text is read from its ?, and chosen there", { tag: ["@creation.details", "@android"] }, async ({ page }) => {
  await start(page, "Curious");
  await ask(page).getByRole("button", { name: "About Emberkin" }).click();
  const about = page.getByRole("dialog", { name: "Emberkin" });
  await expect(about).toContainText("You resist fire");
  await about.getByRole("button", { name: "Choose Emberkin" }).click();
  await expect(about).toHaveCount(0);
  await expect(ask(page).locator(".title")).toContainText("Emberkin: choose a lineage");
  // Choices without a text to read have no ?.
  await step(page, "Emberkin: choose a lineage", "Emberkin; Ash Lineage");
  await step(page, "Choose a background", "Archivist");
  await expect(ask(page).getByRole("button", { name: /^About / })).toHaveCount(0);
});
