// Flow: admin operations surfaces render for the Administrator demo user.
//
// Verifies that the admin can reach the Users, Allowlist, Retention,
// mTLS, and Audit admin pages and that each page renders its header.

import { test, expect } from "@playwright/test";

// TerraOps login is username-based (not email). The seeded admin username
// is "admin" (password TerraOps!2026).
const ADMIN_USER = "admin";
const ADMIN_PW = "TerraOps!2026";

async function login(page: any) {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", ADMIN_USER);
    await page.fill("input[type=password]", ADMIN_PW);
    await page.click("button[type=submit]");
    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });
}

test("admin reaches Users, Allowlist, Retention, mTLS, Audit", async ({ page }) => {
    await login(page);
    for (const slug of ["users", "allowlist", "retention", "mtls", "audit"]) {
        await page.goto(`/admin/${slug}`);
        await expect(page.locator("body")).toContainText(new RegExp(slug, "i"));
    }
});
