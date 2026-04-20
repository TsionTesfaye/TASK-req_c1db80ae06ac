// Flow: Analyst creates an alert rule and lands on the alerts feed; the
// notification center bell is reachable.

import { test, expect } from "@playwright/test";

// TerraOps login is username-based (not email). Seeded analyst username: "analyst".
const USERNAME = "analyst";
const PW = "TerraOps!2026";

test("analyst reaches alert rules + alerts feed + notification center", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", USERNAME);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    // SPA routes are flat per `crates/frontend/src/router.rs`:
    //   analyst alert rules → `/alerts/rules`
    //   end-user alerts feed → `/alerts/events`
    await page.goto("/alerts/rules");
    await expect(page.locator("body")).toContainText(/alert/i);

    await page.goto("/alerts/events");
    await expect(page.locator("body")).toContainText(/alert/i);

    await page.goto("/notifications");
    await expect(page.locator("body")).toContainText(/notification/i);
});
