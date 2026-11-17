import http from 'node:http';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const nginxUrl = process.env.FUSIONPANEL_NGINX_URL;

test('manages jailed static site files served by Nginx and executes cron jobs via Run Now', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(nginxUrl).toBeTruthy();

  // 1. Login
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);

  // 2. Wait for node online
  await page.getByRole('link', { name: 'Nodes' }).click();
  await expect(page.getByText('online', { exact: false }).first()).toBeVisible({ timeout: 30_000 });

  // 3. Create static site files-e2e.local
  await page.getByRole('link', { name: 'Websites', exact: true }).click();
  await page.getByRole('button', { name: 'Create website' }).click();
  await page.getByLabel('Domain').fill('files-e2e.local');
  await page.getByLabel('Document root').fill('/srv/fusionpanel/sites/files-e2e.local');
  await page.getByRole('dialog').getByRole('button', { name: 'Create website' }).click();

  const siteRow = page.locator('tbody tr').filter({ hasText: 'files-e2e.local' });
  await expect(siteRow).toBeVisible({ timeout: 20_000 });
  await expect(siteRow).toContainText(/Active|Failed/, { timeout: 60_000 });
  if ((await siteRow.innerText()).includes('Failed')) {
    throw new Error('SiteCreate failed for files-e2e.local');
  }

  // 4. Write index.html in File Manager and verify Nginx serves it
  await page.getByRole('link', { name: 'Files' }).click();
  await expect(page.getByLabel('Website')).toBeVisible({ timeout: 15_000 });
  await page.getByLabel('File Path').fill('index.html');
  await page.getByLabel('File Content').fill('hello-sprint8');
  await page.getByRole('button', { name: 'Save File' }).click();
  await expect(page.getByRole('status')).toContainText('Saved index.html', { timeout: 15_000 });
  await expect(page.getByRole('button', { name: 'index.html', exact: true })).toBeVisible();

  const nginxRes = await new Promise((resolve, reject) => {
    const req = http.get(nginxUrl, { headers: { Host: 'files-e2e.local' } }, (res) => {
      let body = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => {
        body += chunk;
      });
      res.on('end', () => resolve({ status: res.statusCode, body }));
    });
    req.on('error', reject);
  });
  expect(nginxRes.status).toBe(200);
  expect(nginxRes.body).toContain('hello-sprint8');

  // 5. Attempt path traversal write and confirm jail error
  await page.getByLabel('File Path').fill('../escape.txt');
  await page.getByLabel('File Content').fill('should-fail');
  await page.getByRole('button', { name: 'Save File' }).click();
  await expect(page.getByRole('alert')).toBeVisible({ timeout: 15_000 });

  // 6. Create Cron Job and trigger Run Now
  await page.getByRole('link', { name: 'Cron Jobs' }).click();
  await page.getByRole('button', { name: /New Cron Job/i }).click();
  await page.getByLabel('Name').fill('e2e-heartbeat');
  await page.getByLabel('Schedule').fill('* * * * *');
  await page.getByLabel('Command').fill('echo cron-ok');
  await page.getByRole('button', { name: 'Create Cron Job' }).click();

  const cronRow = page.locator('tr', { hasText: 'e2e-heartbeat' });
  await expect(cronRow).toBeVisible({ timeout: 15_000 });
  await expect(cronRow.getByText('ready')).toBeVisible({ timeout: 30_000 });

  await cronRow.getByRole('button', { name: /Run Now/i }).click();
  await expect(cronRow.getByText('cron-ok', { exact: false })).toBeVisible({ timeout: 30_000 });
});
