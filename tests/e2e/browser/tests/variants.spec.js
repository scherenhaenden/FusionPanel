import { test, expect } from '@playwright/test';

test.describe('Variant Compatibility Browser E2E', () => {

    test('Feature registry queries', async ({ page }) => {
        const url = process.env.FUSIONPANEL_FRONTEND_URL || 'http://127.0.0.1:5173';
        const res = await page.goto(`${url}/settings/variants`).catch(() => null);
        if (!res || !res.ok()) {
            throw new Error(`BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-frontend not reachable at ${url}`);
        }

        // Wait for registry to load
        const registry = page.locator('.variant-registry-table');
        if (await registry.count() === 0) {
            throw new Error(`BLOCKED: incomplete variant registry endpoints.`);
        }

        // Ensure catalog entries are visible for required capabilities
        const requiredCapabilities = ["ADM-02", "ADM-03", "HOST-01", "CTR-01", "BAK-04", "MIG-09"];
        for (const cap of requiredCapabilities) {
            await expect(page.locator(`[data-capability="${cap}"]`)).toBeVisible();
        }
    });

    test('Valid transitions', async ({ page }) => {
        const url = process.env.FUSIONPANEL_FRONTEND_URL || 'http://127.0.0.1:5173';
        const res = await page.goto(`${url}/sites/1/settings`).catch(() => null);
        if (!res || !res.ok()) {
            throw new Error(`BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-frontend not reachable at ${url}`);
        }

        // Prove allowed transitions
        const hostVariantSelect = page.locator('select[name="variant_HOST-01"]');
        if (await hostVariantSelect.count() === 0) {
            throw new Error(`BLOCKED: incomplete variant transition behavior.`);
        }

        await hostVariantSelect.selectOption('upstream_reverse_proxy');
        await page.click('button:has-text("Apply Variant Change")');

        // Confirm successful transition prompt
        await expect(page.locator('.toast-success')).toContainText('Variant updated');
    });

    test('Invalid transitions', async ({ page }) => {
        const url = process.env.FUSIONPANEL_FRONTEND_URL || 'http://127.0.0.1:5173';
        const res = await page.goto(`${url}/sites/1/settings`).catch(() => null);
        if (!res || !res.ok()) {
            throw new Error(`BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-frontend not reachable at ${url}`);
        }

        // Attempt unsafe choice
        const ctrVariantSelect = page.locator('select[name="variant_CTR-01"]');
        if (await ctrVariantSelect.count() === 0) {
            throw new Error(`BLOCKED: missing negative transition cases.`);
        }

        // Verify incompatible choices are denied/disabled
        const unsafeOption = ctrVariantSelect.locator('option[value="unsafe_choice"]');
        await expect(unsafeOption).toBeDisabled();
    });

    test('Data persistence and observed effects after restart', async ({ page }) => {
        const url = process.env.FUSIONPANEL_FRONTEND_URL || 'http://127.0.0.1:5173';
        const res = await page.goto(`${url}/sites/1/settings`).catch(() => null);
        if (!res || !res.ok()) {
            throw new Error(`BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-frontend not reachable at ${url}`);
        }

        // Check persisted modes after a simulated restart/reload
        const hostVariantSelect = page.locator('select[name="variant_HOST-01"]');
        if (await hostVariantSelect.count() === 0) {
            throw new Error(`BLOCKED: missing variant data persistence cases.`);
        }

        await expect(hostVariantSelect).toHaveValue('upstream_reverse_proxy');
    });
});
