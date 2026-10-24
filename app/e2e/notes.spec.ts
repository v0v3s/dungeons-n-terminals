import { dialog, expect, go, invoke, test } from "./fixtures";
import type { Page } from "@playwright/test";

const row = (page: Page, name: string) => page.locator("button.row", { hasText: name });

async function openNotes(page: Page) {
  await page.reload();
  await go(page, "Notes");
}

test("deleting a note asks first", async ({ page }) => {
  await invoke(page, "note_create", { folder: "", title: "Doomed" });
  await openNotes(page);
  await row(page, "Doomed").click();
  await page.locator(".note").getByRole("button", { name: "More" }).click();
  await dialog(page, "Doomed").getByRole("button", { name: "Delete…" }).click();

  // Cancel keeps it.
  const confirm = dialog(page, "Delete Doomed");
  await expect(confirm).toContainText("gone for good");
  await confirm.getByRole("button", { name: "Cancel" }).click();
  expect(await invoke<[string, string][]>(page, "note_search", { query: "Doomed" })).toHaveLength(1);

  await page.locator(".note").getByRole("button", { name: "More" }).click();
  await dialog(page, "Doomed").getByRole("button", { name: "Delete…" }).click();
  await dialog(page, "Delete Doomed").getByRole("button", { name: "Delete" }).click();
  await expect(row(page, "Doomed")).toHaveCount(0);
});

test("a folder can be moved and renamed from the tree's menu", async ({ page }) => {
  await invoke(page, "note_create_folder", { parent: "", name: "Alpha" });
  await invoke(page, "note_create_folder", { parent: "", name: "Beta" });
  await openNotes(page);
  const order = async () => (await page.locator("button.row.dir").allTextContents()).map((t) => t.replace(/[▾▸]/, "").trim());
  expect((await order()).indexOf("Beta")).toBeGreaterThan((await order()).indexOf("Alpha"));

  await row(page, "Beta").click();
  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await dialog(page, "Beta").getByRole("button", { name: "Move Beta up" }).click();
  await expect.poll(async () => (await order()).indexOf("Beta") < (await order()).indexOf("Alpha")).toBe(true);

  await page.locator(".tree").getByRole("button", { name: "More" }).click();
  await dialog(page, "Beta").getByRole("button", { name: "Rename Beta" }).click();
  await dialog(page, "Rename Beta").getByRole("textbox").fill("Gamma");
  await page.keyboard.press("Enter");
  await expect(row(page, "Gamma")).toBeVisible();
  await expect(row(page, "Beta")).toHaveCount(0);
});
