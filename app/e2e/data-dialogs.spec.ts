// The app on a computer: Data's side menu, getting started, character
// creation; the wiki link on every target.
import type { Page } from "@playwright/test";
import { expect, go, invoke, opened, test, windowSize } from "./fixtures";

const WIKI = "https://github.com/v0v3s/dungeons-n-terminals/wiki/Files-and-data";
const menu = (page: Page) => page.getByRole("navigation", { name: "Data" });

/// How far right the main content reaches, as a share of the window.
const reach = (page: Page) =>
  page.evaluate(() => {
    let right = 0;
    for (const el of document.querySelectorAll("main *")) {
      const box = el.getBoundingClientRect();
      if (box.width && box.height) right = Math.max(right, box.right);
    }
    return right / document.documentElement.clientWidth;
  });

test("Data's parts are a menu on the left; ☰ hides it for the select", { tag: ["@data.side-menu", "@windows"] }, async ({ page }) => {
  await go(page, "Data");
  await expect(menu(page)).toBeVisible();
  await menu(page).getByRole("button", { name: "Backups" }).click();
  await expect(menu(page).getByRole("button", { name: "Backups" })).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("button", { name: "Back up now" })).toBeVisible();
  // The menu to the left of the page it shows.
  const nav = (await menu(page).boundingBox())!;
  const part = (await page.getByRole("button", { name: "Back up now" }).boundingBox())!;
  expect(part.x).toBeGreaterThan(nav.x + nav.width);
  // A problem: how many, by its name.
  await invoke(page, "e2e_put_file", { path: "overlays/mine.toml", text: '[[feature]]\nid = "feat:Nonesuch|TST"\n' });
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  await go(page, "Data");
  await expect(menu(page).getByRole("button", { name: /^Problems/ })).toHaveText(/Problems\s*1/);
  // ☰ hides it: the select instead.
  await page.getByRole("button", { name: "Hide the sidebar" }).click();
  await expect(menu(page)).toBeHidden();
  await expect(page.getByRole("combobox", { name: "Data" })).toBeVisible();
});

test.describe("getting started", () => {
  test.use({ seed: "empty" });

  test("it points to the wiki's page on making your own data", { tag: "@welcome.wiki-link" }, async ({ page }) => {
    // The link opens the page in the browser (caught: no browser opens).
    await page.getByRole("link", { name: "How to make your own data (wiki)" }).click();
    expect(await opened(page)).toEqual([WIKI]);
    // Or its address is copied, for reading it elsewhere (caught too).
    await page.evaluate(() => {
      (window as any).copied = null;
      navigator.clipboard.writeText = async (t: string) => void ((window as any).copied = t);
    });
    await page.getByRole("button", { name: "Copy link" }).click();
    expect(await page.evaluate(() => (window as any).copied)).toBe(WIKI);
    await expect(page.getByRole("button", { name: "Copied ✓" })).toBeVisible();
  });

  test("on a computer it uses the window: the welcome left, the steps right", { tag: ["@welcome.desktop", "@windows"] }, async ({ page }) => {
    const hello = (await page.getByRole("heading", { name: "Getting started" }).boundingBox())!;
    const steps = (await page.locator(".welcome .pane").first().boundingBox())!;
    expect(steps.x).toBeGreaterThan(hello.x + hello.width);
    await windowSize(page, { width: 2560, height: 1440 });
    expect(await reach(page)).toBeGreaterThan(0.9);
  });
});

test("a new character's choices are a grid between the steps and the choice read", { tag: ["@creation.desktop", "@windows"] }, async ({ page }) => {
  await go(page, "Characters");
  await page.getByRole("button", { name: "+ New character" }).click();
  const name = page.getByRole("dialog", { name: "New character" }).getByRole("textbox");
  await name.fill("Nova");
  await name.press("Enter");
  const creation = page.locator(".creation");
  await expect(creation).toContainText("Choose a species");
  // The choices: cards side by side.
  const cards = creation.locator(".question ul > li > button");
  const [a, b] = [(await cards.nth(0).boundingBox())!, (await cards.nth(1).boundingBox())!];
  expect(Math.abs(a.y - b.y)).toBeLessThan(2);
  // The steps left, the highlighted choice read on the right.
  const steps = (await creation.locator(".steps").boundingBox())!;
  const question = (await creation.locator(".question").boundingBox())!;
  const preview = (await creation.locator(".preview").boundingBox())!;
  expect(question.x).toBeGreaterThan(steps.x + steps.width - 1);
  expect(preview.x).toBeGreaterThan(question.x + question.width - 1);
  await cards.nth(0).hover();
  await expect(creation.locator(".preview")).not.toBeEmpty();
  // Back and Cancel at the foot.
  await expect(creation.locator(".nav").getByRole("button", { name: "Cancel" })).toBeVisible();
  await invoke(page, "creation_cancel");
});

test("on a big screen Data uses the window", { tag: ["@layout.wide", "@windows"] }, async ({ page }) => {
  await windowSize(page, { width: 2560, height: 1440 });
  await go(page, "Data");
  expect(await reach(page)).toBeGreaterThan(0.9);
});
