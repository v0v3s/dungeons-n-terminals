// Fingers on the phone's screen, through adb: the WebView's own touch
// events, as a person makes them.
import { adb } from "./android";

/// A finger swipe across the element, from `from` to `to` of its width.
export async function swipe(page: import("@playwright/test").Page, target: import("@playwright/test").Locator, from: number, to: number) {
  const box = (await target.boundingBox())!;
  const dpr = await page.evaluate(() => devicePixelRatio);
  const ui = (adb("shell", "uiautomator", "dump", "/sdcard/dnt-e2e-ui.xml"), adb("shell", "cat", "/sdcard/dnt-e2e-ui.xml"));
  const at = ui.match(/class="android\.webkit\.WebView"[^>]*bounds="\[(\d+),(\d+)\]/);
  if (!at) throw new Error("No WebView on the screen");
  const x = (f: number) => String(Math.round(Number(at[1]) + (box.x + box.width * f) * dpr));
  const y = String(Math.round(Number(at[2]) + (box.y + Math.min(box.height, 300) / 2) * dpr));
  adb("shell", "input", "swipe", x(from), y, x(to), y, "250");
}

/// A finger held on the element: put down at `from` of its width, moved
/// through `through`, lifted, each step a separate touch event.
export async function drag(page: import("@playwright/test").Page, target: import("@playwright/test").Locator, from: number, through: number[], check?: () => Promise<void>) {
  const box = (await target.boundingBox())!;
  const dpr = await page.evaluate(() => devicePixelRatio);
  const ui = (adb("shell", "uiautomator", "dump", "/sdcard/dnt-e2e-ui.xml"), adb("shell", "cat", "/sdcard/dnt-e2e-ui.xml"));
  const at = ui.match(/class="android\.webkit\.WebView"[^>]*bounds="\[(\d+),(\d+)\]/);
  if (!at) throw new Error("No WebView on the screen");
  const x = (f: number) => String(Math.round(Number(at[1]) + (box.x + box.width * f) * dpr));
  const y = String(Math.round(Number(at[2]) + (box.y + Math.min(box.height, 300) / 2) * dpr));
  adb("shell", "input", "motionevent", "DOWN", x(from), y);
  for (const f of through) adb("shell", "input", "motionevent", "MOVE", x(f), y);
  await check?.();
  // Held still a moment before lifting: not a flick.
  await page.waitForTimeout(300);
  adb("shell", "input", "motionevent", "UP", x(through.at(-1)!), y);
}
