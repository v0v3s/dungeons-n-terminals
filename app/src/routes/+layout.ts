// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

// In development, in an ordinary browser (not the app's window): the
// app's commands through the desktop app (`npm run web`, see webdev.ts).
if (import.meta.env.DEV && typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
  const { bridge } = await import("../lib/webdev");
  bridge();
}
