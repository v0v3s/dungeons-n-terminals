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
  const menu = page.getByRole("button", { name: "Menu" });
  if (await menu.isVisible()) await menu.click();
  // The desktop header numbers them ("2 Characters").
  await page.getByRole("button", { name: new RegExp(`^(\\d )?${section}$`) }).filter({ visible: true }).first().click();
}

/// Open a character's sheet (Tester by default) on a tab.
export async function openSheet(page: Page, tab: "Overview" | "Features" | "Spells" | "Inventory" = "Overview", name = "Tester") {
  // Another sheet open: close it (the app reopens an open character
  // after a reload) and start from a fresh frontend, on the roster.
  if (await page.locator(".sheet").count()) {
    await invoke(page, "close_character");
    await page.reload();
  }
  await go(page, "Characters");
  await page.locator("button.card", { hasText: name }).click();
  await page.getByRole("button", { name: tab, exact: true }).click();
}

/// The open dialog with this title.
export const dialog = (page: Page, title: string | RegExp) => page.getByRole("dialog", { name: title });

/// Choose an action from the menu dialog titled `title`.
export async function choose(page: Page, title: string | RegExp, action: string | RegExp) {
  await dialog(page, title).getByRole("button", { name: action, exact: typeof action === "string" }).click();
}

/// Answer the prompt titled `title` with `text`.
export async function answer(page: Page, title: string | RegExp, text: string) {
  const box = dialog(page, title).getByRole("textbox");
  await box.fill(text);
  await box.press("Enter");
}

/// Pick these choices in the open picker, then confirm a multiple pick.
export async function pick(page: Page, labels: string[]) {
  const picker = page.getByRole("dialog").last();
  for (const label of labels) {
    // A long list has a filter.
    const filter = picker.getByRole("textbox");
    if (await filter.count()) await filter.fill(label);
    await picker.locator("li button", { hasText: label }).first().click();
  }
  const done = picker.getByRole("button", { name: "done" });
  if (await done.isVisible()) await done.click();
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
