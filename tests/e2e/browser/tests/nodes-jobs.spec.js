import { expect, test } from '@playwright/test';
import {
  assertDisposableFixture,
  restartDisposableServices,
} from '../fixtures/sprint2/fixture-context.js';

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const nodeName = 'sprint2-disposable-agent';

async function waitForApi(page, frontendUrl) {
  const healthUrl = new URL('/v1/health', frontendUrl).toString();
  await expect
    .poll(
      async () => {
        try {
          const response = await page.request.get(healthUrl, { timeout: 2_000 });
          return response.status();
        } catch {
          return 0;
        }
      },
      { timeout: 60_000, intervals: [250, 500, 1_000, 2_000] },
    )
    .toBe(200);
}

async function refreshNodeUntilOnline(page, nodeRow) {
  const stateCell = nodeRow.locator('td').nth(1);
  const refreshButton = page.getByRole('button', { name: 'Refresh nodes' });
  const deadline = Date.now() + 60_000;

  while (Date.now() < deadline) {
    await refreshButton.click({ timeout: 10_000 });
    try {
      await expect(stateCell).toHaveText('online', { timeout: 2_000 });
      return;
    } catch {
      // The restarted agent retries until its API is reachable; refresh the UI
      // until that real heartbeat is visible or the bounded wait expires.
    }
  }

  await expect(stateCell).toHaveText('online', { timeout: 1_000 });
}

test('operator sees the live agent and completes a System Info job', async ({ page }) => {
  test.setTimeout(300_000);
  const fixture = assertDisposableFixture();
  expect(password, 'the disposable fixture must provide a synthetic admin password').toBeTruthy();

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL('/');

  await page.getByRole('link', { name: 'Nodes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nodes', exact: true })).toBeVisible();
  const initialNodeRow = page.getByRole('row').filter({ hasText: nodeName });
  await expect(initialNodeRow).toBeVisible({ timeout: 60_000 });
  await expect(initialNodeRow.locator('td').nth(1)).toHaveText('online');
  const initialNodeText = await initialNodeRow.locator('td').first().innerText();
  const nodeId = initialNodeText.split(/\r?\n/).map((line) => line.trim()).filter(Boolean).at(-1);
  expect(nodeId).toMatch(/^[0-9a-f-]{36}$/i);

  await page.getByRole('link', { name: 'Jobs', exact: true }).click();
  await page.getByRole('button', { name: 'Run System Info Job' }).click();

  const details = page.getByRole('region', { name: 'Job details' });
  await expect(details).toBeVisible();
  const jobId = (await details.locator('p.font-mono').innerText()).trim();
  expect(jobId).toMatch(/^[0-9a-f-]{36}$/i);
  await expect(details.getByText('succeeded', { exact: true }).first()).toBeVisible({ timeout: 60_000 });
  await expect(details.locator('ol')).toContainText('"operation":"system.info"');

  restartDisposableServices(fixture);
  await waitForApi(page, fixture.frontendUrl);

  // Reloading the protected app after the API restart also checks that the
  // operator session survived alongside the node/job records in SQLite.
  await page.goto('/');
  await expect(page).toHaveURL('/');
  await page.getByRole('link', { name: 'Nodes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nodes', exact: true })).toBeVisible();

  const restartedNodeRow = page.getByRole('row').filter({ hasText: nodeName });
  await expect(restartedNodeRow).toBeVisible({ timeout: 60_000 });
  const restartedNodeText = await restartedNodeRow.locator('td').first().innerText();
  const restartedNodeId = restartedNodeText.split(/\r?\n/).map((line) => line.trim()).filter(Boolean).at(-1);
  expect(restartedNodeId).toBe(nodeId);
  await refreshNodeUntilOnline(page, restartedNodeRow);

  await page.getByRole('link', { name: 'Jobs', exact: true }).click();
  const persistedJobRow = page.getByRole('row').filter({ hasText: jobId });
  await expect(persistedJobRow).toBeVisible({ timeout: 30_000 });
  await expect(persistedJobRow).toContainText('succeeded');
  await persistedJobRow.getByRole('button', { name: 'View details' }).click();

  const persistedDetails = page.getByRole('region', { name: 'Job details' });
  await expect(persistedDetails.locator('p.font-mono')).toHaveText(jobId);
  await expect(persistedDetails.getByText('succeeded', { exact: true }).first()).toBeVisible();
  await expect(persistedDetails.locator('ol')).toContainText('"operation":"system.info"');
});
