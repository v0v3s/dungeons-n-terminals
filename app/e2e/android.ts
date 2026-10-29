// The device side: adb, starting the app and reaching its WebView.
import { execFileSync } from "node:child_process";

export const PACKAGE = "cz.v0v3s.dnt";

let serial: string | undefined;

/// The device: DNT_DEVICE, or the only one connected.
export function device(): string {
  if (serial) return serial;
  if (process.env.DNT_DEVICE) return (serial = process.env.DNT_DEVICE);
  const found = execFileSync("adb", ["devices"], { encoding: "utf8" })
    .split("\n")
    .slice(1)
    .filter((l) => l.endsWith("\tdevice"))
    .map((l) => l.split("\t")[0]);
  if (found.length !== 1) throw new Error(`${found.length} devices connected: set DNT_DEVICE to one of them (adb devices)`);
  return (serial = found[0]);
}

export function adb(...args: string[]): string {
  return execFileSync("adb", ["-s", device(), ...args], { encoding: "utf8" });
}

/// The app's process id, or null when it is not running.
function pid(): string | null {
  try {
    return adb("shell", "pidof", PACKAGE).trim() || null;
  } catch {
    return null;
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/// Start the app when it is not running, and forward its WebView's
/// DevTools socket to `port`. Returns the endpoint to connect to.
export async function connect(port = 9333): Promise<string> {
  if (adb("shell", "dumpsys", "window").includes("isKeyguardShowing=true")) {
    throw new Error("The device is locked: unlock it first");
  }
  // Keep the screen on while it is plugged in, so it does not lock mid-run.
  adb("shell", "svc", "power", "stayon", "usb");
  if (!pid()) adb("shell", "monkey", "-p", PACKAGE, "-c", "android.intent.category.LAUNCHER", "1");
  // Upright: the tests are for a phone held in portrait, and monkey turns
  // auto-rotate back on (a phone lying on its side would turn the app).
  adb("shell", "cmd", "window", "user-rotation", "lock", "0");
  for (let i = 0; i < 100; i++) {
    const p = pid();
    if (p && adb("shell", "cat", "/proc/net/unix").includes(`webview_devtools_remote_${p}`)) {
      adb("forward", `tcp:${port}`, `localabstract:webview_devtools_remote_${p}`);
      return `http://127.0.0.1:${port}`;
    }
    await sleep(200);
  }
  throw new Error("The app's WebView cannot be debugged: install a build with the e2e feature (scripts/e2e-app.sh)");
}
