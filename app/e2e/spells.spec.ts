import type { Page } from "@playwright/test";
import { choose, dialog, expect, invoke, openSheet, pick, sheetMore, sheetTab, status, test, wren } from "./fixtures";

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

test("a spell's plain dice are a number, not healing", { tag: "@spells.roll" }, async ({ page }) => {
  await openSheet(page, "Spells", "Hexa");
  await page.locator("button.choice", { hasText: "choose 1 Cleric spell" }).click();
  await pick(page, ["Bane"]);
  await page.getByRole("button", { name: "Roll Bane" }).click();
  await expect(page.locator(".status.roll")).toContainText("Charisma save");
  await expect(page.locator(".status.roll")).not.toContainText("healing");
});

test("dice that are not damage are named for what they do; damage still says damage", { tag: "@spells.roll" }, async ({ page }) => {
  await openSheet(page, "Spells", "Hexa");
  await page.locator("button.choice", { hasText: "choose 2 Cleric cantrip" }).click();
  await pick(page, ["Guidance", "Resistance"]);
  await page.locator("button.choice", { hasText: "choose 1 Cleric spell" }).click();
  await pick(page, ["Guiding Bolt"]);
  // In the list: what each spell's dice are.
  const row = (name: string) => page.locator(".srow", { has: spell(page, name) }).first();
  await expect(row("Guidance")).toContainText("1d4 ability check bonus");
  await expect(row("Guidance")).not.toContainText("damage");
  await expect(row("Resistance")).toContainText("1d4 damage reduction");
  await expect(row("Guiding Bolt")).toContainText("4d6 radiant");
  // Rolled: Guidance's 1d4 is a bonus to a check, not damage.
  const roll = page.locator(".status.roll");
  await page.getByRole("button", { name: "Roll Guidance" }).click();
  await expect(roll).toContainText("ability check bonus");
  await expect(roll).not.toContainText("damage");
  // Not shown as damage either (damage's red).
  await expect(roll.locator(".n.dmg")).toHaveCount(0);
  await roll.click();
  await expect(roll).toHaveCount(0);
  await page.getByRole("button", { name: "Roll Resistance" }).click();
  await expect(roll).toContainText("damage reduction");
  await roll.click();
  await expect(roll).toHaveCount(0);
  // A damage spell still says damage.
  await page.getByRole("button", { name: "Roll Guiding Bolt" }).click();
  await expect(roll).toContainText("radiant damage");
  await expect(roll.locator(".n.dmg")).toHaveCount(1);
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

test("a feature that grants a spell adds it, under that feature; gone with it", { tag: "@spells.granted" }, async ({ page }) => {
  // Hexa to Warlock 2, taking Pact of the Chain (Find Familiar is no Warlock spell).
  await invoke(page, "open_character", { slug: "hexa" });
  let step = await invoke<{ question: any }>(page, "levelup_start");
  while (step.question.type !== "confirm") {
    const q = step.question;
    const want = (c: any) => /^Pact of the Chain$/.test(c.label);
    const answer =
      q.type === "pick"
        ? { type: "pick", value: [...q.choices.filter(want), ...q.choices.filter((c: any) => !want(c))].slice(0, q.count).map((c: any) => c.value) }
        : { type: "number", value: 1 };
    step = await invoke(page, "levelup_answer", { answer });
  }
  await invoke(page, "levelup_apply");
  await invoke(page, "close_character");
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);

  await openSheet(page, "Spells", "Hexa");
  await expect(spell(page, "Find Familiar")).toBeVisible();
  // Listed under the feature that grants it, as from a feature.
  const under = await spell(page, "Find Familiar").evaluate((b) => {
    for (let el: Element | null = b.closest(".srow"); el; el = el.previousElementSibling) {
      const t = el.querySelector?.(".header .title") ?? (el.classList.contains("header") ? el.querySelector(".title") : null);
      if (t) return t.textContent?.trim();
    }
    return null;
  });
  expect(under).toBe("Pact of the Chain");
  await expect(page.locator(".srow", { has: spell(page, "Find Familiar") }).locator(".mark")).toHaveText("◆");
  // The level undone: the invocation goes, and its spell with it.
  await sheetMore(page);
  await choose(page, "Hexa", "Remove the latest level");
  await dialog(page, "Undo a level").getByRole("button", { name: "Yes" }).click();
  await expect(page.locator(".sheet .id")).toContainText("Warlock 1");
  await expect(spell(page, "Find Familiar")).toHaveCount(0);
});

test("a class's spells widen to all spells with a switch, off at first", { tag: "@spells.add-any" }, async ({ page }) => {
  await openSheet(page, "Spells");
  await page.getByRole("button", { name: "+ add" }).first().click();
  const box = page.getByRole("dialog").filter({ has: page.locator(".spellpick") });
  const all = box.getByRole("switch", { name: /All spells/ });
  await expect(all).not.toBeChecked();
  await box.getByRole("textbox").fill("cure wounds");
  await expect(box.getByRole("checkbox", { name: "Choose Cure Wounds", exact: true })).toHaveCount(0);
  // All spells: Cure Wounds (no Wizard spell) is there, and said to be so.
  await all.check();
  await expect(box).toContainText("not only the class's list");
  await box.getByRole("checkbox", { name: "Choose Cure Wounds", exact: true }).check();
  await box.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(spell(page, "Cure Wounds")).toBeVisible();
});

test("several spells are chosen at once: a row read beside, boxes ticked to a count, Confirm", { tag: "@spells.multi-pick" }, async ({ page }) => {
  await openSheet(page, "Spells", "Hexa");
  await page.locator("button.choice", { hasText: "choose 2 Cleric cantrip" }).click();
  const box = page.getByRole("dialog").filter({ has: page.locator(".spellpick") });
  const group = (level: string) => box.locator("li.group", { hasText: new RegExp(`^${level}`) });
  // Cantrips only (the feature's choice), by name, with how many to choose.
  await expect(box.locator("li.group")).toHaveCount(1);
  await expect(group("Cantrips")).toHaveText("Cantrips · choose 2");
  const names = await box.locator("li.item .label").allTextContents();
  expect(names).toEqual([...names].sort((a, b) => a.localeCompare(b)));
  // A click on a row reads it, and keeps it read.
  await box.locator("button.row", { hasText: "Guidance" }).click();
  const read = box.locator(".preview, li.inline").first();
  await expect(read).toContainText("You touch a willing creature and choose a skill");
  await box.locator("button.row", { hasText: "Light" }).first().hover();
  await page.waitForTimeout(300);
  await expect(read).toContainText("You touch a willing creature and choose a skill");
  // Ticked to the count, no more; the heading counts down.
  await box.getByRole("checkbox", { name: "Choose Guidance", exact: true }).check();
  await expect(group("Cantrips")).toHaveText("Cantrips · choose 1");
  await box.getByRole("checkbox", { name: "Choose Resistance", exact: true }).check();
  await expect(group("Cantrips")).toHaveText("Cantrips · none left");
  await expect(box.getByRole("checkbox", { name: "Choose Sacred Flame", exact: true })).toBeDisabled();
  // The keyboard: Space unticks the one under the cursor (Guidance), the
  // arrows move, Space ticks, Enter confirms.
  await box.locator("button.row", { hasText: "Guidance" }).click();
  await page.keyboard.press(" ");
  await expect(group("Cantrips")).toHaveText("Cantrips · choose 1");
  await box.locator("button.row", { hasText: "Sacred Flame" }).click();
  await page.keyboard.press(" ");
  await page.keyboard.press("Enter");
  await expect(box).toHaveCount(0);
  await expect(spell(page, "Resistance")).toBeVisible();
  await expect(spell(page, "Sacred Flame")).toBeVisible();
  await expect(spell(page, "Guidance")).toHaveCount(0);
});

test("a class's spells by level: what is had ticked, given up to free a pick; full groups hidden", { tag: "@spells.multi-pick" }, async ({ page }) => {
  await openSheet(page, "Spells");
  const open = async () => {
    await page.getByRole("button", { name: "+ add" }).first().click();
    return page.getByRole("dialog").filter({ has: page.locator(".spellpick") });
  };
  // A new Wizard: cantrips and level 1, each with its count.
  let box = await open();
  const group = (level: string) => box.locator("li.group", { hasText: new RegExp(`^${level}`) });
  await expect(box.locator("li.group")).toHaveText(["Cantrips · choose 3", "Level 1 · choose 6"]);
  for (const name of ["Fire Bolt", "Light", "Mage Hand", "Magic Missile"]) await box.getByRole("checkbox", { name: `Choose ${name}`, exact: true }).check();
  await expect(group("Cantrips")).toHaveText("Cantrips · none left");
  await expect(group("Level 1")).toHaveText("Level 1 · choose 5");
  await box.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(spell(page, "Magic Missile")).toBeVisible();
  // Again: the cantrips are full, so not shown; what is had is ticked, as had.
  box = await open();
  await expect(box.locator("li.group")).toHaveText(["Level 1 · choose 5"]);
  const missile = box.locator("li.item", { hasText: "Magic Missile" });
  await expect(missile.getByRole("checkbox")).toBeChecked();
  await expect(missile.locator(".had")).toHaveText("in your spellbook");
  // Given up, it frees a pick; Confirm takes it out of the spellbook.
  await missile.getByRole("checkbox").uncheck();
  await expect(group("Level 1")).toHaveText("Level 1 · choose 6");
  await box.getByRole("checkbox", { name: "Choose Shield", exact: true }).check();
  await box.getByRole("button", { name: "Confirm", exact: true }).click();
  await expect(spell(page, "Shield")).toBeVisible();
  await expect(spell(page, "Magic Missile")).toHaveCount(0);
});

test("a spell always prepared is cast free once a day too", { tag: "@spells.prepared-free" }, async ({ page }) => {
  await wren(page, false);
  await openSheet(page, "Spells", "Wren");
  // Listed both ways: always prepared, and its free cast.
  await expect(spell(page, "Shield")).toHaveCount(2);
  await expect(spell(page, "Shield").first()).toContainText("always prepared");
  await expect(spell(page, "Shield").nth(1)).toContainText("1/1 free");
  await spell(page, "Shield").nth(1).click();
  await choose(page, "Shield", "Cast (a free cast)");
  await expect(spell(page, "Shield").nth(1)).toContainText("0/1 free");
});

test("a spell cast with a resource spends it", { tag: "@spells.cast-resource" }, async ({ page }) => {
  await wren(page, true);
  await openSheet(page, "Spells", "Wren");
  await expect(spell(page, "Jump")).toContainText("1 Binding");
  await act(page, "Jump", "Cast (1 Binding)");
  await expect(status(page)).toContainText("Cast Jump (1 Binding)");
  await sheetTab(page, "Overview");
  const bindings = page.locator(".tracker").filter({ has: page.locator(".tname", { hasText: "Bindings" }) });
  await expect(bindings.locator(".count")).toHaveText("1/2");
});


test("a rule grants a spell and a choice of spells", { tag: "@rules.grant-spells" }, async ({ page }) => {
  await wren(page, false, true);
  await openSheet(page, "Spells", "Wren");
  await expect(spell(page, "Light")).toBeVisible();
  const under = await spell(page, "Light").evaluate((b) => {
    for (let el: Element | null = b.closest(".srow"); el; el = el.previousElementSibling) {
      const t = el.querySelector?.(".header .title") ?? (el.classList.contains("header") ? el.querySelector(".title") : null);
      if (t) return t.textContent?.trim();
    }
    return null;
  });
  expect(under).toBe("Patron's Gifts");
  await page.locator("button.choice", { hasText: "choose 1 Wizard" }).click();
  await pick(page, ["Sleep"]);
  await expect(page.locator("button.choice")).toHaveCount(0);
  // Chosen, it is cast free once a day.
  await expect(spell(page, "Sleep")).toContainText("1/1 free");
});
