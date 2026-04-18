// Playwright configuration for TerraOps flow specs.
// Target: the `app` service on https://localhost:8443 (self-signed TLS).
// Real specs (7 flows) are added in P1+; scaffold commits only the config.

import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./specs",
  timeout: 30_000,
  retries: 0,
  reporter: [["list"]],
  use: {
    baseURL: "https://localhost:8443",
    ignoreHTTPSErrors: true, // dev-only self-signed cert
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        launchOptions: {
          executablePath: process.env.CHROME_PATH || undefined,
        },
      },
    },
  ],
});
