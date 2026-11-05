import { test, expect } from '@playwright/test';
import { requireVariantFixture } from './variant-fixture.js';

test.describe.configure({ mode: 'serial' });

test('variant registry exposes the supported capability choices', async ({ page }) => {
  const { frontendUrl } = requireVariantFixture();
  const response = await page.goto(new URL('/settings/variants', frontendUrl).toString());
  if (!response?.ok()) {
    throw new Error('BLOCKED: frontend variant registry route is unavailable');
  }

  const registry = page.locator('.variant-registry-table');
  if ((await registry.count()) === 0) {
    throw new Error('BLOCKED: frontend has no variant registry view');
  }

  for (const capability of ['ADM-02', 'ADM-03', 'HOST-01', 'CTR-01', 'BAK-04', 'MIG-09']) {
    await expect(page.locator(`[data-capability="${capability}"]`)).toBeVisible();
  }
});

test('valid transition persists across reload and restores the fixture value', async ({ page }) => {
  const { frontendUrl, siteId } = requireVariantFixture();
  const response = await page.goto(new URL(`/sites/${siteId}/settings`, frontendUrl).toString());
  if (!response?.ok()) {
    throw new Error('BLOCKED: frontend site settings route is unavailable');
  }

  const select = page.locator('select[name="variant_HOST-01"]');
  if ((await select.count()) === 0) {
    throw new Error('BLOCKED: frontend has no host variant control');
  }
  if ((await select.locator('option[value="upstream_reverse_proxy"]').count()) === 0) {
    throw new Error('BLOCKED: frontend has no supported host variant option');
  }
  const originalValue = await select.inputValue();
  if (!originalValue) {
    throw new Error('BLOCKED: disposable site has no initial host variant to restore');
  }

  try {
    await select.selectOption('upstream_reverse_proxy');
    await page.getByRole('button', { name: 'Apply Variant Change' }).click();
    await expect(page.locator('.toast-success')).toContainText('Variant updated');

    await page.reload();
    await expect(page.locator('select[name="variant_HOST-01"]')).toHaveValue('upstream_reverse_proxy');
  } finally {
    if (originalValue !== 'upstream_reverse_proxy') {
      await page.locator('select[name="variant_HOST-01"]').selectOption(originalValue);
      await page.getByRole('button', { name: 'Apply Variant Change' }).click();
      await expect(page.locator('.toast-success')).toContainText('Variant updated');
    }
  }
});

test('unsafe variant choices are disabled', async ({ page }) => {
  const { frontendUrl, siteId } = requireVariantFixture();
  const response = await page.goto(new URL(`/sites/${siteId}/settings`, frontendUrl).toString());
  if (!response?.ok()) {
    throw new Error('BLOCKED: frontend site settings route is unavailable');
  }

  const select = page.locator('select[name="variant_CTR-01"]');
  if ((await select.count()) === 0) {
    throw new Error('BLOCKED: frontend has no container variant control');
  }
  const unsafeOption = select.locator('option[value="unsafe_choice"]');
  if ((await unsafeOption.count()) === 0) {
    throw new Error('BLOCKED: frontend does not expose the unsafe choice needed for this qualification');
  }
  await expect(unsafeOption).toBeDisabled();
});
