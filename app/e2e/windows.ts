// The Windows side: start the app with its WebView2 open to the DevTools
// protocol, in a home of its own.
import { spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

let app: ChildProcess | undefined;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/// Start the app (DNT_E2E_APP, or the release build) and return the
/// endpoint to connect to.
export async function connect(port = 9333): Promise<string> {
  const exe = process.env.DNT_E2E_APP ?? fileURLToPath(new URL("../../target/release/dnt-app.exe", import.meta.url));
  const home = mkdtempSync(join(tmpdir(), "dnt-e2e-"));
  app = spawn(exe, [], {
    env: {
      ...process.env,
      DNT_HOME: home,
      DNT_SEED: "1",
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
    stdio: "ignore",
  });
  const endpoint = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 150; i++) {
    try {
      if ((await fetch(`${endpoint}/json/version`)).ok) return endpoint;
    } catch {
      // Not up yet.
    }
    await sleep(200);
  }
  throw new Error(`The app did not open its DevTools port (${exe})`);
}

export function stop() {
  app?.kill();
}
