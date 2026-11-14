// Links out of the app: only to the project (its wiki, its source), in the
// system's browser. The app's capability allows those URLs and nothing else
// (src-tauri/capabilities/default.json). The end-to-end tests catch them
// instead of opening a browser: see app/e2e/fixtures.ts.
import { openUrl } from "@tauri-apps/plugin-opener";

export const PROJECT = "https://github.com/v0v3s/dungeons-n-terminals";
/// The wiki's page on making your own data.
export const WIKI_DATA = `${PROJECT}/wiki/Files-and-data`;

/// The links a test caught, when one is listening.
const caught = (): string[] | undefined => (window as { dntE2eLinks?: string[] }).dntE2eLinks;

/// Open a page of the project in the system's browser.
export async function openLink(url: string): Promise<void> {
  if (url !== PROJECT && !url.startsWith(`${PROJECT}/`)) throw new Error(`Not a link to the project: ${url}`);
  const links = caught();
  if (links) {
    links.push(url);
    return;
  }
  await openUrl(url);
}
