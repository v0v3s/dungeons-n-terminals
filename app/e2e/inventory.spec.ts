import { dialog, expect, openSheet, test } from "./fixtures";

// In order: the pack is unpacked once, then its backpack is used.
test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ page }) => openSheet(page, "Inventory"));

const line = (page: import("@playwright/test").Page, name: string) => page.locator("button.line", { hasText: name });

test("a pack can be unpacked into its container", async ({ page }) => {
  await line(page, "Scholar's Pack").click();
  await dialog(page, "Scholar's Pack").getByRole("button", { name: "Unpack" }).click();
  await expect(page.locator(".status")).toContainText("Unpacked Scholar's Pack");
  await expect(line(page, "Backpack")).toBeVisible();
  await expect(line(page, "Ink Pen")).toBeVisible();
  await expect(line(page, "Scholar's Pack")).toHaveCount(0);
});

test("a container folds away its contents", async ({ page }) => {
  await page.getByRole("button", { name: "Fold" }).click();
  await expect(line(page, "Ink Pen")).toHaveCount(0);
  await page.getByRole("button", { name: "Unfold" }).click();
  await expect(line(page, "Ink Pen")).toBeVisible();
});

test("a container offers to add into it and its settings", async ({ page }) => {
  await line(page, "Backpack").click();
  const menu = dialog(page, "Backpack");
  await expect(menu.getByRole("button", { name: "Add an item into it…" })).toBeVisible();
  await expect(menu.getByRole("button", { name: "Add a custom item into it…" })).toBeVisible();
  await menu.getByRole("button", { name: "Container settings…" }).click();
  await expect(dialog(page, "Backpack: container").getByRole("textbox")).toBeVisible();
});

test("a custom item goes into the container", async ({ page }) => {
  await line(page, "Backpack").click();
  await dialog(page, "Backpack").getByRole("button", { name: "Add a custom item into it…" }).click();
  await dialog(page, "A custom item").getByRole("textbox").fill("Lucky Pebble");
  await page.keyboard.press("Enter");
  // Inside the backpack: indented under it.
  const pebble = page.locator(".lrow", { hasText: "Lucky Pebble" });
  await expect(pebble).toBeVisible();
  expect(await pebble.evaluate((e) => parseFloat(getComputedStyle(e).paddingLeft))).toBeGreaterThan(0);
});

test("a weapon is not a container or a monk weapon for a wizard", async ({ page }) => {
  await line(page, "Dagger").click();
  const menu = dialog(page, "Dagger");
  await expect(menu.getByRole("button", { name: "Make it a container…" })).toBeVisible();
  await expect(menu.getByRole("button", { name: /monk weapon/ })).toHaveCount(0);
});

test("+ Homebrew lists the item templates", async ({ page }) => {
  await page.getByRole("button", { name: "+ Homebrew" }).click();
  await expect(dialog(page, "A new homebrew item").getByRole("button").first()).toBeVisible();
});
