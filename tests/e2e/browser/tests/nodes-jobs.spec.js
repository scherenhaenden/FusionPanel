import { expect, test } from '@playwright/test';

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;

test('operator sees the live agent and completes a System Info job', async ({ page }) => {
  test.setTimeout(150_000);
  expect(password, 'the disposable fixture must provide a synthetic admin password').toBeTruthy();

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL('/');

  await page.getByRole('link', { name: 'Nodes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nodes' })).toBeVisible();
  await expect(page.getByText('sprint2-disposable-agent', { exact: true })).toBeVisible({ timeout: 60_000 });
  await expect(page.getByText('online', { exact: true })).toBeVisible();

  await page.getByRole('link', { name: 'Jobs', exact: true }).click();
  await page.getByRole('button', { name: 'Run System Info Job' }).click();

  const details = page.getByRole('region', { name: 'Job details' });
  await expect(details).toBeVisible();
  await expect(details.getByText('succeeded', { exact: true }).first()).toBeVisible({ timeout: 60_000 });
  await expect(details.locator('ol')).toContainText('"operation":"system.info"');
});
