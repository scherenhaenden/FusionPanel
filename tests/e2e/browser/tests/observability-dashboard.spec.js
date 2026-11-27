import { expect, test } from '@playwright/test';

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const nodeName = process.env.FUSIONPANEL_E2E_NODE_NAME || 'sprint10-disposable-agent';
const webhookUrl = process.env.FUSIONPANEL_E2E_WEBHOOK_URL;
const webhookSecret = process.env.FUSIONPANEL_E2E_WEBHOOK_SECRET;
const webhookReceiptFile = process.env.FUSIONPANEL_E2E_WEBHOOK_RECEIPT_FILE;

test('operator views live agent telemetry, creates a signed alert rule, and observes delivery on Dashboard', async ({ page }) => {
  test.setTimeout(120_000);
  expect(password, 'disposable fixture must provide admin password').toBeTruthy();
  expect(webhookUrl, 'disposable fixture must provide loopback webhook URL').toBeTruthy();
  expect(webhookSecret, 'disposable fixture must provide webhook signing secret').toBeTruthy();

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL('/');

  await expect(page.getByRole('heading', { name: 'Node telemetry' })).toBeVisible();
  const nodeRow = page.getByRole('row').filter({ hasText: nodeName });
  await expect(nodeRow).toBeVisible({ timeout: 30_000 });

  // Wait for the real agent to publish its first sample and appear as Fresh.
  await expect
    .poll(
      async () => {
        const refresh = page.getByRole('button', { name: /Refresh/i });
        if (await refresh.isVisible()) {
          await refresh.click();
        }
        return await nodeRow.innerText();
      },
      { timeout: 30_000, intervals: [500, 1000, 1500] },
    )
    .toContain('Fresh');

  await expect(page.getByRole('heading', { name: 'Recent CPU history' })).toBeVisible();
  await expect(page.getByRole('img', { name: `CPU history for ${nodeName}` })).toBeVisible();

  // Create a signed alert rule via the Dashboard UI.
  await page.getByLabel('Name').fill('Browser E2E memory rule');
  await page.getByLabel('Metric').selectOption('memory_used_percent');
  await page.getByLabel('Condition').selectOption('above');
  await page.getByLabel('Threshold').fill('0');
  await page.getByLabel('Severity').selectOption('warning');
  await page.getByLabel('Webhook URL').fill(webhookUrl);
  await page.getByLabel('Signing secret').fill(webhookSecret);
  await page.getByRole('button', { name: 'Create alert rule' }).click();

  await expect(page.getByText('Browser E2E memory rule').first()).toBeVisible({ timeout: 10_000 });

  // Wait for the next real agent sample to trigger the rule and record delivery in the UI.
  await expect
    .poll(
      async () => {
        const refresh = page.getByRole('button', { name: /Refresh/i });
        if (await refresh.isVisible()) {
          await refresh.click();
        }
        const text = await page.locator('main').innerText();
        return text.includes('1 active alert') && text.toLowerCase().includes('delivered');
      },
      { timeout: 45_000, intervals: [1000, 1500, 2000] },
    )
    .toBe(true);

  if (webhookReceiptFile) {
    const fs = await import('node:fs');
    const receipts = JSON.parse(fs.readFileSync(webhookReceiptFile, 'utf8'));
    expect(receipts.length).toBeGreaterThanOrEqual(1);
    expect(receipts[0].verified).toBe(true);
    expect(receipts[0].event).toBe('triggered');
    expect(receipts[0].deliveryId).toBeTruthy();
  }

  // Verify a non-observability workflow (Nodes view) on the same candidate.
  await page.getByRole('link', { name: 'Nodes', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nodes', exact: true })).toBeVisible();
  await expect(page.getByRole('row').filter({ hasText: nodeName })).toBeVisible();
});
