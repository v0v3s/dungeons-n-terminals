import { expect, fakeFilePicker, go, invoke, test } from "./fixtures";

/// A step of getting started, by its title.
const step = (page: import("@playwright/test").Page, title: string) =>
  page.locator("section.pane").filter({ has: page.locator("h2.title", { hasText: title }) });

test.describe("a new app", () => {
  test.use({ seed: "empty" });

  test("the first start imports the data zip, then asks for a campaign", { tag: "@setup.import-first-run" }, async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Getting started" })).toBeVisible();
    await expect(step(page, "Import the data")).toContainText("ships no game content");
    await fakeFilePicker(page);
    await page.getByRole("button", { name: "Import a zip…" }).click();
    await expect(step(page, "Import the data")).toContainText(/Imported \d+ new/);
    await expect(step(page, "Create a campaign").getByPlaceholder("the campaign's name")).toBeVisible();
    await expect(page.getByRole("button", { name: "Create the campaign" })).toBeDisabled();
  });
});

test.describe("with the data", () => {
  test.use({ seed: "data" });

  test("the first campaign is created and becomes the active one", { tag: "@setup.first-campaign" }, async ({ page }) => {
    await page.getByPlaceholder("the campaign's name").fill("E2E First");
    await page.getByRole("button", { name: "Create the campaign" }).click();
    await expect(step(page, "Create a campaign")).toContainText("E2E First is ready");
    await page.getByRole("button", { name: "Create a character" }).click();
    await expect(page.getByRole("button", { name: "+ New character" })).toBeVisible();
    await go(page, "Campaigns");
    const card = page.locator("section.card", { hasText: "E2E First" });
    await expect(card.locator(".badge")).toHaveText("active");
    expect((await invoke<{ campaign: string }>(page, "status")).campaign).toBe("E2E First");
  });
});
