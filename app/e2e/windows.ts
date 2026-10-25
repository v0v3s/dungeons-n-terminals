// The Windows side: start the app with its WebView2 open to the DevTools
// protocol, in a home of its own.
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, openSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

let app: ChildProcess | undefined;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/// What the app printed, kept with the test results.
const LOG = fileURLToPath(new URL("./results/app.log", import.meta.url));

/// Start the app (DNT_E2E_APP, or the release build) and return the
/// endpoint to connect to.
export async function connect(port = 9333): Promise<string> {
  const exe = process.env.DNT_E2E_APP ?? fileURLToPath(new URL("../../target/release/dnt-app.exe", import.meta.url));
  if (!existsSync(exe)) throw new Error(`No app at ${exe}: build it with the e2e feature (scripts/e2e-app.sh)`);
  const home = mkdtempSync(join(tmpdir(), "dnt-e2e-"));
  mkdirSync(join(LOG, ".."), { recursive: true });
  const log = openSync(LOG, "a");
  let exited: string | null = null;
  app = spawn(exe, [], {
    env: {
      ...process.env,
      DNT_HOME: home,
      DNT_SEED: "1",
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
    stdio: ["ignore", log, log],
  });
  app.on("exit", (code, signal) => (exited = `exited (${code ?? signal})`));
  app.on("error", (e) => (exited = String(e)));
  const endpoint = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 450; i++) {
    if (exited) throw new Error(`The app ${exited}; its output:\n${readFileSync(LOG, "utf8")}`);
    try {
      if ((await fetch(`${endpoint}/json/version`)).ok) return endpoint;
    } catch {
      // Not up yet.
    }
    await sleep(200);
  }
  throw new Error(`The app did not open its DevTools port in 90 s (${exe}); its output:\n${readFileSync(LOG, "utf8")}`);
}

export function stop() {
  app?.kill();
}
