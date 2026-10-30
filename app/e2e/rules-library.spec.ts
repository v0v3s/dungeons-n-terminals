// The rules library: a tree of folders, files and rules to tick and import
// as your own rules (the tests' library: e2e/rules).
import type { Page } from "@playwright/test";
import { dialog, expect, go, invoke, openSheet, test, useTestRulesLibrary } from "./fixtures";

const library = (page: Page) => dialog(page, "Rules library");
/// A row's tick (a folder, a file or a rule), by its name.
const tick = (page: Page, name: string) => library(page).getByRole("checkbox", { name });
/// Unfold a folder or a file.
const unfold = (page: Page, name: string) => library(page).getByRole("button", { name: `Unfold ${name}` }).click();
/// The overview's tile with this label.
const tile = (page: Page, label: string) => page.locator("button.tile", { hasText: label });

async function open(page: Page) {
  await useTestRulesLibrary(page);
  await go(page, "Data", "Rules");
  await page.getByRole("button", { name: "Open the rules library…" }).click();
  await expect(tick(page, "SRD")).toBeVisible();
}

/// Your own rules for the Wizard's Ritual Adept: 10 more feet of speed.
async function ownRules(page: Page) {
  const mine = '[[feature]]\nid = "classFeature:Ritual Adept|Wizard|SRD52|1"\n[[feature.effect]]\ntarget = "speed"\nadd = "10"\n';
  await invoke(page, "e2e_put_file", { path: "overlays/mine.toml", text: mine });
}

test("the library is a tree of folders, files and rules", { tag: "@rules.library-browse" }, async ({ page }) => {
  await open(page);
  // Folded at first: the folders, the SRD first, then the homebrew.
  const names = await library(page).locator(".folder .name").allInnerTexts();
  expect(names).toEqual(["SRD", "Homebrew"]);
  await expect(tick(page, "SRD")).toContainText("3 rule(s)");
  await expect(tick(page, "Classes")).toHaveCount(0);
  // Unfolded: its files, folded too; then a file's rules.
  await unfold(page, "SRD");
  await expect(tick(page, "Classes")).toContainText("2 rule(s)");
  await expect(tick(page, "Ritual Adept (Wizard 1)")).toHaveCount(0);
  await unfold(page, "Classes");
  await expect(tick(page, "Ritual Adept (Wizard 1)")).toContainText("new");
  await expect(tick(page, "Favored Enemy (Ranger 1)")).toContainText("new · replaces dnt's own");
  // And folded again.
  await library(page).getByRole("button", { name: "Fold SRD" }).click();
  await expect(tick(page, "Classes")).toHaveCount(0);
});

test("ticked rules are imported and the sheet uses them", { tag: "@rules.library-import" }, async ({ page }) => {
  await open(page);
  await unfold(page, "SRD");
  await unfold(page, "Classes");
  // A whole file: its rules are ticked, its folder partly.
  await tick(page, "Classes").click();
  await expect(tick(page, "Ritual Adept (Wizard 1)")).toHaveAttribute("aria-checked", "true");
  await expect(tick(page, "SRD")).toHaveAttribute("aria-checked", "mixed");
  await library(page).getByRole("button", { name: "Import 2 rule(s)" }).click();
  await expect(library(page)).toContainText("✓ Imported 2 rule(s)");
  await expect(tick(page, "Ritual Adept (Wizard 1)")).toContainText("yours already");
  await page.keyboard.press("Escape");
  // Ritual Adept's (invented) five feet.
  await openSheet(page);
  await expect(tile(page, "Speed")).toContainText("35");
});

test("rules that differ from yours ask to overwrite or keep", { tag: "@rules.library-conflict" }, async ({ page }) => {
  await ownRules(page);
  await open(page);
  await unfold(page, "SRD");
  await unfold(page, "Classes");
  await expect(tick(page, "Ritual Adept (Wizard 1)")).toContainText("differs from yours");
  await tick(page, "SRD").click();
  await library(page).getByRole("button", { name: "Import 3 rule(s)" }).click();
  await expect(library(page)).toContainText("These differ from your rules");
  const choice = library(page).getByRole("group", { name: "Ritual Adept (Wizard 1)" });
  // Kept unless chosen; all at once, or one by one.
  await expect(choice.getByRole("button", { name: "Keep" })).toHaveAttribute("aria-pressed", "true");
  await library(page).getByRole("button", { name: "Overwrite all" }).click();
  await expect(choice.getByRole("button", { name: "Overwrite" })).toHaveAttribute("aria-pressed", "true");
  await library(page).getByRole("button", { name: "Keep all" }).click();
  await expect(choice.getByRole("button", { name: "Keep" })).toHaveAttribute("aria-pressed", "true");
  await choice.getByRole("button", { name: "Overwrite" }).click();
  await library(page).getByRole("button", { name: "Import", exact: true }).click();
  await expect(library(page)).toContainText("✓ Imported 3 rule(s), 1 of yours overwritten");
  await page.keyboard.press("Escape");
  await openSheet(page);
  await expect(tile(page, "Speed")).toContainText("35");
});

test("kept rules stay yours", { tag: "@rules.library-conflict" }, async ({ page }) => {
  await ownRules(page);
  await open(page);
  await unfold(page, "SRD");
  await tick(page, "Classes").click();
  await library(page).getByRole("button", { name: "Import 2 rule(s)" }).click();
  await library(page).getByRole("button", { name: "Import", exact: true }).click();
  await expect(library(page)).toContainText("✓ Imported 1 rule(s), 1 of yours kept");
  await page.keyboard.press("Escape");
  await openSheet(page);
  await expect(tile(page, "Speed")).toContainText("40");
});
