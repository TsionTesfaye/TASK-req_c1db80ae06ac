// Flow: Recruiter reaches candidates, recommendations, weights, watchlists.

import { test, expect } from "@playwright/test";

const EMAIL = "recruiter@terraops.local";
const PW = "TerraOps!2026";

test("recruiter reaches talent surfaces", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", EMAIL);
    await page.fill("input[type=password]", PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });

    // SPA routes are flat per `crates/frontend/src/router.rs`: the recruiter
    // surfaces live under `/talent/*`, not `/recruiter/*`.
    for (const slug of ["candidates", "roles", "recommendations", "weights", "watchlists"]) {
        await page.goto(`/talent/${slug}`);
        await expect(page.locator("body")).toContainText(new RegExp(slug, "i"));
    }
});
