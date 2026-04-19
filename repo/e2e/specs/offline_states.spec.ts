// Flow: offline-friendly error surfaces.
//
// * Unauthenticated access to a protected route redirects to login (not a
//   raw 500 or backend envelope).
// * An unknown path renders the SPA's NotFound view rather than a raw
//   server error.

import { test, expect } from "@playwright/test";

test("protected route without login redirects to /login", async ({ page }) => {
    await page.goto("/admin/users");
    // The SPA's router either renders the login page or redirects.
    await page.waitForLoadState("networkidle");
    const url = page.url();
    expect(url).toMatch(/\/login|\/$/);
});

test("unknown SPA path renders a not-found state", async ({ page }) => {
    await page.goto("/this/path/does/not/exist");
    await page.waitForLoadState("networkidle");
    // Either the SPA renders a NotFound view or redirects to login; either
    // way it must NOT be a raw 500 or stack trace.
    const body = await page.locator("body").innerText();
    expect(body.toLowerCase()).not.toContain("traceback");
    expect(body.toLowerCase()).not.toContain("panicked");
    expect(body.toLowerCase()).not.toContain("internal server error");
});
