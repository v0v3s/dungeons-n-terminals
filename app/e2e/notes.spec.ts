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
