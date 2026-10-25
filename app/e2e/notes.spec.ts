import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, go, invoke, test } from "./fixtures";

const row = (page: Page, name: string) => page.locator("button.row", { hasText: name });

test("deleting a note and a folder asks first; declining keeps them", { tag: "@notes.delete" }, async ({ page }) => {
  await invoke(page, "note_create", { folder: "", title: "Doomed" });
  await invoke(page, "note_create_folder", { parent: "", name: "Attic" });
  await page.reload();
  await go(page, "Notes");

  await row(page, "Doomed").click();
  await page.locator(".note").getByRole("button", { name: "More" }).click();
  await choose(page, "Doomed", "Delete…");
  await expect(dialog(page, "Delete Doomed")).toContainText("gone for good");
  await choose(page, "Delete Doomed", "Cancel");
  expect(await invoke<[string, string][]>(page, "note_search", { query: "Doomed" })).toHaveLength(1);
  await page.locator(".note").getByRole("button", { name: "More" }).click();
  await choose(page, "Doomed", "Delete…");
  await choose(page, "Delete Doomed", "Delete");
  await expect(row(page, "Doomed")).toHaveCount(0);

  await row(page, "Attic").click();
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await choose(page, "Attic", "Delete Attic…");
  await choose(page, "Delete Attic", "Delete");
  await expect(row(page, "Attic")).toHaveCount(0);
});

test("a top-level folder moves up and down", { tag: "@notes.folder-order" }, async ({ page }) => {
  await invoke(page, "note_create_folder", { parent: "", name: "Alpha" });
  await invoke(page, "note_create_folder", { parent: "", name: "Beta" });
  await page.reload();
  await go(page, "Notes");
  const order = async () => (await page.locator("button.row.dir").allTextContents()).map((t) => t.replace(/[▾▸]/, "").trim());
  const before = await order();
  expect(before.indexOf("Beta")).toBeGreaterThan(before.indexOf("Alpha"));
  await row(page, "Beta").click();
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await choose(page, "Beta", "Move Beta up");
  await expect.poll(async () => (await order()).indexOf("Beta") < (await order()).indexOf("Alpha")).toBe(true);
});

test("a note and a folder are renamed", { tag: "@notes.rename" }, async ({ page }) => {
  await invoke(page, "note_create_folder", { parent: "", name: "Beta" });
  await invoke(page, "note_create", { folder: "", title: "Draft" });
  await page.reload();
  await go(page, "Notes");
  await row(page, "Beta").click();
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await choose(page, "Beta", "Rename Beta");
  await answer(page, "Rename Beta", "Gamma");
  await expect(row(page, "Gamma")).toBeVisible();
  await expect(row(page, "Beta")).toHaveCount(0);

  await row(page, "Draft").click();
  await page.locator(".note").getByRole("button", { name: "More" }).click();
  await choose(page, "Draft", "Rename");
  await answer(page, "Rename", "Final");
  // The open note's pane is titled with its path (on a phone it hides the tree).
  await expect(page.locator(".note")).toContainText("Final.md");
});

/// Write notes as they are on disk (seeding only).
async function seedNotes(page: Page, notes: Record<string, string>) {
  for (const [path, text] of Object.entries(notes)) await invoke(page, "note_write", { path, text });
  await page.reload();
  await go(page, "Notes");
}

/// The open note's pane (titled with its path).
const note = (page: Page) => page.locator(".note");
/// Back: the app bar's on a phone, the note's own on a desktop.
const back = (page: Page) => page.getByRole("button", { name: /^(Back|‹ back)$/ }).filter({ visible: true }).first();

/// Back to the folder tree (a phone shows the note instead of it).
async function toTree(page: Page) {
  for (let i = 0; i < 5 && !(await page.locator(".tree").isVisible()); i++) await back(page).click();
  await expect(page.locator(".tree")).toBeVisible();
}

const file = (page: Page, name: string) => page.locator("button.row:not(.dir)", { hasText: name });
const folderRow = (page: Page, name: string) => page.locator("button.row.dir", { hasText: name });

test("the notes tree: folders open and close, a note opens", { tag: "@notes.browse" }, async ({ page }) => {
  await go(page, "Notes");
  for (const f of ["Sessions", "NPCs", "Quests", "Places", "Party", "Factions", "Lore"]) await expect(folderRow(page, f)).toBeVisible();
  await expect(file(page, "Campaign")).toBeVisible();
  await expect(file(page, "Inbox")).toBeVisible();
  // Party/Party.md, inside an open folder.
  const party = file(page, "Party");
  await expect(party).toBeVisible();
  // The first tap chooses the folder, the next closes it.
  await folderRow(page, "Party").click();
  await folderRow(page, "Party").click();
  await expect(party).toHaveCount(0);
  await expect(folderRow(page, "Party")).toContainText("▸");
  await folderRow(page, "Party").click();
  await expect(party).toBeVisible();
  await expect(folderRow(page, "Party")).toContainText("▾");

  await file(page, "Campaign").click();
  await expect(note(page)).toContainText("Campaign.md");
  await expect(note(page).locator(".read")).toContainText("E2E");
  await toTree(page);
  await party.click();
  await expect(note(page)).toContainText("Party/Party.md");
});

test("a note is made from its folder's template, and a folder is made", { tag: "@notes.new" }, async ({ page }) => {
  await go(page, "Notes");
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await choose(page, "Notes", "New folder");
  await answer(page, "New folder", "Downtime");
  await expect(folderRow(page, "Downtime")).toBeVisible();

  // The new folder is chosen: a note goes in it, from the generic template.
  await page.locator(".tree").getByRole("button", { name: /\+ Note/ }).click();
  await answer(page, "New note in Downtime", "Crafting");
  await expect(note(page)).toContainText("Downtime/Crafting.md");
  await expect(note(page).locator("textarea")).toHaveValue(/# Crafting/);

  await toTree(page);
  await folderRow(page, "NPCs").click();
  await page.locator(".tree").getByRole("button", { name: /\+ Note/ }).click();
  await answer(page, "New note in NPCs", "Old Miller");
  await expect(note(page)).toContainText("NPCs/Old Miller.md");
  const text = note(page).locator("textarea");
  await expect(text).toHaveValue(/type: npc/);
  await expect(text).toHaveValue(/# Old Miller/);
  await expect(text).toHaveValue(/\*\*Wants:\*\*/);
  await toTree(page);
  await expect(file(page, "Old Miller")).toBeVisible();
});

test("today's session note is made from the template", { tag: "@notes.session" }, async ({ page }) => {
  await go(page, "Notes");
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await choose(page, "Notes", "Today's session note");
  await expect(note(page)).toContainText(/Sessions\/\d{4}-\d{2}-\d{2}\.md/);
  const path = (await note(page).textContent())!.match(/Sessions\/(\d{4}-\d{2}-\d{2})\.md/)![1];
  const text = note(page).locator("textarea");
  await expect(text).toHaveValue(/type: session/);
  await expect(text).toHaveValue(new RegExp(`date: ${path}`));
  await expect(text).toHaveValue(new RegExp(`# ${path}`));
  await expect(text).toHaveValue(/## Recap/);
  await toTree(page);
  await expect(file(page, path)).toBeVisible();

  // A note in Sessions is a session note too: the next one of the day.
  await folderRow(page, "Sessions").click();
  await page.locator(".tree").getByRole("button", { name: /\+ Note/ }).click();
  await expect(note(page)).toContainText(`Sessions/${path} (2).md`);
});

test("a quick note lands in the Inbox with the time", { tag: "@notes.capture" }, async ({ page }) => {
  await go(page, "Notes");
  await page.getByRole("button", { name: "Quick note" }).click();
  await answer(page, "Quick note", "The miller lies about the key");
  await expect(page.locator(".notes .status")).toContainText("Captured to Inbox");
  await file(page, "Inbox").click();
  await expect(note(page).locator(".read")).toContainText(/\d{4}-\d{2}-\d{2} \d{2}:\d{2} The miller lies about the key/);
});

test("a note is edited and saved", { tag: "@notes.edit" }, async ({ page }) => {
  await go(page, "Notes");
  await file(page, "Inbox").click();
  await note(page).getByRole("button", { name: "Edit" }).click();
  const text = note(page).locator("textarea");
  const before = await text.inputValue();
  await text.fill(before + "\nThe bridge to the mill is out.\n");
  await expect(note(page)).toContainText("Inbox.md ●");
  await note(page).getByRole("button", { name: "Save" }).click();
  await expect(note(page)).not.toContainText("●");
  await note(page).getByRole("button", { name: "Read" }).click();
  await expect(note(page).locator(".read")).toContainText("The bridge to the mill is out.");
  // Saved: the file has it.
  const saved = await invoke<{ text: string }>(page, "note_read", { path: "Inbox.md" });
  expect(saved.text).toContain("The bridge to the mill is out.");
});

test("links go to notes and to the compendium; a missing note is made from its link; back returns", { tag: "@notes.links" }, async ({ page }) => {
  await seedNotes(page, {
    "Bridge.md": "# Bridge\n\nAsk [[Old Miller|the miller]] about [[spell:Ember Bolt]] and [[Lost Mill]].\n",
    "NPCs/Old Miller.md": "# Old Miller\n\nKeeps the key.\n",
  });
  await file(page, "Bridge").click();
  const read = note(page).locator(".read");
  await read.getByRole("link", { name: "the miller" }).click();
  await expect(note(page)).toContainText("NPCs/Old Miller.md");
  await expect(note(page).locator(".read")).toContainText("Keeps the key.");
  await back(page).click();
  await expect(note(page)).toContainText("Bridge.md");

  await read.getByRole("link", { name: "Ember Bolt" }).click();
  await expect(dialog(page, "Ember Bolt")).toBeVisible();
  await expect(dialog(page, "Ember Bolt")).toContainText("3d6");
  await page.keyboard.press("Escape");
  await expect(dialog(page, "Ember Bolt")).toHaveCount(0);

  await read.getByRole("link", { name: "Lost Mill" }).click();
  await choose(page, "“Lost Mill” does not exist yet", "Create it at the top level");
  await expect(note(page)).toContainText("Lost Mill.md");
  await back(page).click();
  await expect(note(page)).toContainText("Bridge.md");
  expect(await invoke<[string, string][]>(page, "note_search", { query: "Lost Mill" })).toHaveLength(1);
});

test("a note shows which notes link to it", { tag: "@notes.backlinks" }, async ({ page }) => {
  await seedNotes(page, {
    "Bridge.md": "# Bridge\n\nAsk [[Old Miller]].\n",
    "Places/Mill.md": "# Mill\n\nOwned by [[NPCs/Old Miller|him]].\n",
    "NPCs/Old Miller.md": "# Old Miller\n",
    "Lore/Unrelated.md": "# Unrelated\n",
  });
  await folderRow(page, "NPCs").click();
  await file(page, "Old Miller").click();
  const linked = note(page).locator(".backlinks");
  await expect(linked).toContainText("Linked from");
  await expect(linked.getByRole("button", { name: "Bridge" })).toBeVisible();
  await expect(linked.getByRole("button", { name: "Mill" })).toBeVisible();
  await expect(linked).not.toContainText("Unrelated");
  await linked.getByRole("button", { name: "Mill" }).click();
  await expect(note(page)).toContainText("Places/Mill.md");
});

test("query blocks are answered: the hub's lists and a note's own", { tag: "@notes.queries" }, async ({ page }) => {
  await seedNotes(page, {
    "Quests/Lost Ring.md": "---\ntype: quest\nstatus: active\n---\n# Lost Ring\n",
    "Quests/Old Debt.md": "---\ntype: quest\nstatus: done\n---\n# Old Debt\n",
    "NPCs/Old Miller.md": "---\ntype: npc\nstatus: alive\ntags: [villain]\n---\n# Old Miller\n",
    "NPCs/Dead Baron.md": "---\ntype: npc\nstatus: dead\n---\n# Dead Baron\n",
    "Sessions/2026-01-02.md": "---\ntype: session\n---\n# 2026-01-02\n",
    "Villains.md": "# Villains\n\n```dnt The bad ones\n#villain\n```\n",
  });
  await file(page, "Campaign").click();
  const query = (title: string) => note(page).locator(".query", { has: page.locator(".qtitle", { hasText: title }) });
  await expect(query("Open quests")).toContainText("Lost Ring");
  await expect(query("Open quests")).not.toContainText("Old Debt");
  await expect(query("People")).toContainText("Old Miller");
  await expect(query("People")).not.toContainText("Dead Baron");
  await expect(query("Recent sessions")).toContainText("2026-01-02");
  await query("Open quests").getByRole("button", { name: "Lost Ring" }).click();
  await expect(note(page)).toContainText("Quests/Lost Ring.md");

  await toTree(page);
  await file(page, "Villains").click();
  await expect(query("The bad ones")).toContainText("Old Miller");
  await expect(query("The bad ones").locator("button")).toHaveCount(1);
});

test("notes are searched by words, #tag, key:value and @Folder", { tag: "@notes.search" }, async ({ page }) => {
  await seedNotes(page, {
    "NPCs/Old Miller.md": "---\ntype: npc\nstatus: alive\ntags: [suspect]\n---\n# Old Miller\n",
    "NPCs/Dead Baron.md": "---\ntype: npc\nstatus: dead\n---\n# Dead Baron\n",
    "Places/Mill.md": "# Mill\n\nWho did it? #suspect\n",
  });
  const search = page.getByPlaceholder("search notes");
  const results = page.locator(".tree button.row");

  await search.fill("miller");
  await expect(results).toHaveCount(1);
  await expect(results).toContainText("Old Miller");
  await search.fill("#suspect");
  await expect(results).toHaveCount(2);
  await expect(results.filter({ hasText: "Old Miller" })).toHaveCount(1);
  await expect(results.filter({ hasText: "Places/Mill.md" })).toHaveCount(1);
  await search.fill("status:dead");
  await expect(results).toHaveCount(1);
  await expect(results).toContainText("Dead Baron");
  await search.fill("@Places");
  await expect(results).toHaveCount(1);
  await expect(results).toContainText("Mill");
  await search.fill("nothing like this");
  await expect(page.locator(".tree")).toContainText("Nothing matches.");

  await search.fill("baron");
  await results.first().click();
  await expect(note(page)).toContainText("NPCs/Dead Baron.md");
});
