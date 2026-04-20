// Flow: Analyst navigates the metric + report surfaces.

import { test, expect } from "@playwright/test";

// TerraOps login is username-based (not email). Seeded analyst username: "analyst".
const USERNAME = "analyst";
const PW = "TerraOps!2026";

test("analyst reaches sources, definitions, kpi, reports", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", USERNAME);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    // SPA routes are flat per `crates/frontend/src/router.rs`:
    //   `/env/sources`, `/metrics/definitions`, `/kpi`, `/reports`.
    const routes: Array<[string, RegExp]> = [
        ["/env/sources", /source/i],
        ["/metrics/definitions", /definition/i],
        ["/kpi", /kpi/i],
        ["/reports", /report/i],
    ];
    for (const [path, needle] of routes) {
        await page.goto(path);
        await expect(page.locator("body")).toContainText(needle);
    }
});
