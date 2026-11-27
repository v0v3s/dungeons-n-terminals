// For development only: the page in an ordinary browser (prototyping the
// UI with hot reload and the browser's phone and tablet views). Its calls
// go to the desktop app run with the `web-dev` feature (`npm run web`),
// which runs them with its own handlers (src-tauri/src/webdev.rs); Vite
// passes `/__ipc` on to it.
import { mockIPC } from "@tauri-apps/api/mocks";

/// Calls that only mean something inside the app's own window (listening
/// for Android's back button, for events): nothing to do in a browser.
const LOCAL = ["plugin:app|register_listener", "plugin:app|remove_listener", "plugin:event|listen", "plugin:event|unlisten"];

export function bridge() {
  mockIPC(async (cmd, args) => {
    if (LOCAL.includes(cmd)) return null;
    const res = await fetch(`/__ipc/${encodeURIComponent(cmd)}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(args ?? {}),
    });
    const value = await res.json();
    if (!res.ok) throw value;
    return value;
  });
}
