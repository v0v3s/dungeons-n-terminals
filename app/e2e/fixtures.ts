// The test fixtures: one connection to the app for the run, which starts
// from a known state (the app reset, the data imported, a campaign and a
// character), and a screenshot of the device when a test fails.
import { test as base, chromium, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { connect } from "./android";

/// Call one of the app's commands, as the frontend does.
export function invoke<T = unknown>(page: Page, cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  return page.evaluate(([cmd, args]) => (window as any).__TAURI_INTERNALS__.invoke(cmd, args), [cmd, args] as const) as Promise<T>;
}

export const CAMPAIGN = "E2E";
export const CHARACTER = "Tester";

type Choice = { label: string; hint: string; value: string };
type Question =
  | { type: "pick"; title: string; choices: Choice[]; count: number }
  | { type: "number"; title: string }
  | { type: "abilities"; scores: { scores: number[] } }
  | { type: "confirm" };

/// The test character's choices, by label; any other question takes its
/// first choices. A level 1 human wizard with the Scholar's Pack.
const PREFERRED = ["Human", "Acolyte", "Standard array", "Alert", "Wizard"];

async function createCharacter(page: Page) {
  let step = await invoke<{ question: Question }>(page, "creation_start", { name: CHARACTER });
  for (let i = 0; i < 50; i++) {
    const q = step.question;
    if (q.type === "confirm") {
      await invoke(page, "creation_finish");
      return;
    }
    let answer;
    if (q.type === "pick") {
      const wanted =
        q.choices.find((c) => PREFERRED.includes(c.label) && c.hint === "XPHB") ??
        q.choices.find((c) => PREFERRED.includes(c.label)) ??
        // Starting equipment: the class's or background's items (A), not gold.
        q.choices.find((c) => c.label.startsWith("A:"));
      answer = { type: "pick", value: wanted ? [wanted.value] : q.choices.slice(0, q.count).map((c) => c.value) };
    } else if (q.type === "abilities") {
      answer = { type: "scores", value: q.scores.scores };
    } else {
      answer = { type: "number", value: 1 };
    }
    step = await invoke(page, "creation_answer", { answer });
  }
  throw new Error("Character creation did not finish");
}

/// Import the data zip (DNT_E2E_ZIP): its bytes go with the call.
async function importData(page: Page) {
  const zip = process.env.DNT_E2E_ZIP;
  if (!zip) throw new Error("DNT_E2E_ZIP: the data zip to import (scripts/e2e-android.sh sets it)");
  await page.evaluate((b64) => {
    const bytes = Array.from(atob(b64), (c) => c.charCodeAt(0));
    return (window as any).__TAURI_INTERNALS__.invoke("e2e_import_zip", { bytes });
  }, readFileSync(zip).toString("base64"));
}

/// Start from a known state: everything reset, the data imported, the
/// campaign and the character created.
async function seed(page: Page) {
  // The reset wipes everything: never on a device with someone's campaigns.
  const status = await invoke<{ campaigns: { name: string }[] }>(page, "status");
  const others = status.campaigns.map((c) => c.name).filter((n) => n !== CAMPAIGN);
  if (others.length && !process.env.DNT_E2E_RESET) {
    throw new Error(
      `The app on this device has campaigns (${others.join(", ")}), and the tests reset it. ` +
        "Use a test device, or set DNT_E2E_RESET=1 to wipe them.",
    );
  }
  await invoke(page, "reset_app");
  await importData(page);
  await invoke(page, "create_campaign", { name: CAMPAIGN, sources: ["XPHB", "XDMG", "XMM"] });
  await createCharacter(page);
  await invoke(page, "close_character");
  await page.reload();
}

export const test = base.extend<{ page: Page }, { app: Page }>({
  app: [
    async ({}, use) => {
      const browser = await chromium.connectOverCDP(await connect());
      const page = browser.contexts()[0].pages()[0];
      // The frontend is up when it can call the app.
      await page.waitForFunction(() => (window as any).__TAURI_INTERNALS__);
      await seed(page);
      await use(page);
      await browser.close();
    },
    { scope: "worker" },
  ],
  page: async ({ app }, use, info) => {
    await use(app);
    if (info.status !== info.expectedStatus) {
      await info.attach("screen", { body: await app.screenshot(), contentType: "image/png" });
    }
    // The next test starts with no dialog open.
    for (let i = 0; i < 3 && (await app.getByRole("dialog").count()); i++) await app.keyboard.press("Escape");
  },
});

export { expect };

/// Go to a section through the menu (☰).
export async function go(page: Page, section: "Compendium" | "Characters" | "Notes" | "Campaigns" | "Data") {
  await page.getByRole("button", { name: "Menu" }).click();
  await page.getByRole("button", { name: section }).click();
}

/// Open the test character's sheet on a tab.
export async function openSheet(page: Page, tab: "Overview" | "Features" | "Spells" | "Inventory" = "Overview") {
  // A fresh frontend: the roster, whatever the last test left open.
  await invoke(page, "close_character");
  await page.reload();
  await go(page, "Characters");
  await page.getByRole("button", { name: new RegExp(CHARACTER) }).click();
  await page.getByRole("button", { name: tab, exact: true }).click();
}

/// The dialog with this title.
export const dialog = (page: Page, title: string | RegExp) => page.getByRole("dialog", { name: title });
