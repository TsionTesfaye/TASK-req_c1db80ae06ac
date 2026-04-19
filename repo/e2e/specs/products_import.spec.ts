// Flow: Data Steward imports a products batch via the real UI.
//
// Smoke level: reaches the products + imports pages and confirms the
// steward can load the list, not the admin-only pages.

import { test, expect } from "@playwright/test";

const EMAIL = "steward@terraops.local";
const PW = "TerraOps!2026";

test("steward reaches products + imports surfaces", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", EMAIL);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    await page.goto("/products");
    await expect(page.locator("body")).toContainText(/product/i);

    await page.goto("/imports");
    await expect(page.locator("body")).toContainText(/import/i);
});
