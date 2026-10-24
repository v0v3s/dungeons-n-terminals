import { dialog, expect, openSheet, test } from "./fixtures";

// In order: the spells are chosen once, then used.
test.describe.configure({ mode: "serial" });

test.beforeEach(async ({ page }) => openSheet(page, "Spells"));

test("a new wizard chooses cantrips from the list", async ({ page }) => {
  const choose = page.locator("button.choice", { hasText: "cantrip" }).first();
  const what = (await choose.textContent()) ?? "";
  const count = Number(what.match(/choose (\d+)/)?.[1] ?? 1);
  await choose.click();
  const picker = page.getByRole("dialog").last();
  for (let i = 0; i < count; i++) await picker.locator("li button").nth(i).click();
  await expect(page.locator("button.spell")).toHaveCount(count);
});

test("a cantrip rolls with the wizard's numbers", async ({ page }) => {
  await page.locator("button.spell").first().click();
  await page.getByRole("dialog").last().getByRole("button", { name: "Roll it" }).click();
  await expect(page.locator(".status.roll")).toBeVisible();
});

test("a spell's menu reads it", async ({ page }) => {
  const first = page.locator("button.spell").first();
  const name = (await first.locator(".name").textContent())?.trim() ?? "";
  await first.click();
  await dialog(page, name).getByRole("button", { name: "Read it" }).click();
  await expect(page.getByRole("dialog").last()).toContainText(name);
});
