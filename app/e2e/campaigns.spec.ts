import type { Page } from "@playwright/test";
import { CAMPAIGN, answer, choose, dialog, expect, go, invoke, test } from "./fixtures";

/// A campaign's card, by its exact name.
const card = (page: Page, name: string) =>
  page.locator("section.card").filter({ has: page.locator(".name", { hasText: new RegExp(`^${name}$`) }) });

/// Create a campaign as a user does, with all the books.
async function create(page: Page, name: string) {
  await go(page, "Campaigns");
  await page.getByRole("button", { name: "+ New campaign" }).click();
  await dialog(page, "New campaign").getByRole("textbox").fill(name);
  await choose(page, "New campaign", "Create");
  await expect(card(page, name)).toBeVisible();
}

test("another campaign is created", { tag: "@campaigns.create" }, async ({ page }) => {
  await create(page, "E2E Second");
  await expect(card(page, CAMPAIGN)).toBeVisible();
  await expect(card(page, "E2E Second")).toContainText("all official");
  expect((await invoke<{ campaigns: { name: string }[] }>(page, "status")).campaigns.map((c) => c.name)).toContain("E2E Second");
});

test("making another campaign active shows its characters and notes", { tag: "@campaigns.activate" }, async ({ page }) => {
  await invoke(page, "note_create", { folder: "", title: "Party Plans" });
  await create(page, "E2E Second");
  // A new campaign does not take over the active one.
  await expect(card(page, CAMPAIGN).locator(".badge")).toHaveText("active");
  await card(page, "E2E Second").getByRole("button", { name: "Make active" }).click();
  await expect(card(page, "E2E Second").locator(".badge")).toHaveText("active");
  await go(page, "Characters");
  await expect(page.getByRole("button", { name: "+ New character" })).toBeVisible();
  await expect(page.locator("button.card", { hasText: "Tester" })).toHaveCount(0);
  await go(page, "Notes");
  await expect(page.locator("button.row", { hasText: "Party Plans" })).toHaveCount(0);

  await go(page, "Campaigns");
  await card(page, CAMPAIGN).getByRole("button", { name: "Make active" }).click();
  await expect(card(page, CAMPAIGN).locator(".badge")).toHaveText("active");
  await expect(card(page, "E2E Second").locator(".badge")).toHaveCount(0);
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
  await go(page, "Notes");
  await expect(page.locator("button.row", { hasText: "Party Plans" })).toBeVisible();
});

test("a renamed campaign stays active", { tag: "@campaigns.rename" }, async ({ page }) => {
  await go(page, "Campaigns");
  await card(page, CAMPAIGN).getByRole("button", { name: "Rename" }).click();
  await answer(page, `Rename ${CAMPAIGN}`, "E2E Renamed");
  await expect(card(page, "E2E Renamed").locator(".badge")).toHaveText("active");
  await expect(card(page, CAMPAIGN)).toHaveCount(0);
  // Its characters are still there.
  await go(page, "Characters");
  await expect(page.locator("button.card", { hasText: "Tester" })).toBeVisible();
  expect((await invoke<{ campaign: string }>(page, "status")).campaign).toBe("E2E Renamed");
});

test("deleting a campaign asks first; declining keeps it", { tag: "@campaigns.delete" }, async ({ page }) => {
  await create(page, "E2E Doomed");
  await card(page, "E2E Doomed").getByRole("button", { name: "Delete" }).click();
  await expect(dialog(page, "Delete E2E Doomed")).toContainText("cannot be undone");
  await choose(page, "Delete E2E Doomed", "Cancel");
  await expect(card(page, "E2E Doomed")).toBeVisible();

  await card(page, "E2E Doomed").getByRole("button", { name: "Delete" }).click();
  await choose(page, "Delete E2E Doomed", "Delete");
  await expect(card(page, "E2E Doomed")).toHaveCount(0);
  await expect(card(page, CAMPAIGN)).toBeVisible();
  expect((await invoke<{ campaigns: { name: string }[] }>(page, "status")).campaigns.map((c) => c.name)).toEqual([CAMPAIGN]);
});
