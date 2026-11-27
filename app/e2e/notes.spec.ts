import type { Page } from "@playwright/test";
import { answer, choose, dialog, expect, go, invoke, test } from "./fixtures";

const row = (page: Page, name: string) => page.locator("button.row", { hasText: name });
/// The notes' toolbar (+ Note, Quick note, …, ⋯): over the folders on a
/// phone, at the top of the note's side on a computer.
const bar = (page: Page) => page.getByRole("toolbar", { name: "Notes" });
/// A phone or a tablet: the folders are sections, a folder's menu on its title.
const sectioned = (page: Page) => page.evaluate(() => !matchMedia("(pointer: fine) and (min-width: 761px)").matches);
const folderRow = (page: Page, name: string) => page.locator(".tree button.dir", { hasText: name });

/// Something done to a folder: from its title's menu on a phone or a
/// tablet, else chosen in the tree and from the toolbar's ⋯.
async function folderAction(page: Page, name: string, action: string) {
  await folderRow(page, name).click();
  if (!(await sectioned(page))) await bar(page).getByRole("button", { name: "More" }).click();
  await choose(page, name, action);
}

test("deleting a note and a folder asks first; declining keeps them", { tag: "@notes.delete" }, async ({ page }) => {
  await invoke(page, "note_create", { folder: "", title: "Doomed" });
  await invoke(page, "note_create_folder", { parent: "", name: "Attic" });
  await page.reload();
  await go(page, "Notes");

  await row(page, "Doomed").click();
  await page.locator(".ntools").getByRole("button", { name: "More" }).click();
  await choose(page, "Doomed", "Delete…");
  await expect(dialog(page, "Delete Doomed")).toContainText("gone for good");
  await choose(page, "Delete Doomed", "Cancel");
  expect(await invoke<[string, string][]>(page, "note_search", { query: "Doomed" })).toHaveLength(1);
  await page.locator(".ntools").getByRole("button", { name: "More" }).click();
  await choose(page, "Doomed", "Delete…");
  await choose(page, "Delete Doomed", "Delete");
  await expect(row(page, "Doomed")).toHaveCount(0);

  await folderAction(page, "Attic", "Delete Attic…");
  await choose(page, "Delete Attic", "Delete");
  await expect(folderRow(page, "Attic")).toHaveCount(0);
});

test("a top-level folder moves up and down", { tag: "@notes.folder-order" }, async ({ page }) => {
  await invoke(page, "note_create_folder", { parent: "", name: "Alpha" });
  await invoke(page, "note_create_folder", { parent: "", name: "Beta" });
  await page.reload();
  await go(page, "Notes");
  const order = async () => (await page.locator(".tree button.dir").allTextContents()).map((t) => t.replace(/[▾▸]/, "").trim());
  const before = await order();
  expect(before.indexOf("Beta")).toBeGreaterThan(before.indexOf("Alpha"));
  await folderAction(page, "Beta", "Move Beta up");
  await expect.poll(async () => (await order()).indexOf("Beta") < (await order()).indexOf("Alpha")).toBe(true);
});

test("a note and a folder are renamed", { tag: "@notes.rename" }, async ({ page }) => {
  await invoke(page, "note_create_folder", { parent: "", name: "Beta" });
  await invoke(page, "note_create", { folder: "", title: "Draft" });
  await page.reload();
  await go(page, "Notes");
  await folderAction(page, "Beta", "Rename Beta");
  await answer(page, "Rename Beta", "Gamma");
  await expect(folderRow(page, "Gamma")).toBeVisible();
  await expect(folderRow(page, "Beta")).toHaveCount(0);

  await row(page, "Draft").click();
  await page.locator(".ntools").getByRole("button", { name: "More" }).click();
  await choose(page, "Draft", "Rename");
  await answer(page, "Rename", "Final");
  // The open note's pane shows its path (on a phone it hides the tree).
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

test("the notes tree: folders open and close, a note opens", { tag: "@notes.browse" }, async ({ page }) => {
  await go(page, "Notes");
  for (const f of ["Sessions", "NPCs", "Quests", "Places", "Party", "Factions", "Lore"]) await expect(folderRow(page, f)).toBeVisible();
  await expect(file(page, "Campaign")).toBeVisible();
  await expect(file(page, "Inbox")).toBeVisible();
  // Party/Party.md, inside an open folder.
  const party = file(page, "Party");
  await expect(party).toBeVisible();
  if (await sectioned(page)) {
    // Sections: a folder's title is its menu; it is always open.
    await folderRow(page, "Party").click();
    await expect(dialog(page, "Party")).toContainText("New note in Party");
    await page.keyboard.press("Escape");
    await expect(party).toBeVisible();
    await file(page, "Campaign").click();
    await expect(note(page)).toContainText("Campaign.md");
    await expect(note(page).locator(".read")).toContainText("E2E");
    await toTree(page);
    await party.click();
    await expect(note(page)).toContainText("Party/Party.md");
    return;
  }
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
  await bar(page).getByRole("button", { name: "More" }).click();
  await choose(page, "Notes", "New folder");
  await answer(page, "New folder", "Downtime");
  await expect(folderRow(page, "Downtime")).toBeVisible();

  // A note in it, from the generic template: from its menu on a phone (the
  // new folder is chosen on a computer: + Note puts it there).
  if (await sectioned(page)) await folderAction(page, "Downtime", "New note in Downtime");
  else await bar(page).getByRole("button", { name: /\+ Note/ }).click();
  await answer(page, "New note in Downtime", "Crafting");
  await expect(note(page)).toContainText("Downtime/Crafting.md");
  await expect(note(page).locator("textarea")).toHaveValue(/# Crafting/);

  await toTree(page);
  // A phone: from the folder's menu (still empty, it has no + of its own).
  if (await sectioned(page)) await folderAction(page, "NPCs", "New note in NPCs");
  else {
    await folderRow(page, "NPCs").click();
    await bar(page).getByRole("button", { name: /\+ Note/ }).click();
  }
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
  await bar(page).getByRole("button", { name: "More" }).click();
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
  if (await sectioned(page)) {
    await page.getByRole("button", { name: "New note in Sessions" }).click();
    await expect(note(page)).toContainText(`Sessions/${path} (2).md`);
    return;
  }
  // Sessions chosen (a tap on the chosen folder chooses its parent): the
  // button says where its note goes.
  const plus = bar(page).getByRole("button", { name: /\+ Note/ });
  await expect(async () => {
    if (!(await plus.textContent())?.includes("Sessions")) await folderRow(page, "Sessions").click();
    await expect(plus).toContainText("Sessions", { timeout: 1000 });
  }).toPass({ timeout: 5000 });
  await plus.click();
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

test("a note is edited and saved, then read", { tag: ["@notes.edit", "@notes.save-reads"] }, async ({ page }) => {
  await go(page, "Notes");
  await file(page, "Inbox").click();
  await note(page).getByRole("button", { name: "Edit" }).click();
  const text = note(page).locator("textarea");
  const before = await text.inputValue();
  await text.fill(before + "\nThe bridge to the mill is out.\n");
  await expect(note(page)).toContainText("Inbox.md");
  await expect(note(page)).toContainText("●");
  await note(page).getByRole("button", { name: "Save" }).click();
  await expect(note(page)).not.toContainText("●");
  // Saved: back to reading it.
  await expect(text).toHaveCount(0);
  await expect(note(page).locator(".read")).toContainText("The bridge to the mill is out.");
  // Saved: the file has it.
  const saved = await invoke<{ text: string }>(page, "note_read", { path: "Inbox.md" });
  expect(saved.text).toContain("The bridge to the mill is out.");
});

test("links go to notes and to the compendium; a missing note is made from its link; back returns", { tag: "@notes.links" }, async ({ page }) => {
  await seedNotes(page, {
    "Bridge.md": "# Bridge\n\nAsk [[Old Miller|the miller]] about [[spell:Ray of Frost]] and [[Lost Mill]].\n",
    "NPCs/Old Miller.md": "# Old Miller\n\nKeeps the key.\n",
  });
  await file(page, "Bridge").click();
  const read = note(page).locator(".read");
  await read.getByRole("link", { name: "the miller" }).click();
  await expect(note(page)).toContainText("NPCs/Old Miller.md");
  await expect(note(page).locator(".read")).toContainText("Keeps the key.");
  await back(page).click();
  await expect(note(page)).toContainText("Bridge.md");

  await read.getByRole("link", { name: "Ray of Frost" }).click();
  await expect(dialog(page, "Ray of Frost")).toBeVisible();
  await expect(dialog(page, "Ray of Frost")).toContainText("1d8 Cold damage");
  await page.keyboard.press("Escape");
  await expect(dialog(page, "Ray of Frost")).toHaveCount(0);

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

test("a note's tasks are checkboxes: ticking one changes the note", { tag: "@notes.tasks" }, async ({ page }) => {
  await seedNotes(page, { "Quests/Lost Ring.md": ["# Lost Ring", "", "<!-- a comment", "over lines -->", "- [ ] Ask the miller", "- [x] Find the mill", "- A plain item", ""].join("\n") });
  await go(page, "Notes");
  await file(page, "Lost Ring").click();
  const read = note(page).locator(".read");
  const ask = read.getByRole("checkbox", { name: "Ask the miller" });
  const find = read.getByRole("checkbox", { name: "Find the mill" });
  await expect(ask).not.toBeChecked();
  await expect(find).toBeChecked();
  await expect(read.getByRole("checkbox")).toHaveCount(2);
  await ask.click();
  await expect(ask).toBeChecked();
  await expect.poll(async () => (await invoke<{ text: string }>(page, "note_read", { path: "Quests/Lost Ring.md" })).text).toContain("- [x] Ask the miller");
  // By its text too, and back.
  await read.getByText("Find the mill").click();
  await expect(find).not.toBeChecked();
  const saved = (await invoke<{ text: string }>(page, "note_read", { path: "Quests/Lost Ring.md" })).text;
  expect(saved).toContain("- [ ] Find the mill");
  expect(saved).toContain(["<!-- a comment", "over lines -->"].join("\n"));
  await expect(note(page)).not.toContainText("●");
});

test("Enter goes on with a list, and on an empty item ends it", { tag: "@notes.list-enter" }, async ({ page }) => {
  await seedNotes(page, { "Lists.md": "# Lists\n" });
  await go(page, "Notes");
  await file(page, "Lists").click();
  await note(page).getByRole("button", { name: "Edit" }).click();
  const text = note(page).locator("textarea");
  await text.click();
  await text.press("Control+End");
  await text.pressSequentially("- bread");
  await text.press("Enter");
  await text.pressSequentially("milk");
  await text.press("Enter");
  // An empty item: Enter ends the list.
  await text.press("Enter");
  await text.pressSequentially("1. one");
  await text.press("Enter");
  await text.pressSequentially("two");
  await text.press("Enter");
  await text.press("Enter");
  await text.pressSequentially("- [x] packed");
  await text.press("Enter");
  await text.pressSequentially("unpacked");
  await expect.poll(async () => (await text.inputValue()).trimEnd()).toBe(["# Lists", "- bread", "- milk", "1. one", "2. two", "- [x] packed", "- [ ] unpacked"].join("\n"));
});

test("on a phone the folders are sections, a note a section too", { tag: ["@notes.phone", "@android"] }, async ({ page }) => {
  await seedNotes(page, {
    ...Object.fromEntries(["Ana", "Bors", "Cato", "Dara", "Eld"].map((n) => [`NPCs/${n}.md`, `---\ntype: npc\n---\n# ${n}\n`])),
    "Quests/Lost Ring.md": "---\ntype: quest\nstatus: active\n---\n# Lost Ring\n",
  });
  const tree = page.locator(".tree");
  await expect(tree).toHaveClass(/\bsection\b/);
  // The search a prompt line with + note, quick note and ⋯ on it.
  const line = page.locator(".searchline");
  await expect(line.getByPlaceholder("search notes")).toBeVisible();
  await expect(line.getByRole("button", { name: "+ note" })).toBeVisible();
  // A folder: its title, its count and + on one line; the first few notes,
  // the rest a tap away; a note's status or type at its right.
  const npcs = page.locator(".fsec", { has: page.locator("button.dir", { hasText: "NPCs" }) });
  await expect(npcs.locator(".fhead")).toHaveText(/^NPCs\s*5\s*\+$/);
  await expect(npcs.getByRole("button", { name: "New note in NPCs" })).toBeVisible();
  await expect(npcs.locator("button.row")).toHaveCount(3);
  await expect(npcs.locator("button.row").first()).toContainText("npc");
  await npcs.getByRole("button", { name: "2 more ▾" }).click();
  await expect(npcs.locator("button.row")).toHaveCount(5);
  await expect(file(page, "Lost Ring")).toContainText("active");
  // Empty folders on one line at the end.
  await expect(page.locator(".fhead.empty")).toContainText("Places");
  await expect(page.locator(".fhead.empty")).not.toContainText("NPCs");

  // A note: its name the section's title, read/edit and ⋯ on it, its path
  // under it; an empty list is just its title.
  await file(page, "Campaign").click();
  const n = note(page);
  await expect(n).toHaveClass(/\bsection\b/);
  await expect(n.locator(".title")).toHaveText("Campaign");
  await expect(n.locator(".aside").getByRole("button", { name: "edit" })).toBeVisible();
  await expect(n.locator(".where")).toHaveText("Campaign.md");
  await expect(n).not.toContainText("Nothing yet.");
});
