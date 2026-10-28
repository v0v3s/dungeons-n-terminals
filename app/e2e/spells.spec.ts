import type { Page } from "@playwright/test";
import { choose, dialog, expect, invoke, openSheet, pick, sheetMore, sheetTab, status, test } from "./fixtures";

const spell = (page: Page, name: string) => page.locator("button.spell", { hasText: name });
const slot = (page: Page, name: string) => page.locator(".slot", { hasText: name });

/// Add spells to the first class on the Spells tab, one at a time.
async function addSpells(page: Page, names: string[]) {
  for (const name of names) {
    await page.getByRole("button", { name: "+ add" }).first().click();
    await pick(page, [name]);
    await expect(spell(page, name)).toBeVisible();
  }
}

/// Choose what to do with a spell from its menu.
async function act(page: Page, name: string, action: string) {
  await spell(page, name).first().click();
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

test("a new mage adds cantrips and spells to the grimoire", { tag: "@spells.add" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle", "Spark", "Glimmer"]);
  await expect(page.locator("button.spell")).toHaveCount(3);
  await addSpells(page, ["Ember Bolt"]);
  await expect(spell(page, "Ember Bolt")).toBeVisible();
});

test("a cantrip rolls with the mage's numbers", { tag: "@spells.roll" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle"]);
  await spell(page, "Frost Needle").click();
  await dialog(page, "Frost Needle").getByRole("button", { name: "Roll it" }).click();
  await expect(page.locator(".status.roll")).toContainText("Frost Needle");
});

test("the spells to choose: a class's, and a feat's choice", { tag: "@spells.choose" }, async ({ page }) => {
  await openSheet(page, "Overview", "Hexa");
  // Where the flag says "spells to choose" in full, "spells" and "to" are
  // a space apart, not run together.
  const gap = await page.locator(".flag", { hasText: "spells" }).evaluate((flag) => {
    const wide = flag.querySelector(".wide");
    const words = wide?.previousSibling;
    if (!wide?.firstChild || !words || getComputedStyle(wide).display === "none") return null;
    const before = document.createRange();
    before.selectNodeContents(words);
    const to = document.createRange();
    const at = (wide.firstChild.textContent ?? "").search(/\S/);
    to.setStart(wide.firstChild, at);
    to.setEnd(wide.firstChild, at + 1);
    return to.getBoundingClientRect().left - before.getBoundingClientRect().right;
  });
  if (gap !== null) expect(gap, "a space between the words").toBeGreaterThan(2);
  // The flag counts the class's 2 cantrips and 2 spells and the feat's 2
  // cantrips, and goes to the spells to choose.
  await page.getByRole("button", { name: /6 spells/ }).click();
  await pick(page, ["Frost Needle", "Mending Hum"]);
  await expect(spell(page, "Frost Needle")).toBeVisible();
  await expect(spell(page, "Mending Hum")).toBeVisible();
  await expect(page.locator("button.choice")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /4 spells/ })).toBeVisible();

  // The class's cantrips.
  await page.getByRole("button", { name: "+ add" }).first().click();
  await pick(page, ["Hex Bolt"]);
  await expect(spell(page, "Hex Bolt")).toBeVisible();
  await expect(page.getByRole("button", { name: /3 spells/ })).toBeVisible();
});

test("a grant's choice is made from its line", { tag: "@spells.choose" }, async ({ page }) => {
  await openSheet(page, "Spells", "Hexa");
  await page.locator("button.choice", { hasText: "choose 2 Mage cantrip" }).click();
  await pick(page, ["Spark", "Glimmer"]);
  await expect(page.locator("button.choice")).toHaveCount(0);
  await expect(spell(page, "Spark")).toBeVisible();
});

test("casting with a slot spends it; a concentration spell is noted", { tag: "@spells.cast" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ember Bolt", "Ward of Thorns"]);
  await expect(slot(page, "Level 1")).toContainText("2/2");
  await act(page, "Ember Bolt", "Cast…");
  await pick(page, ["Level 1"]);
  await expect(slot(page, "Level 1")).toContainText("1/2");

  await act(page, "Ward of Thorns", "Cast…");
  await pick(page, ["Level 1"]);
  await expect(slot(page, "Level 1")).toContainText("0/2");
  await sheetTab(page, "Overview");
  await expect(page.locator(".chip.conc")).toContainText("Ward of Thorns");
});

test("a free cast is spent and comes back on a long rest", { tag: "@spells.cast-free" }, async ({ page }) => {
  // An Emberkin casts Ember Bolt once a day from level 3.
  await seedLevels(page, "hexa", 2);
  await openSheet(page, "Spells", "Hexa");
  await expect(spell(page, "Ember Bolt")).toBeVisible();
  const pact = await page.locator(".slot").first().textContent();
  await act(page, "Ember Bolt", "Cast (a free cast)");
  await expect(status(page)).toContainText("Ember Bolt");
  // No slot was spent, and the free cast is gone.
  await expect(page.locator(".slot").first()).toHaveText(pact ?? "");
  await spell(page, "Ember Bolt").click();
  await expect(
    dialog(page, "Ember Bolt").getByRole("button", {
      name: "Cast (a free cast)",
    }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");

  await sheetMore(page);
  await choose(page, "Hexa", "Long rest");
  await choose(page, "Long rest", "Yes");
  await spell(page, "Ember Bolt").click();
  await expect(
    dialog(page, "Ember Bolt").getByRole("button", {
      name: "Cast (a free cast)",
    }),
  ).toBeVisible();
});

test("a spell in the spellbook is prepared and unprepared", { tag: "@spells.prepare" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ember Bolt"]);
  const mark = spell(page, "Ember Bolt").locator(".mark");
  const before = await mark.textContent();
  await act(page, "Ember Bolt", before === "●" ? "Unprepare" : "Prepare");
  await expect(mark).not.toHaveText(before ?? "");
  await act(page, "Ember Bolt", before === "●" ? "Prepare" : "Unprepare");
  await expect(mark).toHaveText(before ?? "");
});

test("a spell is made a favorite, and no longer", { tag: "@spells.favorite" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle", "Ember Bolt"]);
  await act(page, "Ember Bolt", "Make it a favorite");
  // Favorites come first.
  await expect(page.locator("button.spell").first()).toContainText("Ember Bolt");
  await spell(page, "Ember Bolt").first().click();
  await expect(
    dialog(page, "Ember Bolt").getByRole("button", {
      name: "No longer a favorite",
    }),
  ).toBeVisible();
  await choose(page, "Ember Bolt", "No longer a favorite");
  await expect(page.locator(".header .title", { hasText: /Favorite/i })).toHaveCount(0);
});

test("a spell is removed", { tag: "@spells.remove" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Frost Needle", "Ember Bolt"]);
  await act(page, "Ember Bolt", "Remove");
  await expect(spell(page, "Ember Bolt")).toHaveCount(0);
  await expect(spell(page, "Frost Needle")).toBeVisible();
});
