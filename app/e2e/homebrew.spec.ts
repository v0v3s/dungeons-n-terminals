import type { Locator, Page } from "@playwright/test";
import { answer, choose, dialog, expect, go, invoke, phone, test } from "./fixtures";

test.use({ seed: "campaign" });

const exact = (name: string) => new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
const names = (page: Page) => page.locator(".rows .name");
const heading = (page: Page) => page.locator(".detail h1");
const doc = (page: Page) => page.locator(".detail .doc");

/// On a phone the entry hides the list: back to it.
async function toList(page: Page) {
  if (await phone(page)) {
    const back = page.getByRole("button", { name: "Back", exact: true });
    while (await back.isVisible()) await back.click();
  }
}

async function search(page: Page, query: string) {
  await toList(page);
  await page.locator(".compendium .prompt input").filter({ visible: true }).fill(query);
}

/// Tap a result to read it.
async function read(page: Page, name: string) {
  await toList(page);
  await page.locator(".rows button", { has: page.locator(".name", { hasText: exact(name) }) }).first().click();
  await expect(heading(page)).toContainText(name);
}

/// A new spell in the campaign's homebrew, open in the editor.
async function newSpell(page: Page, name: string): Promise<Locator> {
  await go(page, "Compendium");
  await page
    .getByRole("button", { name: "New homebrew entry" })
    .or(page.getByRole("button", { name: "+ new" }))
    .filter({ visible: true })
    .first()
    .click();
  await choose(page, "New homebrew entry", "Spell");
  await answer(page, "New spell", name);
  const editor = dialog(page, `Edit · ${name}`);
  await expect(editor).toBeVisible();
  return editor;
}

/// Clone a book entry into the campaign as its campaign version, open in the editor.
async function campaignVersion(page: Page, name: string): Promise<Locator> {
  await go(page, "Compendium");
  await search(page, name.toLowerCase());
  await read(page, name);
  await page.getByRole("button", { name: "Clone into the campaign" }).click();
  await choose(page, `Clone ${name}`, /A campaign version/);
  const editor = dialog(page, `Edit · ${name}`);
  await expect(editor).toContainText("campaign version");
  return editor;
}

const field = (editor: Locator, label: string) => editor.locator("button.field", { has: editor.page().locator(".name", { hasText: exact(label) }) });

/// Write a field's new text and save it.
async function setText(page: Page, editor: Locator, label: string, text: string) {
  await field(editor, label).click();
  await dialog(page, label).getByRole("textbox").fill(text);
  await dialog(page, label).getByRole("button", { name: "Save" }).click();
  await expect(dialog(page, label)).toHaveCount(0);
}

test("a field of a homebrew entry is edited, and the compendium shows it", { tag: "@homebrew.edit-field" }, async ({ page }) => {
  const editor = await newSpell(page, "Moth Whisper");
  await setText(page, editor, "Description", "Moths carry your words to a friend.");
  await expect(field(editor, "Description")).toContainText("Moths carry your words");
  await setText(page, editor, "Name", "Moth Message");
  await expect(dialog(page, "Edit · Moth Message")).toBeVisible();
  await dialog(page, "Edit · Moth Message").getByRole("button", { name: "Done" }).click();

  await search(page, "moth");
  await expect(names(page)).toHaveText(["Moth Message"]);
  await read(page, "Moth Message");
  await expect(doc(page)).toContainText("Moths carry your words to a friend.");
});

test("a homebrew entry is edited as JSON", { tag: "@homebrew.edit-json" }, async ({ page }) => {
  const editor = await newSpell(page, "Moth Whisper");
  await editor.getByRole("button", { name: "Edit as JSON" }).click();
  const json = dialog(page, "Moth Whisper · JSON");
  const box = json.getByRole("textbox");
  const entry = JSON.parse(await box.inputValue());
  expect(entry.name).toBe("Moth Whisper");
  entry.entries = ["Written in JSON, a moth flutters."];
  entry.level = 2;
  await box.fill(JSON.stringify(entry, null, 2));
  await json.getByRole("button", { name: "Save" }).click();
  await expect(json).toHaveCount(0);
  await expect(field(editor, "Description")).toContainText("Written in JSON");
  await editor.getByRole("button", { name: "Done" }).click();

  await search(page, "moth whisper");
  await read(page, "Moth Whisper");
  await expect(doc(page)).toContainText("Written in JSON, a moth flutters.");
  await search(page, "moth ^2");
  await expect(names(page)).toHaveText(["Moth Whisper"]);
});

test("a changed field of a campaign version goes back to the book", { tag: "@homebrew.revert-field" }, async ({ page }) => {
  const editor = await campaignVersion(page, "Glimmer");
  await setText(page, editor, "Description", "A pale moon rises in your palm.");
  await expect(field(editor, "Description")).toHaveClass(/changed/);
  await field(editor, "Description").click();
  await dialog(page, "Description").getByRole("button", { name: "Back to the book" }).click();
  await expect(dialog(page, "Description")).toHaveCount(0);
  await expect(field(editor, "Description")).not.toHaveClass(/changed/);
  await expect(field(editor, "Description")).toContainText("An object sheds dim light.");
  await editor.getByRole("button", { name: "Done" }).click();

  await read(page, "Glimmer");
  await expect(doc(page)).toContainText("An object sheds dim light.");
  await expect(doc(page)).not.toContainText("A pale moon");
});

test("a field is added to a homebrew entry", { tag: "@homebrew.add-field" }, async ({ page }) => {
  const editor = await newSpell(page, "Moth Whisper");
  await editor.getByRole("button", { name: "+ Field" }).click();
  await answer(page, "New field", "damageInflict");
  await answer(page, "damageInflict =", '["cold"]');
  await expect(field(editor, "damageInflict")).toContainText("cold");
  await editor.getByRole("button", { name: "Done" }).click();

  // The compendium finds it by its new damage type.
  await search(page, "moth *cold ");
  await expect(names(page)).toHaveText(["Moth Whisper"]);
});

test("a campaign entry is made global: every campaign has it", { tag: "@homebrew.promote" }, async ({ page }) => {
  const editor = await newSpell(page, "Moth Whisper");
  await expect(editor).toContainText("campaign homebrew");
  await editor.getByRole("button", { name: "Make global" }).click();
  await expect(editor).toContainText("global homebrew: every campaign has it");
  await expect(editor.getByRole("button", { name: "Make global" })).toHaveCount(0);
  await editor.getByRole("button", { name: "Done" }).click();

  // Another campaign has it too.
  const made = await invoke<{ campaigns: { name: string; slug: string }[] }>(page, "create_campaign", { name: "E2E Two", sources: [] });
  const two = made.campaigns.find((c) => c.name === "E2E Two")!;
  await invoke(page, "activate_campaign", { slug: two.slug });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Compendium");
  await search(page, "moth whisper");
  await expect(names(page)).toHaveText(["Moth Whisper"]);
});

test("a homebrew entry is deleted after confirming", { tag: "@homebrew.delete" }, async ({ page }) => {
  const editor = await newSpell(page, "Moth Whisper");
  await editor.getByRole("button", { name: "Delete" }).click();
  const confirm = dialog(page, "Delete Moth Whisper");
  await expect(confirm).toContainText("this campaign's homebrew");
  await confirm.getByRole("button", { name: "Cancel" }).click();
  await expect(confirm).toHaveCount(0);
  await expect(editor).toBeVisible();

  await editor.getByRole("button", { name: "Delete" }).click();
  await confirm.getByRole("button", { name: "Delete" }).click();
  await expect(editor).toHaveCount(0);
  await search(page, "moth whisper");
  await expect(page.locator(".rows")).toContainText("Nothing matches.");
});
