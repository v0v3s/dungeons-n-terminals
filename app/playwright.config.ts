// End-to-end tests: they drive the app on an Android device over the
// WebView's DevTools protocol. Run them with scripts/e2e-android.sh.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  // One app on one device: the tests take turns.
  workers: 1,
  fullyParallel: false,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  reporter: [["list"], ["html", { outputFolder: "e2e/report", open: "never" }]],
  outputDir: "e2e/results",
});
