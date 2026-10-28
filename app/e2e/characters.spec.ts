import type { Page } from "@playwright/test";
import { choose, dialog, expect, fakeFilePicker, go, invoke, openSheet, pick, sheetMore, sheetTab, status, test } from "./fixtures";

const card = (page: Page, name: string) => page.locator("button.card", { hasText: name });

/// Choose an action from the sheet's ⋯ menu.
async function more(page: Page, name: string, action: string) {
  await sheetMore(page);
  await choose(page, name, action);
}

/// The character's file as the app shows it, changed by `edit`, saved.
async function editFile(page: Page, name: string, edit: (text: string) => string) {
  await more(page, name, "Edit the character file…");
  const box = dialog(page, `${name} · the file`).getByRole("textbox");
  await box.fill(edit(await box.inputValue()));
  await dialog(page, `${name} · the file`).getByRole("button", { name: "Save" }).click();
  await expect(status(page)).toContainText("Saved the character");
}

test("the roster lists the party with their levels", { tag: "@roster.list" }, async ({ page }) => {
  await go(page, "Characters");
  await expect(page.locator("button.card .name")).toHaveText(["Brawler", "Hexa", "Ranger", "Tester"]);
  await expect(card(page, "Brawler")).toContainText("Stoneling · Drifter · Monk 1");
  await expect(card(page, "Hexa")).toContainText("Emberkin; Ash Lineage · Acolyte of Gears · Hexer 1");
  await expect(card(page, "Tester")).toContainText("Kindred · Archivist · Mage 1");
  await expect(card(page, "Tester")).toContainText("Level 1");
});

test("deleting a character asks first; declining keeps them", { tag: "@roster.delete" }, async ({ page }) => {
  await openSheet(page, "Overview", "Ranger");
  await more(page, "Ranger", "Delete the character…");
  await expect(dialog(page, "Delete Ranger")).toContainText("gone for good");
  await choose(page, "Delete Ranger", "Cancel");
  await expect(page.locator(".sheet h1")).toContainText("Ranger");

  await more(page, "Ranger", "Delete the character…");
  await choose(page, "Delete Ranger", "Delete");
  await expect(card(page, "Tester")).toBeVisible();
  await expect(card(page, "Ranger")).toHaveCount(0);
  await expect(page.locator("button.card")).toHaveCount(3);
});

test("editing the character's file changes the sheet", { tag: "@character.edit-file" }, async ({ page }) => {
  await openSheet(page);
  await editFile(page, "Tester", (t) => t.replace("str = 15", "str = 20"));
  await sheetTab(page, "Skills");
  await expect(page.locator('.abil .score[data-of="Strength"]')).toHaveText("20");
  await expect(page.locator("button.num:not(.save)").first()).toContainText("+5");
});

test("a broken file is not saved and says why", { tag: "@character.edit-file" }, async ({ page }) => {
  await openSheet(page);
  await more(page, "Tester", "Edit the character file…");
  const file = dialog(page, "Tester · the file");
  await file.getByRole("textbox").fill("name = ");
  await file.getByRole("button", { name: "Save" }).click();
  await expect(file).toBeVisible();
  await expect(file.locator(".file-err")).not.toBeEmpty();
  await file.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".sheet h1")).toContainText("Tester");
});

test("a character file from a newer dnt is not read", { tag: "@character.newer-file" }, async ({ page }) => {
  await invoke(page, "e2e_put_file", { path: "characters/later.toml", text: 'format = 2\nname = "Later"\n' });
  await go(page, "Characters");
  await expect(page.locator(".roster .err")).toContainText("Could not read later.toml: written by a newer dnt");
  await expect(page.locator(".card", { hasText: "Later" })).toHaveCount(0);
  await expect(page.locator(".card", { hasText: "Tester" })).toBeVisible();
});

test("the file editor will not save a newer dnt's file", { tag: "@character.newer-file" }, async ({ page }) => {
  await openSheet(page);
  await more(page, "Tester", "Edit the character file…");
  const file = dialog(page, "Tester · the file");
  await file.getByRole("textbox").fill('format = 2\nname = "Tester"\n');
  await file.getByRole("button", { name: "Save" }).click();
  await expect(file.locator(".file-err")).toContainText("written by a newer dnt");
  await file.getByRole("button", { name: "Cancel" }).click();
  await expect(page.locator(".sheet h1")).toContainText("Tester");
});

test("the sheet exports as a printable page and as Markdown", { tag: "@character.export" }, async ({ page }) => {
  await openSheet(page);
  await fakeFilePicker(page);
  for (const [action, file] of [
    ["Export as a printable page…", "Tester.html"],
    ["Export as Markdown…", "Tester.md"],
  ]) {
    await more(page, "Tester", action);
    await expect(status(page)).toContainText("Exported Tester");
    const text = await invoke<string>(page, "e2e_read", { path: `e2e:${file}` });
    expect(text).toContain("Tester");
    expect(text).toContain("Kindred");
    if (file.endsWith(".html")) expect(text).toMatch(/<html/i);
    else expect(text).toMatch(/^# /m);
  }
});

test("the character's own note opens", { tag: "@character.note" }, async ({ page }) => {
  await openSheet(page);
  await more(page, "Tester", "Open the character's note");
  await expect(page.locator(".note")).toContainText("Tester.md");

  await openSheet(page);
  await sheetTab(page, "Notes");
  await expect(page.locator(".note")).toContainText("Tester.md");
});

test("what the rules could not work out shows as warnings", { tag: "@character.warnings" }, async ({ page }) => {
  await openSheet(page);
  await expect(page.getByRole("button", { name: "Warnings" })).toHaveCount(0);
  // Into a class the scores do not allow.
  await more(page, "Tester", "Level up…");
  await pick(page, ["New class: Hexer"]);
  // A Hexer chooses a subclass at level 1.
  await pick(page, ["The Lantern"]);
  await pick(page, ["Average"]);
  await dialog(page, "Level up").getByRole("button", { name: "Apply the level" }).click();
  await expect(page.locator(".sheet .id")).toContainText("Hexer 1");
  await page.getByRole("button", { name: "Warnings" }).click();
  await expect(dialog(page, "Warnings")).toContainText("multiclassing Hexer requires");
});

test("on a phone the app bar has the name, the level and the sheet's menu", { tag: ["@sheet.app-bar", "@android"] }, async ({ page }) => {
  await openSheet(page);
  await expect(page.locator(".bar .name")).toHaveText("Tester · Lv 1");
  await expect(page.locator(".sheet .head")).toBeHidden();
  await expect(page.locator(".sheet .icontabs").getByRole("tab")).toHaveCount(6);
  await sheetMore(page);
  await expect(dialog(page, "Tester")).toBeVisible();
});
