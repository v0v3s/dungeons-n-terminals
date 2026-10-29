import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, invoke, openSheet, pick, sheetMore, sheetTab, status, test } from "./fixtures";

const roll = (page: Page) => page.locator(".status.roll");
const attack = (page: Page, name: string | RegExp) => page.locator(".attack").filter({ has: page.locator(".aname", { hasText: name }) });
/// The Quarterstaff itself, not the Arcane Focus (Quarterstaff).
const STAFF = /^\s*Quarterstaff\s*$/;
const held = (page: Page) => page.locator(".held");
const chip = (page: Page) => page.locator(".state");
const tracker = (page: Page, name: string) => page.locator(".tracker").filter({ has: page.locator(".tname", { hasText: name }) });
const tile = (page: Page, label: string) => page.locator("button.tile", { hasText: label });

/// The sheet's own menu (⋯ by the name).
async function more(page: Page, name: string, action: string | RegExp) {
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

/// Add a tracker of your own through the Tracking pane.
async function addTracker(page: Page, name: string, uses: number, back: string) {
  await page.getByRole("button", { name: "+ Tracker" }).click();
  await answer(page, "New tracker", name);
  await answer(page, `${name}: how many uses?`, String(uses));
  await choose(page, `${name}: the uses come back`, back);
}

test("an attack rolls to hit with its damage", { tag: "@roll.attack" }, async ({ page }) => {
  await openSheet(page);
  await attack(page, "Dagger").locator("button.arow").click();
  await expect(roll(page)).toContainText("Dagger");
  await expect(roll(page)).toContainText(/to hit|critical|miss/);
  await expect(roll(page)).toContainText("piercing damage");

  // The same from the attack's menu.
  await attack(page, STAFF).getByRole("button", { name: "More" }).click();
  await choose(page, "Quarterstaff", "Roll the attack");
  await expect(roll(page)).toContainText("Quarterstaff");
  await expect(roll(page)).toContainText("bludgeoning damage");
});

test("advantage or disadvantage is chosen for the next roll only", { tag: "@roll.advantage" }, async ({ page }) => {
  await openSheet(page);
  const mode = chip(page).locator("button.mode");
  await expect(mode).toHaveText(/next roll: normal/);
  await mode.click();
  await expect(mode).toHaveText(/next roll: advantage/);
  await tile(page, "Init").click();
  await expect(roll(page)).toContainText("advantage (chosen)");
  await expect(mode).toHaveText(/next roll: normal/);

  await mode.click();
  await mode.click();
  await expect(mode).toHaveText(/next roll: disadvantage/);
  await tile(page, "Init").click();
  await expect(roll(page)).toContainText("disadvantage (chosen)");
  await expect(mode).toHaveText(/next roll: normal/);

  await tile(page, "Init").click();
  await expect(roll(page)).toContainText("Initiative");
  await expect(roll(page)).not.toContainText("chosen");
});

test("rolls are written into the latest session note", { tag: "@roll.to-notes" }, async ({ page }) => {
  // The choice is kept on the device: start without it.
  await page.evaluate(() => localStorage.removeItem("dnt.logRolls"));
  const note = await invoke<string>(page, "note_new_session");
  try {
    await openSheet(page, "Skills");
    await more(page, "Tester", "Write rolls into the session note");
    await page.locator("button.skill", { hasText: "Arcana" }).click();
    await expect(roll(page)).toContainText("Arcana");
    await expect.poll(async () => (await invoke<{ text: string }>(page, "note_read", { path: note })).text).toContain("Arcana");

    await more(page, "Tester", "Stop writing rolls into the session note");
    await page.locator("button.skill", { hasText: "History" }).click();
    await expect(roll(page)).toContainText("History");
    await page.waitForTimeout(500);
    expect((await invoke<{ text: string }>(page, "note_read", { path: note })).text).not.toContain("History");
  } finally {
    await page.evaluate(() => localStorage.removeItem("dnt.logRolls"));
  }
});

test("conditions are turned on and off; exhaustion goes up and down", { tag: "@conditions.toggle" }, async ({ page }) => {
  await openSheet(page);
  await chip(page).getByRole("button", { name: "No conditions" }).click();
  const conditions = dialog(page, "Conditions");
  await conditions.locator("button.cond", { hasText: "Poisoned" }).click();
  await expect(conditions.locator("button.cond.on")).toHaveText(/Poisoned/);
  await conditions.locator(".exh").getByRole("button", { name: "+" }).click();
  await conditions.locator(".exh").getByRole("button", { name: "+" }).click();
  await expect(conditions.locator(".exh b")).toHaveText("2");
  await conditions.locator(".exh").getByRole("button", { name: "−" }).click();
  await expect(conditions.locator(".exh b")).toHaveText("1");
  await page.keyboard.press("Escape");
  await expect(chip(page).locator("button.chip").first()).toHaveText("Poisoned · Exhaustion 1");
  await expect(page.locator(".overview")).toContainText("Poisoned:");

  await chip(page).locator("button.chip").first().click();
  await conditions.locator("button.cond", { hasText: "Poisoned" }).click();
  await expect(conditions.locator("button.cond.on")).toHaveCount(0);
  await conditions.locator(".exh").getByRole("button", { name: "−" }).click();
  await expect(conditions.locator(".exh b")).toHaveText("0");
  await page.keyboard.press("Escape");
  await expect(chip(page).locator("button.chip").first()).toHaveText("No conditions");
});

test("a feature's toggle is turned on and off, and the numbers follow", { tag: "@toggles.flip" }, async ({ page }) => {
  await openSheet(page);
  const ac = tile(page, "Armor Class").locator("b");
  const before = Number(await ac.innerText());
  await writeRules(
    page,
    "Wizard 1",
    "Arcane Recovery",
    '[[feature.toggle]]\nkey = "warded"\nname = "Warded"\n\n[[feature.effect]]\ntarget = "ac"\nadd = "2"\nwhen = "toggle(\'warded\')"\n',
  );
  const toggle = page.locator("button.toggle", { hasText: "Warded" });
  await expect(toggle).toHaveText(/○\s*Warded/);
  await expect(ac).toHaveText(String(before));
  await toggle.click();
  await expect(toggle).toHaveText(/◉\s*Warded/);
  await expect(ac).toHaveText(String(before + 2));
  await toggle.click();
  await expect(toggle).toHaveText(/○\s*Warded/);
  await expect(ac).toHaveText(String(before));
});

test("a tracker of your own is added with its uses and when they come back", { tag: "@trackers.add" }, async ({ page }) => {
  await openSheet(page);
  await addTracker(page, "Lucky Coin", 3, "At dawn");
  const row = tracker(page, "Lucky Coin");
  await expect(row.locator(".count")).toHaveText("3/3");
  await expect(row.locator(".recharge")).toHaveText("dawn");
  await row.getByRole("button", { name: "Use" }).click();
  await expect(row.locator(".count")).toHaveText("2/3");
});

test("removing a tracker of your own asks first", { tag: "@trackers.remove" }, async ({ page }) => {
  await openSheet(page);
  await addTracker(page, "Lucky Coin", 3, "After a long rest");
  await expect(tracker(page, "Lucky Coin")).toHaveCount(1);

  await tracker(page, "Lucky Coin").locator("button.tname").click();
  await choose(page, "Lucky Coin", "Remove the tracker…");
  // A confirmation, as for anything removed for good: declining keeps it.
  await dialog(page, "Remove Lucky Coin").getByRole("button", { name: "Cancel" }).click();
  await expect(tracker(page, "Lucky Coin")).toHaveCount(1);

  await tracker(page, "Lucky Coin").locator("button.tname").click();
  await choose(page, "Lucky Coin", "Remove the tracker…");
  await dialog(page, "Remove Lucky Coin").getByRole("button", { name: "Remove" }).click();
  await expect(tracker(page, "Lucky Coin")).toHaveCount(0);
});

test("a weapon is held from the attacks: main hand, off hand or both hands", { tag: "@attacks.hold" }, async ({ page }) => {
  await openSheet(page);
  await expect(held(page)).toHaveText(/In hand\s+nothing/);

  await attack(page, STAFF).getByRole("button", { name: "More" }).click();
  await choose(page, "Quarterstaff", "Hold with both hands");
  await expect(held(page)).toContainText("Quarterstaff (two hands)");
  await expect(attack(page, STAFF).locator(".hand")).not.toBeEmpty();

  await attack(page, "Dagger").getByRole("button", { name: "More" }).click();
  await choose(page, "Dagger", "Hold in the main hand");
  await expect(held(page)).toContainText("Dagger");
  // Both hands are needed for the staff: it is put away.
  await expect(held(page)).not.toContainText("Quarterstaff");

  await attack(page, "Dagger").getByRole("button", { name: "More" }).click();
  await choose(page, "Dagger", "Hold in the off hand");
  await expect(held(page)).toContainText("Dagger");
  const hands = await page.locator(".attack .hand").allInnerTexts();
  expect(hands.filter((h) => h.trim()).length).toBeGreaterThan(0);
});

test("concentration on a spell is ended from the overview", { tag: "@concentration.end" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await page.getByRole("button", { name: "+ add" }).first().click();
  await pick(page, ["Fog Cloud"]);
  const spell = page.locator("button.spell", { hasText: "Fog Cloud" });
  await expect(spell).toBeVisible();
  await spell.click();
  if (await dialog(page, "Fog Cloud").getByRole("button", { name: "Prepare", exact: true }).count()) {
    await choose(page, "Fog Cloud", "Prepare");
    await spell.click();
  }
  await choose(page, "Fog Cloud", "Cast…");
  await page.getByRole("dialog").last().locator("li button").first().click();
  await expect(status(page)).toContainText("Fog Cloud");

  await sheetTab(page, "Overview");
  const conc = chip(page).locator("button.conc");
  await expect(conc).toContainText("Fog Cloud");
  await conc.click();
  await expect(conc).toHaveCount(0);
});
