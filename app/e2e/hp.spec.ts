import type { Page } from "@playwright/test";
import { answer, choose, expect, openSheet, status, test } from "./fixtures";

// Tester, a level 1 Wizard, has 8 hit points and one d6 hit die.
test.beforeEach(async ({ page }) => {
  await openSheet(page);
  // The pane folds by its title: open it.
  const fold = page.locator(".pane.hp button.fold");
  if ((await fold.getAttribute("aria-expanded")) === "false") await fold.click();
});

/// The hit points as shown: "8/8", with "+N temp" when there are some.
const hp = (page: Page) => page.locator(".hpnum");
const hpButton = (page: Page, name: "Damage" | "Heal" | "Temp") =>
  page.locator(".pane.hp").getByRole("button", { name, exact: true });
/// The death saves line, while dying or dead.
const death = (page: Page) => page.locator(".death");

async function damage(page: Page, n: number) {
  await hpButton(page, "Damage").click();
  await answer(page, "Damage", String(n));
}

test("damage takes the temporary hit points first, then the hit points; at 0 the character is dying", { tag: "@hp.damage" }, async ({ page }) => {
  await hpButton(page, "Temp").click();
  await answer(page, "Temporary hit points", "3");
  await damage(page, 2);
  await expect(hp(page)).toHaveText(/^\s*8\/8\s*\+1 temp$/);
  await damage(page, 4);
  await expect(hp(page)).toHaveText(/^\s*5\/8$/);
  await expect(death(page)).toHaveCount(0);
  await damage(page, 5);
  await expect(hp(page)).toHaveText(/^\s*0\/8$/);
  await expect(death(page)).toContainText("Death saves");
  await expect(death(page).getByRole("button", { name: "Roll" })).toBeVisible();
});

test("damage that kills outright asks: dead, or dying at 0", { tag: "@hp.damage-lethal" }, async ({ page }) => {
  // 8 hit points: 16 leaves 8 over, the maximum.
  await damage(page, 16);
  await choose(page, "16 damage kills outright", "Leave at 0 hit points, dying");
  await expect(hp(page)).toHaveText(/^\s*0\/8$/);
  await expect(death(page)).toContainText("Death saves");
  await expect(death(page)).not.toContainText("Dead");

  await hpButton(page, "Heal").click();
  await answer(page, "Heal", "8");
  await damage(page, 20);
  await choose(page, "20 damage kills outright", "Dead");
  await expect(death(page)).toContainText("Dead");
  await expect(death(page).getByRole("button", { name: "Not dead" })).toBeVisible();
});

test("healing stops at the maximum", { tag: "@hp.heal" }, async ({ page }) => {
  await damage(page, 5);
  await expect(hp(page)).toHaveText(/^\s*3\/8$/);
  await hpButton(page, "Heal").click();
  await answer(page, "Heal", "3");
  await expect(hp(page)).toHaveText(/^\s*6\/8$/);
  await hpButton(page, "Heal").click();
  await answer(page, "Heal", "20");
  await expect(hp(page)).toHaveText(/^\s*8\/8$/);
});

test("temporary hit points are added and do not stack", { tag: "@hp.temp" }, async ({ page }) => {
  await hpButton(page, "Temp").click();
  await answer(page, "Temporary hit points", "5");
  await expect(hp(page)).toHaveText(/^\s*8\/8\s*\+5 temp$/);
  // The higher stays.
  await hpButton(page, "Temp").click();
  await answer(page, "Temporary hit points", "2");
  await expect(hp(page)).toHaveText(/^\s*8\/8\s*\+5 temp$/);
});

test("a dying character rolls death saves; a dead one is marked not dead", { tag: "@hp.death-save" }, async ({ page }) => {
  await damage(page, 8);
  await expect(death(page)).toContainText("Death saves");
  const marks = async () => ((await death(page).locator(".ok, .bad").allTextContents()).join("").match(/●/g) ?? []).length;
  expect(await marks()).toBe(0);
  await death(page).getByRole("button", { name: "Roll" }).click();
  await expect(page.locator(".status.roll")).toContainText(/death save/i);
  // A success or a failure is marked, or a natural 20 brings 1 hit point back.
  await expect
    .poll(async () => (await marks()) > 0 || /^\s*1\/8$/.test(await hp(page).innerText()))
    .toBe(true);

  await hpButton(page, "Heal").click();
  await answer(page, "Heal", "8");
  await damage(page, 30);
  await choose(page, "30 damage kills outright", "Dead");
  await expect(death(page)).toContainText("Dead");
  await death(page).getByRole("button", { name: "Not dead" }).click();
  await expect(death(page)).not.toContainText("Dead");
  await expect(death(page)).toContainText("Death saves");
  expect(await marks()).toBe(0);
});

test("a hit die is rolled with Constitution and heals", { tag: "@roll.hit-die" }, async ({ page }) => {
  await damage(page, 7);
  await expect(hp(page)).toHaveText(/^\s*1\/8$/);
  const die = page.locator(".tracker", { hasText: "Hit dice" });
  await expect(die.locator(".count")).toHaveText("1/1");
  await die.getByRole("button", { name: /Roll d6 \+ Con, heal/ }).click();
  await expect(status(page)).toContainText(/d6/);
  await expect(die.locator(".count")).toHaveText("0/1");
  await expect(hp(page)).not.toHaveText(/^\s*1\/8$/);
  await expect(die.getByRole("button", { name: /Roll d6 \+ Con, heal/ })).toBeDisabled();
});
