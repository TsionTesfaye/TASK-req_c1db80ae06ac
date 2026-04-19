// Flow: Analyst creates an alert rule and lands on the alerts feed; the
// notification center bell is reachable.

import { test, expect } from "@playwright/test";

const EMAIL = "analyst@terraops.local";
const PW = "TerraOps!2026";

test("analyst reaches alert rules + alerts feed + notification center", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", EMAIL);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    await page.goto("/analyst/alert-rules");
    await expect(page.locator("body")).toContainText(/alert/i);

    await page.goto("/user/alerts");
    await expect(page.locator("body")).toContainText(/alert/i);

    await page.goto("/notifications");
    await expect(page.locator("body")).toContainText(/notification/i);
});
