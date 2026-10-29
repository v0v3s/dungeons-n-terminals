import type { Page } from "@playwright/test";
import { choose, dialog, expect, invoke, openSheet, pick, sheetMore, sheetTab, status, test } from "./fixtures";

const spell = (page: Page, name: string) => page.locator("button.spell", { hasText: name });
/// A level's slots (or the pact slots), in its heading on the Spells tab.
const slot = (page: Page, name: string) => page.locator(`.slot[data-pool^="${name}"]`);

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

test("a new wizard adds cantrips and spells to the spellbook", { tag: "@spells.add" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ray of Frost", "Acid Splash", "Mage Hand"]);
  await expect(page.locator("button.spell")).toHaveCount(3);
  await addSpells(page, ["Burning Hands"]);
  await expect(spell(page, "Burning Hands")).toBeVisible();
});

test("a cantrip rolls with the wizard's numbers", { tag: "@spells.roll" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ray of Frost"]);
  await spell(page, "Ray of Frost").click();
  await dialog(page, "Ray of Frost").getByRole("button", { name: "Roll it" }).click();
  await expect(page.locator(".status.roll")).toContainText("Ray of Frost");
  await page.locator(".status.roll").click();
  // Or straight from its roll button.
  await page.getByRole("button", { name: "Roll Ray of Frost" }).click();
  await expect(page.locator(".status.roll")).toContainText("Ray of Frost");
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
  // cantrips and 1 spell, and goes to the spells to choose.
  await page.getByRole("button", { name: /7 spells/ }).click();
  await pick(page, ["Guidance", "Sacred Flame"]);
  await expect(spell(page, "Guidance")).toBeVisible();
  await expect(spell(page, "Sacred Flame")).toBeVisible();
  // The feat's level 1 spell is still to choose.
  await expect(page.locator("button.choice")).toHaveCount(1);
  await expect(page.getByRole("button", { name: /5 spells/ })).toBeVisible();

  // The class's cantrips.
  await page.getByRole("button", { name: "+ add" }).first().click();
  await pick(page, ["Eldritch Blast"]);
  await expect(spell(page, "Eldritch Blast")).toBeVisible();
  await expect(page.getByRole("button", { name: /4 spells/ })).toBeVisible();
});

test("a grant's choice is made from its line", { tag: "@spells.choose" }, async ({ page }) => {
  await openSheet(page, "Spells", "Hexa");
  await page.locator("button.choice", { hasText: "choose 2 Cleric cantrip" }).click();
  await pick(page, ["Guidance", "Sacred Flame"]);
  await expect(page.locator("button.choice")).toHaveCount(1);
  await expect(spell(page, "Sacred Flame")).toBeVisible();
  await page.locator("button.choice", { hasText: "choose 1 Cleric spell" }).click();
  await pick(page, ["Bless"]);
  await expect(page.locator("button.choice")).toHaveCount(0);
  await expect(spell(page, "Bless")).toBeVisible();
});

test("casting with a slot spends it; a concentration spell is noted", { tag: "@spells.cast" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Burning Hands", "Fog Cloud"]);
  await expect(slot(page, "Level 1")).toContainText("2/2");
  await act(page, "Burning Hands", "Cast…");
  await pick(page, ["Level 1"]);
  await expect(slot(page, "Level 1")).toContainText("1/2");

  await act(page, "Fog Cloud", "Cast…");
  await pick(page, ["Level 1"]);
  await expect(slot(page, "Level 1")).toContainText("0/2");
  await sheetTab(page, "Overview");
  await expect(page.locator(".chip.conc")).toContainText("Fog Cloud");
});

test("a free cast is spent and comes back on a long rest", { tag: "@spells.cast-free" }, async ({ page }) => {
  // A High Elf casts Detect Magic once a day from level 3.
  await seedLevels(page, "hexa", 2);
  await openSheet(page, "Spells", "Hexa");
  await expect(spell(page, "Detect Magic")).toBeVisible();
  const pact = await page.locator(".slot").first().textContent();
  await act(page, "Detect Magic", "Cast (a free cast)");
  await expect(status(page)).toContainText("Detect Magic");
  // No slot was spent, and the free cast is gone.
  await expect(page.locator(".slot").first()).toHaveText(pact ?? "");
  await spell(page, "Detect Magic").click();
  await expect(
    dialog(page, "Detect Magic").getByRole("button", {
      name: "Cast (a free cast)",
    }),
  ).toHaveCount(0);
  await page.keyboard.press("Escape");

  await sheetMore(page);
  await choose(page, "Hexa", "Long rest");
  await choose(page, "Long rest", "Yes");
  await spell(page, "Detect Magic").click();
  await expect(
    dialog(page, "Detect Magic").getByRole("button", {
      name: "Cast (a free cast)",
    }),
  ).toBeVisible();
});

test("a spell in the spellbook is prepared and unprepared", { tag: "@spells.prepare" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Burning Hands"]);
  // Its box on the left, and its menu, both prepare it.
  const prepared = page.getByRole("checkbox", { name: "Prepared: Burning Hands" });
  const before = (await prepared.getAttribute("aria-checked")) === "true";
  await prepared.click();
  await expect(prepared).toHaveAttribute("aria-checked", String(!before));
  await act(page, "Burning Hands", before ? "Prepare" : "Unprepare");
  await expect(prepared).toHaveAttribute("aria-checked", String(before));
});

test("spells are grouped by level, each level's slots in its heading", { tag: "@spells.by-level" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ray of Frost", "Burning Hands"]);
  const level = (title: string) => page.locator(".level", { has: page.locator(".ltitle", { hasText: title }) });
  await expect(level("Cantrips")).toBeVisible();
  await expect(level("Level 1").locator(".slot")).toContainText("2/2");
  // A spell sits under its level's heading.
  const after = (title: string, name: string) =>
    page.locator(`.level:has(.ltitle:text-is("${title}")) ~ .srow`, { hasText: name }).first();
  await expect(after("Level 1", "Burning Hands")).toBeVisible();
  await expect(page.locator(".boxes").first()).toContainText("save DC");
});

test("a spell is made a favorite, and no longer", { tag: "@spells.favorite" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ray of Frost", "Burning Hands"]);
  await act(page, "Burning Hands", "Make it a favorite");
  // Favorites come first.
  await expect(page.locator("button.spell").first()).toContainText("Burning Hands");
  await spell(page, "Burning Hands").first().click();
  await expect(
    dialog(page, "Burning Hands").getByRole("button", {
      name: "No longer a favorite",
    }),
  ).toBeVisible();
  await choose(page, "Burning Hands", "No longer a favorite");
  await expect(page.locator(".header .title", { hasText: /Favorite/i })).toHaveCount(0);
});

test("a spell is removed", { tag: "@spells.remove" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await addSpells(page, ["Ray of Frost", "Burning Hands"]);
  await act(page, "Burning Hands", "Remove");
  await expect(spell(page, "Burning Hands")).toHaveCount(0);
  await expect(spell(page, "Ray of Frost")).toBeVisible();
});
