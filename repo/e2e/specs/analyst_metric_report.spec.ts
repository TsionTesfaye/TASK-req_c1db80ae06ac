// Flow: Analyst navigates the metric + report surfaces.

import { test, expect } from "@playwright/test";

const EMAIL = "analyst@terraops.local";
const PW = "TerraOps!2026";

test("analyst reaches sources, definitions, kpi, reports", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", EMAIL);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    for (const slug of ["sources", "definitions", "kpi", "reports"]) {
        await page.goto(`/analyst/${slug}`);
        await expect(page.locator("body")).toContainText(new RegExp(slug, "i"));
    }
});
