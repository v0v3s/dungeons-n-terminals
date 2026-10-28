import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, openSheet, sheetMore, sheetTab, test } from "./fixtures";

/// A pool by its name, on its tab: spell slots on Spells, hit dice (by
/// the hit points) on the Overview.
async function at(page: Page, name: string) {
  const slots = name.endsWith("slots");
  await sheetTab(page, slots ? "Spells" : "Overview");
  return slots
    ? page.locator(".slot").filter({ has: page.locator(".sname", { hasText: name }) })
    : page.locator(".tracker").filter({ has: page.locator(".tname", { hasText: name }) });
}
const left = async (page: Page, name: string) => (await at(page, name)).locator(".count");
const use = async (page: Page, name: string) => (await at(page, name)).getByRole("button", { name: "Use" }).click();
const restore = async (page: Page, name: string) => (await at(page, name)).getByRole("button", { name: "Restore" }).click();
const hp = (page: Page) => page.locator(".hpnum");

/// The sheet's own menu (⋯ by the name).
async function more(page: Page, name: string, action: string) {
  await sheetMore(page);
  await choose(page, name, action);
}

/// Write rules for one of the features on the Features tab.
async function writeRules(page: Page, group: string, feature: string, rules: string) {
  await sheetTab(page, "Features");
  const fold = page.locator(".pane").filter({ has: page.locator("button.fold", { hasText: group }) }).locator("button.fold");
  if ((await fold.getAttribute("aria-expanded")) === "false") await fold.click();
  await page.locator(".row", { has: page.locator(".name", { hasText: feature }) }).getByRole("button", { name: "rules" }).click();
  const editor = dialog(page, `Rules · ${feature}`);
  const box = editor.getByRole("textbox");
  await expect(box).toHaveValue(/id = "/);
  const id = (await box.inputValue()).match(/^id = ".*"$/m)![0];
  await box.fill(`[[feature]]\n${id}\n\n${rules}`);
  await expect(editor.locator(".check")).not.toContainText("✗");
  await editor.getByRole("button", { name: "Save" }).click();
  await expect(editor).toHaveCount(0);
  await sheetTab(page, "Overview");
}

test("a short rest, once confirmed, brings back the pact slots but not the hit dice", { tag: "@rest.short" }, async ({ page }) => {
  await openSheet(page, "Overview", "Hexa");
  await use(page, "Pact slots");
  await use(page, "Hit dice");
  await expect(await left(page, "Pact slots")).toHaveText("0/1");
  await expect(await left(page, "Hit dice")).toHaveText("0/1");

  await more(page, "Hexa", "Short rest");
  await choose(page, "Short rest", "Cancel");
  await expect(await left(page, "Pact slots")).toHaveText("0/1");

  await more(page, "Hexa", "Short rest");
  await choose(page, "Short rest", "Yes");
  await expect(await left(page, "Pact slots")).toHaveText("1/1");
  await expect(await left(page, "Hit dice")).toHaveText("0/1");
});

test("a long rest, once confirmed, brings back hit points, slots and hit dice", { tag: "@rest.long" }, async ({ page }) => {
  await openSheet(page);
  await page.locator(".pane.hp").getByRole("button", { name: "Damage", exact: true }).click();
  await answer(page, "Damage", "4");
  await use(page, "Level 1 slots");
  await use(page, "Level 1 slots");
  await use(page, "Hit dice");
  await sheetTab(page, "Overview");
  await expect(hp(page)).toHaveText(/^\s*3\/7$/);

  await more(page, "Tester", "Long rest");
  await expect(dialog(page, "Long rest")).toContainText("Take a long rest?");
  await choose(page, "Long rest", "Cancel");
  await expect(await left(page, "Level 1 slots")).toHaveText("0/2");

  await more(page, "Tester", "Long rest");
  await choose(page, "Long rest", "Yes");
  await sheetTab(page, "Overview");
  await expect(hp(page)).toHaveText(/^\s*7\/7$/);
  await expect(await left(page, "Level 1 slots")).toHaveText("2/2");
  await expect(await left(page, "Hit dice")).toHaveText("1/1");
});

test("spell slots and hit dice are spent and restored", { tag: "@resources.spend" }, async ({ page }) => {
  await openSheet(page);
  await use(page, "Level 1 slots");
  await expect(await left(page, "Level 1 slots")).toHaveText("1/2");
  await expect((await at(page, "Level 1 slots")).locator(".pips")).toHaveText("●○");
  await use(page, "Level 1 slots");
  await expect(await left(page, "Level 1 slots")).toHaveText("0/2");
  // None left: nothing more to spend.
  await use(page, "Level 1 slots");
  await expect(await left(page, "Level 1 slots")).toHaveText("0/2");
  await restore(page, "Level 1 slots");
  await expect(await left(page, "Level 1 slots")).toHaveText("1/2");

  await use(page, "Hit dice");
  await expect(await left(page, "Hit dice")).toHaveText("0/1");
  await restore(page, "Hit dice");
  await expect(await left(page, "Hit dice")).toHaveText("1/1");
});

test("pact slots are spent and restored", { tag: "@resources.spend" }, async ({ page }) => {
  await openSheet(page, "Overview", "Hexa");
  await use(page, "Pact slots");
  await expect(await left(page, "Pact slots")).toHaveText("0/1");
  await restore(page, "Pact slots");
  await expect(await left(page, "Pact slots")).toHaveText("1/1");
});

test("a feature's uses are spent, restored, and come back on a short rest", { tag: ["@resources.spend", "@rest.short"] }, async ({ page }) => {
  await openSheet(page);
  await writeRules(
    page,
    "Mage 1",
    "Arcane Recall",
    '[[feature.resource]]\nkey = "recall"\nname = "Recall"\nmax = "2"\nrecharge = "short_rest"\n',
  );
  await expect(await left(page, "Recall")).toHaveText("2/2");
  await use(page, "Recall");
  await use(page, "Recall");
  await expect(await left(page, "Recall")).toHaveText("0/2");
  await restore(page, "Recall");
  await expect(await left(page, "Recall")).toHaveText("1/2");

  await more(page, "Tester", "Short rest");
  await choose(page, "Short rest", "Yes");
  await expect(await left(page, "Recall")).toHaveText("2/2");
});
