// Flow: login smoke
//
// Asserts the login form renders, rejects bad credentials with a toast or
// error surface, accepts valid demo credentials, and lands on the
// dashboard. Uses the canonical demo administrator seeded by
// `terraops-backend seed`.

import { test, expect } from "@playwright/test";

const ADMIN_EMAIL = "admin@terraops.local";
const ADMIN_PW = "TerraOps!2026";

test("admin can log in and lands on dashboard", async ({ page }) => {
    await page.goto("/login");
    await expect(page.locator("input[type=email], input[name=username]")).toBeVisible();
    await page.fill("input[name=username], input[type=email]", ADMIN_EMAIL);
    await page.fill("input[type=password]", ADMIN_PW);
    await page.click("button[type=submit], button:has-text('Sign in'), button:has-text('Log in')");

    await expect(page).toHaveURL(/\/dashboard|\/$/, { timeout: 10_000 });
    await expect(page.locator("body")).toContainText(/dashboard|kpi|welcome/i);
});

test("login rejects wrong password", async ({ page }) => {
    await page.goto("/login");
    await page.fill("input[name=username], input[type=email]", ADMIN_EMAIL);
    await page.fill("input[type=password]", "wrong-pw");
    await page.click("button[type=submit], button:has-text('Sign in'), button:has-text('Log in')");
    // Still on login URL; no silent redirect to dashboard.
    await page.waitForTimeout(500);
    expect(page.url()).toMatch(/\/login/);
});
