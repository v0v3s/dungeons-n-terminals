// The test fixtures, the same on every target (DNT_E2E_TARGET: android or
// windows). Each test starts from a known state, made through the app's own
// commands: the app reset, the test data imported (e2e/data), the campaign
// and the test party (e2e/characters.json), unless it asks for less.
import { test as base, chromium, expect, type Locator, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import * as android from "./android";
import * as windows from "./windows";

export const TARGET = (process.env.DNT_E2E_TARGET ?? "android") as "android" | "windows";
const E2E = fileURLToPath(new URL("../../e2e", import.meta.url));
const PARTY = JSON.parse(readFileSync(join(E2E, "characters.json"), "utf8")) as {
  campaign: string;
  characters: { name: string; prefer: string[] }[];
};
export const CAMPAIGN = PARTY.campaign;

/// Call one of the app's commands, as the frontend does.
export function invoke<T = unknown>(page: Page, cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  return page.evaluate(([cmd, args]) => (window as any).__TAURI_INTERNALS__.invoke(cmd, args), [cmd, args] as const) as Promise<T>;
}

/// The test data zip (made by the runner script from e2e/data and e2e/homebrew).
export function dataZip(): Buffer {
  const zip = process.env.DNT_E2E_ZIP;
  if (!zip) throw new Error("DNT_E2E_ZIP: the test data zip (scripts/e2e-app.sh sets it)");
  return readFileSync(zip);
}

/// Import the test data: its bytes go with the call, as no file the tests
/// write can be read by the app on a phone.
export async function importData(page: Page) {
  await page.evaluate((b64) => {
    const bytes = Array.from(atob(b64), (c) => c.charCodeAt(0));
    return (window as any).__TAURI_INTERNALS__.invoke("e2e_import_zip", { bytes });
  }, dataZip().toString("base64"));
}

type Choice = { label: string; value: string };
type Question =
  | { type: "pick"; choices: Choice[]; count: number }
  | { type: "number" }
  | { type: "abilities"; scores: { scores: number[] } }
  | { type: "confirm" };

/// Make a character through character creation, answering as
/// e2e/characters.json describes (the same as the terminal's harness).
export async function createCharacter(page: Page, name: string, prefer: string[]) {
  let step = await invoke<{ question: Question }>(page, "creation_start", { name });
  for (let i = 0; i < 60; i++) {
    const q = step.question;
    if (q.type === "confirm") {
      await invoke(page, "creation_finish");
      await invoke(page, "close_character");
      return;
    }
    let answer;
    if (q.type === "pick") {
      const wanted = (l: string) => prefer.some((p) => p === l || (p.endsWith(":") && l.startsWith(p)));
      const values = q.choices.filter((c) => wanted(c.label)).slice(0, q.count).map((c) => c.value);
      for (const c of q.choices) if (values.length < q.count && !values.includes(c.value)) values.push(c.value);
      answer = { type: "pick", value: values };
    } else if (q.type === "abilities") {
      answer = { type: "scores", value: q.scores.scores };
    } else {
      answer = { type: "number", value: 1 };
    }
    step = await invoke(page, "creation_answer", { answer });
  }
  throw new Error(`Creating ${name} did not finish`);
}

/// What a test starts from: nothing, the data, the campaign, or the party.
export type Seed = "empty" | "data" | "campaign" | "party";

async function seed(page: Page, what: Seed) {
  // The reset wipes everything: never on a device with someone's campaigns.
  const status = await invoke<{ campaigns: { name: string }[] }>(page, "status");
  const others = status.campaigns.map((c) => c.name).filter((n) => !n.startsWith(CAMPAIGN));
  if (others.length && !process.env.DNT_E2E_RESET) {
    throw new Error(
      `The app on this device has campaigns (${others.join(", ")}), and the tests reset it. ` +
        "Use a test device, or set DNT_E2E_RESET=1 to wipe them.",
    );
  }
  await invoke(page, "reset_app");
  // What the frontend remembers (folded panes, the compendium's order) too.
  await page.evaluate(() => localStorage.clear());
  if (what !== "empty") await importData(page);
  if (what === "campaign" || what === "party") await invoke(page, "create_campaign", { name: CAMPAIGN, sources: [] });
  if (what === "party") for (const c of PARTY.characters) await createCharacter(page, c.name, c.prefer);
  await page.reload();
  await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
}

export const test = base.extend<{ page: Page; seed: Seed }, { app: Page }>({
  seed: ["party", { option: true }],
  app: [
    async ({}, use) => {
      const endpoint = TARGET === "windows" ? await windows.connect() : await android.connect();
      const browser = await chromium.connectOverCDP(endpoint);
      // The WebView's page may come a moment after its DevTools port.
      const context = browser.contexts()[0];
      const page = context.pages()[0] ?? (await context.waitForEvent("page"));
      // A test that does not answer the file dialogs cancels them: the
      // system's own would stop the run (fakeFilePicker answers them).
      await context.addInitScript(() => ((window as any).dntE2eFiles = { open: null, save: null }));
      // The frontend is up when it can call the app.
      await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
      await use(page);
      await browser.close();
      if (TARGET === "windows") windows.stop();
    },
    // Starting the app (and its WebView) the first time takes a while.
    { scope: "worker", timeout: 120_000 },
  ],
  page: async ({ app, seed: what }, use, info) => {
    await seed(app, what);
    await use(app);
    if (info.status !== info.expectedStatus) {
      await info.attach("screen", { body: await app.screenshot(), contentType: "image/png" });
    }
  },
});

export { expect };

/// A phone: a narrow screen with a drawer and bottom sheets.
export const phone = (page: Page) => page.evaluate(() => matchMedia("(max-width: 760px)").matches);

/// Go to a section: through the menu (☰) on a phone, the header on a desktop.
export async function go(page: Page, section: "Compendium" | "Characters" | "Notes" | "Campaigns" | "Data") {
  const menu = page.getByRole("button", { name: "Menu", exact: true });
  if (await menu.isVisible()) await menu.click();
  // The desktop header numbers them ("2 Characters").
  // A section, not something inside one (the character's Notes tab).
  const button = page.getByRole("button", { name: new RegExp(`^(\\d )?${section}$`) }).and(page.locator(":not(.child)"));
  await button.filter({ visible: true }).first().click();
}

/// Open a character's sheet (Tester by default) on a tab.
export async function openSheet(page: Page, tab: Exclude<SheetTab, "Notes"> = "Overview", name = "Tester") {
  // A character open (shown or not): close it, as the app shows an open
  // character's sheet in Characters, and start from a fresh frontend.
  if (await invoke(page, "sheet").then(() => true, () => false)) {
    await invoke(page, "close_character");
    await page.reload();
    await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
  }
  await go(page, "Characters");
  await page.locator("button.card", { hasText: name }).click();
  await sheetTab(page, tab);
}

export type SheetTab = "Overview" | "Skills" | "Spells" | "Inventory" | "Features" | "Notes";

/// Change the open sheet's tab (Notes opens the character's note): its
/// icon on a phone, its select elsewhere.
export async function sheetTab(page: Page, tab: SheetTab) {
  if (await phone(page)) await page.locator(".sheet .icontabs").getByRole("tab", { name: tab, exact: true }).click();
  else await page.locator(".sheet .tabs select").selectOption(tab);
}

/// The open sheet's tab, as its icons or its select show it.
export async function expectTab(page: Page, tab: SheetTab) {
  if (await phone(page)) await expect(page.locator('.sheet .icontabs [aria-selected="true"]')).toHaveAttribute("aria-label", tab);
  else await expect(page.locator(".sheet .tabs select")).toHaveValue(tab);
}

/// The sheet's own ⋯ menu: in the app bar on a phone, by the name elsewhere.
export async function sheetMore(page: Page) {
  await page.locator(".bar, .sheet .head").getByRole("button", { name: "More", exact: true }).filter({ visible: true }).first().click();
}

/// The open dialog with this title.
export const dialog = (page: Page, title: string | RegExp) => page.getByRole("dialog", { name: title });

/// Choose an action from the menu dialog titled `title`.
export async function choose(page: Page, title: string | RegExp, action: string | RegExp) {
  await dialog(page, title).getByRole("button", { name: action, exact: typeof action === "string" }).click();
}

/// An action from the ⋯ menu of the compendium entry being read.
export async function entryAction(page: Page, name: string, action: string) {
  await page.locator(".detail .title").getByRole("button", { name: "More" }).click();
  await menu(page, name, action);
}

/// An action from the ⋯ menu of a campaign's card.
export async function campaignAction(page: Page, name: string, action: string) {
  await page.locator("section.card").filter({ has: page.locator(".name", { hasText: new RegExp(`^${name}$`) }) }).getByRole("button", { name: "More" }).click();
  await menu(page, name, action);
}

/// An action from the ⋯ menu titled exactly `title` (a dialog still
/// closing may have a longer title with it in it).
async function menu(page: Page, title: string, action: string) {
  await page.getByRole("dialog", { name: title, exact: true }).getByRole("button", { name: action, exact: true }).click();
}

/// Answer the prompt titled `title` with `text`.
export async function answer(page: Page, title: string | RegExp, text: string) {
  const box = dialog(page, title).getByRole("textbox");
  await box.fill(text);
  await box.press("Enter");
}

/// Pick these choices in the open picker, then confirm a multiple pick.
export async function pick(page: Page, labels: string[]) {
  // A dialog closing as this one opens is still there while it fades: wait
  // for the transitions (Web Animations) to end, or "last" may be that one.
  await page
    .waitForFunction(() => document.getAnimations().every((a) => a.playState !== "running"), null, { timeout: 2000 })
    .catch(() => {});
  // The picker offering the first choice (the question before may still
  // be up while the app works out the next), else the last one open.
  const offering = page.getByRole("dialog").filter({ has: page.locator("li button", { hasText: labels[0] }) });
  await offering.last().waitFor({ timeout: 5000 }).catch(() => {});
  const picker = (await offering.count()) ? offering.last() : page.getByRole("dialog").last();
  // A multiple pick says "0 of N chosen"; one of all N goes on by itself,
  // fewer need "done".
  const counter = picker.locator(".count");
  const of = (await counter.count()) ? Number((await counter.first().textContent())?.match(/of (\d+)/)?.[1] ?? 1) : 1;
  for (const label of labels) {
    // A long list has a filter.
    const filter = picker.getByRole("textbox");
    if (await filter.count()) await filter.fill(label);
    await picker.locator("li button", { hasText: label }).first().click();
  }
  if (labels.length < of) await picker.getByRole("button", { name: "done" }).click();
  // Answered: wait for the next question (the app may take a moment), so
  // the next pick does not find this one.
  const last = labels[labels.length - 1];
  if (last) await expect(picker.locator("li button", { hasText: last })).toHaveCount(0, { timeout: 5000 }).catch(() => {});
}

/// The status line under the sheet (messages and rolls).
export const status = (page: Page): Locator => page.locator(".status");

/// Answer the app's file dialogs instead of the system (whose own dialogs
/// cannot be driven): opening picks the test data, handed over to the app
/// first; saving picks `e2e:<the name it suggests>`, read back with
/// `invoke(page, "e2e_read", { path: "e2e:<name>" })`.
export async function fakeFilePicker(page: Page) {
  await page.evaluate((b64) => {
    const bytes = Array.from(atob(b64), (c) => c.charCodeAt(0));
    return (window as any).__TAURI_INTERNALS__.invoke("e2e_stash_zip", { bytes });
  }, dataZip().toString("base64"));
  await page.evaluate(() => ((window as any).dntE2eFiles = { open: "e2e:stash", save: "e2e:" }));
}
