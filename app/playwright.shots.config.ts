// The documentation's screenshots of the app (docs/wiki/images), made on a
// device like the end-to-end tests: `npx playwright test -c playwright.shots.config.ts`
// with the tests' environment (DNT_E2E_ZIP, DNT_DEVICE, …).
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "shots",
  testMatch: "*.ts",
  workers: 1,
  timeout: 120_000,
  reporter: [["list"]],
  outputDir: "e2e/results/shots",
});
