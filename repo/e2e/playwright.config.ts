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
    // When running the flow gate from the `tests` container, set
    // `PLAYWRIGHT_BASE_URL=https://app:8443` so Chromium resolves the
    // sibling `app` service on the compose network. Default stays
    // `https://localhost:8443` for host-side ad-hoc runs.
    baseURL: process.env.PLAYWRIGHT_BASE_URL || "https://localhost:8443",
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
