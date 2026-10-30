// Updating dnt: checking for a newer one, and installing it (up to the
// system's installer), with the tests' releases (e2e/update).
import type { Page } from "@playwright/test";
import { expect, go, invoke, TARGET, test, useTestRelease } from "./fixtures";

/// The release file this app updates with.
const FILE = TARGET === "windows" ? "dnt-app-windows-x86_64-setup.exe" : "dnt-app-android.apk";
const updates = (page: Page) => page.locator("section.pane", { has: page.locator(".title", { hasText: "Updates" }) });

test("a newer dnt and what is new in it", { tag: "@update.check" }, async ({ page }) => {
  await useTestRelease(page, "newer");
  await go(page, "Data", "About");
  await updates(page).getByRole("button", { name: "Check for updates" }).click();
  await expect(updates(page)).toContainText("dnt 99.0.0 is out.");
  await expect(updates(page)).toContainText("Everything the tests need.");
  // Nothing newer: it says so.
  await useTestRelease(page, "older");
  await updates(page).getByRole("button", { name: "Check for updates" }).click();
  await expect(updates(page)).toContainText(/✓ dnt [\d.]+ is the newest/);
});

test("updating downloads the newer dnt and checks it", { tag: "@update.install" }, async ({ page }) => {
  // A download that does not match its SHA256SUMS is not installed.
  await useTestRelease(page, "broken");
  await go(page, "Data", "About");
  await updates(page).getByRole("button", { name: "Check for updates" }).click();
  await updates(page).getByRole("button", { name: "Update to 99.0.1" }).click();
  await expect(updates(page).locator(".err")).toContainText(`${FILE} does not match its SHA256SUMS: not installed`);
  // The real one, up to the system's installer.
  await useTestRelease(page, "newer");
  await updates(page).getByRole("button", { name: "Check for updates" }).click();
  await updates(page).getByRole("button", { name: "Update to 99.0.0" }).click();
  await expect(updates(page)).toContainText(`dnt 99.0.0 downloaded and checked (${FILE}); the tests do not install it`);
});

test("the daily check says a newer dnt is out, and can be turned off", { tag: "@update.auto" }, async ({ page }) => {
  await useTestRelease(page, "newer");
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  const notice = page.getByRole("status").filter({ hasText: "dnt 99.0.0 is out" });
  await expect(notice).toBeVisible();
  await notice.getByRole("button", { name: "What's new" }).click();
  await expect(page.getByRole("dialog", { name: "What's new in dnt 99.0.0" })).toContainText("Everything the tests need.");
  await page.keyboard.press("Escape");
  await notice.getByRole("button", { name: "Close the notice" }).click();
  await expect(notice).toHaveCount(0);
  // Once a day: not again today.
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Data", "About");
  await expect(page.getByRole("status").filter({ hasText: "is out" })).toHaveCount(0);
  // Off: it checks no more.
  await updates(page).getByRole("checkbox", { name: "Check for a newer dnt once a day" }).uncheck();
  expect((await invoke<{ auto: boolean }>(page, "update_state")).auto).toBe(false);
});
