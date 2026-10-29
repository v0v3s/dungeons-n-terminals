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
  await step(page, "Choose a species", "Human");
  await step(page, "Choose a background", "Sage");
  // The scores start as the standard array: raising one swaps it with the next value.
  await expect(ask(page).getByRole("button", { name: "Standard array" })).toHaveAttribute("aria-pressed", "true");
  await expect(value(page, "Intelligence")).toHaveText("12");
  await raise(page, "Intelligence");
  await expect(value(page, "Intelligence")).toHaveText("13");
  await expect(value(page, "Constitution")).toHaveText("12");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "Background (Sage): ability increases", "+2 / +1");
  await step(page, "+2 to which ability?", "Intelligence");
  await step(page, "+1 to which ability?", "Wisdom");
  // The Human's Versatile: an origin feat.
  await step(page, "Choose a feat", "Alert");
  await step(page, "Choose your starting class", "Wizard");
  // Arcana and History come with the Sage, so the Wizard does not offer them.
  await step(page, "Wizard: choose 2 skill(s)", "Investigation", "Nature");
  await step(page, "Human: choose 1 skill(s)", "Stealth");
  await step(page, "Class (Wizard): starting equipment", "A:");
  await step(page, "Background (Sage): starting equipment", "B:");
  await create(page);

  await expect(page.locator(".sheet h1")).toContainText("Nova");
  await expect(identity(page)).toContainText("Human · Sage · Wizard 1");
  // Intelligence 13 + 2.
  await sheetTab(page, "Skills");
  await expect(page.locator('.abil .score[data-of="Intelligence"]')).toHaveText("15");
  await expect(page.locator("button.skill", { hasText: "Stealth" }).locator(".mark")).toHaveClass(/on/);
  await expect(page.locator("button.skill", { hasText: "Survival" }).locator(".mark")).not.toHaveClass(/on/);
  await tab(page, "Features");
  await expect(page.locator(".feature", { hasText: "Alert" })).toBeVisible();
  await expect(page.locator(".feature", { hasText: "Magic Initiate; Wizard" })).toBeVisible();
  await tab(page, "Inventory");
  await expect(page.locator("button.line", { hasText: "Scholar’s Pack" })).toBeVisible();

  await invoke(page, "close_character");
  await page.reload();
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Nova" })).toContainText("Wizard 1");
});

test(
  "a lineage, a subclass at level 1 and a holy symbol are chosen",
  {
    tag: ["@creation.lineage", "@creation.subclass", "@creation.equipment-pick"],
  },
  async ({ page }) => {
    await start(page, "Ember");
    await step(page, "Choose a species", "Tiefling");
    await step(page, "Tiefling: choose a lineage", "Tiefling; Chthonic Legacy");
    await step(page, "Choose a background", "Acolyte");
    await ask(page).getByRole("button", { name: "Next" }).click();
    await step(page, "ability increases", "Skip");
    // The Hexer (the test supplement) chooses its subclass at level 1.
    await step(page, "Choose your starting class", "Hexer");
    await step(page, "Choose a subclass", "The Tide");
    await step(page, "Hexer: choose 2 skill(s)", "Arcana", "Deception");
    await step(page, "Class (Hexer): starting equipment", "A:");
    await step(page, "Background (Acolyte): starting equipment", "A:");
    await step(page, "Choose a holy symbol", "Reliquary");
    await create(page);

    await expect(identity(page)).toContainText("Tiefling; Chthonic Legacy · Acolyte · Hexer 1");
    await tab(page, "Features");
    await expect(page.locator(".feature", { hasText: "The Tide" }).first()).toBeVisible();
    await tab(page, "Inventory");
    await expect(page.locator("button.line", { hasText: "Reliquary" })).toBeVisible();
    await expect(page.locator("button.line", { hasText: "Amulet" })).toHaveCount(0);
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
  "a class feature's option, weapon masteries and a gaming set are chosen at level 1",
  { tag: ["@creation.feature-choice", "@creation.equipment-pick"] },
  async ({ page }) => {
    await start(page, "Scout");
    await step(page, "Choose a species", "Tiefling");
    await step(page, "Tiefling: choose a lineage", "Tiefling; Abyssal Legacy");
    await step(page, "Choose a background", "Soldier");
    await ask(page).getByRole("button", { name: "Next" }).click();
    await step(page, "ability increases", "Skip");
    await step(page, "Choose your starting class", "Fighter");
    // Athletics and Intimidation come with the Soldier.
    await step(page, "Fighter: choose 2 skill(s)", "Acrobatics", "Perception");
    await step(page, "Fighting Style: choose one", "Two-Weapon Fighting");
    await step(page, "Weapon Mastery: choose 3", "Flail", "Greatsword", "Longbow");
    await step(page, "Class (Fighter): starting equipment", "A:");
    await step(page, "Background (Soldier): starting equipment", "A:");
    await step(page, "Choose a gaming set", "Playing Cards");
    await create(page);

    await expect(identity(page)).toContainText("Fighter 1");
    await tab(page, "Features");
    await expect(page.locator(".feature", { hasText: "Two-Weapon Fighting" })).toBeVisible();
    await expect(page.locator(".feature", { hasText: "Archery" })).toHaveCount(0);
    const masteries = page
      .locator("section.pane", {
        has: page.locator(".title", { hasText: "Weapon Mastery" }),
      })
      .last();
    await expect(masteries).toContainText("Flail");
    await expect(masteries).toContainText("Greatsword");
    await expect(masteries).toContainText("Longbow");
    await tab(page, "Inventory");
    await expect(page.locator("button.line", { hasText: "Playing Cards" })).toBeVisible();
    await expect(page.locator("button.line", { hasText: "Dice" })).toHaveCount(0);
  },
);

test("ability scores are bought with 27 points", { tag: "@creation.point-buy" }, async ({ page }) => {
  await start(page, "Thrifty");
  await step(page, "Choose a species", "Tiefling");
  await step(page, "Tiefling: choose a lineage", "Tiefling; Abyssal Legacy");
  await step(page, "Choose a background", "Acolyte");
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
  await step(page, "Choose a species", "Human");
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
  await step(page, "Choose a species", "Human");
  await page.locator(".creation").getByRole("button", { name: "Cancel", exact: true }).click();
  await expect(page.locator(".creation")).toHaveCount(0);
  await expect(page.locator("button.card", { hasText: "Quitter" })).toHaveCount(0);
  expect(await invoke(page, "creation_current")).toBeNull();
});

const hint = (page: Page, label: string) => ask(page).locator("li button", { has: page.locator(".label", { hasText: new RegExp(`^${escape(label)}$`) }) }).locator(".hint");

test("each choice says what it gives", { tag: "@creation.row-hints" }, async ({ page }) => {
  await start(page, "Reader");
  await expect(hint(page, "Human")).toHaveText("speed 30 ft · any 1 skill · SRD52");
  await expect(hint(page, "Elf")).toHaveText("speed 30 ft · darkvision 60 ft · 1 of Insight, Perception, Survival · SRD52");
  await step(page, "Choose a species", "Human");
  await expect(hint(page, "Sage")).toHaveText("Arcana, History · +2/+1 CON/INT/WIS · Magic Initiate; Wizard · SRD52");
  await step(page, "Choose a background", "Sage");
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "ability increases", "Skip");
  await step(page, "Choose a feat", "Alert");
  await expect(ask(page).locator(".title")).toContainText("Choose your starting class");
  await expect(hint(page, "Ranger")).toHaveText("d10 · DEX & WIS · saves STR, DEX · SRD52");
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
  await step(page, "Choose a species", "Human");
  await step(page, "Choose a background", "Sage");
  const list = await checklist(page);
  await expect(item(list, "Species")).toContainText("Human");
  await expect(item(list, "Background")).toContainText("Sage");
  await expect(item(list, "Ability scores")).toHaveClass(/current/);
  await expect(item(list, "Class")).not.toHaveClass(/done|current/);

  // The review is the whole checklist.
  await ask(page).getByRole("button", { name: "Next" }).click();
  await step(page, "ability increases", "Skip");
  await step(page, "Choose a feat", "Alert");
  await step(page, "Choose your starting class", "Wizard");
  await step(page, "Wizard: choose 2 skill(s)", "Investigation", "Nature");
  await step(page, "Human: choose 1 skill(s)", "Stealth");
  await step(page, "Class (Wizard): starting equipment", "A:");
  await step(page, "Background (Sage): starting equipment", "B:");
  await expect(ask(page)).toContainText("Everything is chosen");
  const review = await checklist(page);
  await expect(item(review, "Class")).toContainText("Wizard 1");
  await expect(item(review, "Skills & features")).toContainText("Investigation, Nature");
  await expect(item(review, "Equipment")).toContainText("Class (Wizard)");
});

test("a finished step is chosen again from the checklist", { tag: "@creation.reopen" }, async ({ page }) => {
  await start(page, "Changer");
  await step(page, "Choose a species", "Human");
  await step(page, "Choose a background", "Sage");
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
  await ask(page).getByRole("button", { name: "About Tiefling" }).click();
  const about = page.getByRole("dialog", { name: "Tiefling" });
  await expect(about).toContainText("You are the recipient of a legacy");
  await about.getByRole("button", { name: "Choose Tiefling" }).click();
  await expect(about).toHaveCount(0);
  await expect(ask(page).locator(".title")).toContainText("Tiefling: choose a lineage");
  // Choices without a text to read have no ?.
  await step(page, "Tiefling: choose a lineage", "Tiefling; Abyssal Legacy");
  await step(page, "Choose a background", "Sage");
  await expect(ask(page).getByRole("button", { name: /^About / })).toHaveCount(0);
});
