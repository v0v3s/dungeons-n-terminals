import type { Page } from "@playwright/test";
import { choose, dialog, expect, invoke, openSheet, pick, sheetMore, sheetTab, status, test } from "./fixtures";

/// Choose an action from the sheet's ⋯ menu.
async function more(page: Page, name: string, action: string) {
  await sheetMore(page);
  await choose(page, name, action);
}

/// Level a character up `n` times before the test (the first choices,
/// average hit points), as a state to start from.
async function seedLevels(page: Page, slug: string, n: number) {
  await invoke(page, "open_character", { slug });
  for (let l = 0; l < n; l++) {
    let step = await invoke<{ question: any }>(page, "levelup_start");
    while (step.question.type !== "confirm") {
      const q = step.question;
      const answer =
        q.type === "pick"
          ? {
              type: "pick",
              value: q.choices.slice(0, q.count).map((c: any) => c.value),
            }
          : { type: "number", value: 1 };
      step = await invoke(page, "levelup_answer", { answer });
    }
    await invoke(page, "levelup_apply");
  }
  await invoke(page, "close_character");
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
}

const identity = (page: Page) => page.locator(".sheet .id");
const hp = (page: Page) => page.locator(".hpnum");
const apply = (page: Page) =>
  dialog(page, /Level up|First level/)
    .getByRole("button", { name: "Apply the level" })
    .click();
/// An ability's score on the overview.
const score = (page: Page, ability: string) => page.locator(`.abil .score[data-of="${ability}"]`);

test("levelling up the same class takes the average hit points", { tag: "@levelup.average" }, async ({ page }) => {
  await openSheet(page);
  await expect(hp(page)).toContainText("8/8");
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 1 → 2"]);
  await pick(page, ["Average"]);
  await expect(dialog(page, "Level up")).toContainText("Hit points: Average 4");
  await apply(page);
  await expect(status(page)).toContainText("Now level 2");
  await expect(identity(page)).toContainText("Wizard 2");
  // 4 for the level and 2 for Constitution.
  await expect(hp(page)).toContainText("14/14");
});

test("hit points rolled at the table are typed in", { tag: "@levelup.hp" }, async ({ page }) => {
  await openSheet(page);
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 1 → 2"]);
  await pick(page, ["Roll it at the table…"]);
  const roll = dialog(page, "Level up").getByPlaceholder("the roll");
  await roll.fill("3");
  await dialog(page, "Level up").getByRole("button", { name: "OK" }).click();
  await apply(page);
  await expect(identity(page)).toContainText("Wizard 2");
  // 3 for the level and 2 for Constitution.
  await expect(hp(page)).toContainText("13/13");
});

test("the maximum hit points can be taken", { tag: "@levelup.hp" }, async ({ page }) => {
  await openSheet(page);
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 1 → 2"]);
  await pick(page, ["Maximum"]);
  await apply(page);
  // 6 for the level and 2 for Constitution.
  await expect(hp(page)).toContainText("16/16");
});

test("a level in another class, when the scores allow it", { tag: "@levelup.multiclass" }, async ({ page }) => {
  await openSheet(page, "Skills");
  await expect(score(page, "Strength")).toHaveText("15");
  // Strength 15: the Fighter is open.
  await more(page, "Tester", "Level up…");
  const fighter = dialog(page, "Level up").locator("li button", {
    hasText: "New class: Fighter",
  });
  await expect(fighter).toBeVisible();
  await expect(fighter.locator(".warn")).toHaveCount(0);
  // Not the Hexer: Charisma 8.
  await expect(dialog(page, "Level up").locator("li button", { hasText: "New class: Hexer" }).locator(".warn")).toContainText("Charisma 13");
  await fighter.click();
  await pick(page, ["Average"]);
  // The new class's own choices at its first level.
  await expect(dialog(page, "Level up")).toContainText("Fighting Style");
  await pick(page, ["Defense"]);
  await expect(dialog(page, "Level up")).toContainText("Weapon Mastery");
  await pick(page, ["Greatsword", "Handaxe", "Longsword"]);
  await apply(page);
  await expect(identity(page)).toContainText("Wizard 1");
  await expect(identity(page)).toContainText("Fighter 1");
  await expect(page.getByRole("button", { name: "Warnings" })).toHaveCount(0);
});

test("the subclass is chosen at the level the class gets it", { tag: "@levelup.subclass" }, async ({ page }) => {
  await seedLevels(page, "tester", 1);
  await openSheet(page);
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 2 → 3"]);
  await expect(dialog(page, "Level up")).toContainText("Choose a subclass");
  // Only the Wizard's subclasses.
  await expect(dialog(page, "Level up")).not.toContainText("The Lantern");
  await pick(page, ["Evoker"]);
  await pick(page, ["Average"]);
  await apply(page);
  await expect(identity(page)).toContainText("Wizard 3");
  await sheetTab(page, "Features");
  await expect(page.locator(".pane", { hasText: "Evoker" }).first()).toBeVisible();
});

test("an ability score improvement raises a score", { tag: "@levelup.asi" }, async ({ page }) => {
  await seedLevels(page, "tester", 2);
  await openSheet(page, "Skills");
  await expect(score(page, "Intelligence")).toHaveText("13");
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 3 → 4"]);
  await pick(page, ["Average"]);
  await pick(page, ["+2 to one ability score"]);
  await pick(page, ["Intelligence"]);
  await apply(page);
  await expect(identity(page)).toContainText("Wizard 4");
  await expect(score(page, "Intelligence")).toHaveText("15");
});

test("an ability score improvement can be a feat instead", { tag: "@levelup.asi" }, async ({ page }) => {
  await seedLevels(page, "tester", 2);
  await openSheet(page);
  await more(page, "Tester", "Level up…");
  await pick(page, ["Wizard 3 → 4"]);
  await pick(page, ["Average"]);
  await pick(page, ["Take a feat instead…"]);
  await pick(page, ["Grappler"]);
  await apply(page);
  await sheetTab(page, "Features");
  await expect(page.locator(".feature", { hasText: "Grappler" })).toBeVisible();
});

test("a character with no levels takes the first one", { tag: "@levelup.first-level" }, async ({ page }) => {
  await openSheet(page);
  // No levels: the file without them.
  await more(page, "Tester", "Edit the character file…");
  const box = dialog(page, "Tester · the file").getByRole("textbox");
  await box.fill((await box.inputValue()).replace('[[levels]]\nclass = "class:Wizard|SRD52"\n', ""));
  await dialog(page, "Tester · the file").getByRole("button", { name: "Save" }).click();
  await expect(identity(page)).not.toContainText("Wizard");

  await page.getByRole("button", { name: "Take the first level" }).click();
  await expect(dialog(page, "First level")).toBeVisible();
  await pick(page, ["Wizard"]);
  await apply(page);
  await expect(identity(page)).toContainText("Wizard 1");
  await expect(page.getByRole("button", { name: "Take the first level" })).toHaveCount(0);
  // The maximum 6 at the first level and 2 for Constitution.
  await expect(hp(page)).toContainText("8/8");
});

test("undoing the latest level asks first", { tag: "@levelup.undo" }, async ({ page }) => {
  await seedLevels(page, "tester", 1);
  await openSheet(page);
  await expect(identity(page)).toContainText("Wizard 2");
  await more(page, "Tester", "Remove the latest level");
  await choose(page, "Undo a level", "Cancel");
  await expect(identity(page)).toContainText("Wizard 2");
  await more(page, "Tester", "Remove the latest level");
  await choose(page, "Undo a level", "Yes");
  await expect(identity(page)).toContainText("Wizard 1");
  await expect(hp(page)).toContainText("8/8");
});

test("choices a level left open are made afterwards", { tag: "@choices.pending" }, async ({ page }) => {
  await openSheet(page, "Overview", "Ranger");
  // A Fighter level written into the file, without its choices.
  await more(page, "Ranger", "Edit the character file…");
  const box = dialog(page, "Ranger · the file").getByRole("textbox");
  await box.fill(
    // After the first level and its choices, before the inventory.
    (await box.inputValue()).replace("[[inventory]]", '[[levels]]\nclass = "class:Fighter|SRD52"\n\n[[inventory]]'),
  );
  await dialog(page, "Ranger · the file").getByRole("button", { name: "Save" }).click();
  await expect(identity(page)).toContainText("Fighter 1");

  await page.getByRole("button", { name: /2 choices/ }).click();
  await expect(dialog(page, /Fighting Style/)).toBeVisible();
  await pick(page, ["Defense"]);
  const masteries = dialog(page, /Weapon Mastery/);
  await expect(masteries).toBeVisible();
  // Three picks of three: the third one closes it.
  await masteries.locator("li button", { hasText: "Greatsword" }).click();
  await masteries.locator("li button", { hasText: "Handaxe" }).click();
  await masteries.locator("li button", { hasText: "Longsword" }).click();
  await expect(masteries).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /choices? to make|^\d+ choices?$/ })).toHaveCount(0);
  await sheetTab(page, "Features");
  await expect(page.locator(".feature", { hasText: "Defense" })).toBeVisible();
  await expect(page.locator(".feature", { hasText: "Greatsword" })).toBeVisible();
  await expect(page.locator(".feature", { hasText: "Handaxe" })).toBeVisible();
});

test("open choices are also made from the sheet's menu", { tag: "@choices.pending" }, async ({ page }) => {
  await openSheet(page, "Overview", "Ranger");
  await more(page, "Ranger", "Edit the character file…");
  const box = dialog(page, "Ranger · the file").getByRole("textbox");
  await box.fill(
    // After the first level and its choices, before the inventory.
    (await box.inputValue()).replace("[[inventory]]", '[[levels]]\nclass = "class:Fighter|SRD52"\n\n[[inventory]]'),
  );
  await dialog(page, "Ranger · the file").getByRole("button", { name: "Save" }).click();
  await more(page, "Ranger", "Make 2 missing choice(s)…");
  await pick(page, ["Archery"]);
  await page.keyboard.press("Escape");
  // One left, for later.
  await expect(page.getByRole("button", { name: /^1 choice/ })).toBeVisible();
});
