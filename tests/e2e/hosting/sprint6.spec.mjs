import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const pgContainer = process.env.FUSIONPANEL_S6_PG_CONTAINER;
const sitesDir = process.env.FUSIONPANEL_S6_SITES_DIR;
const minioUser = process.env.FUSIONPANEL_S6_MINIO_USER || 'minioadmin';
const minioPass = process.env.FUSIONPANEL_S6_MINIO_PASS || 'minioadmin123';

function runPsqlAsAdmin(dbName, sql) {
  return execFileSync(
    'docker',
    ['exec', pgContainer, 'psql', '-h', '127.0.0.1', '-U', 'postgres', '-d', dbName, '-t', '-A', '-c', sql],
    { encoding: 'utf8' },
  );
}

test('creates S3 bundle backup (static site + PostgreSQL) via MinIO and restores after deletion', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(pgContainer).toBeTruthy();
  expect(sitesDir).toBeTruthy();

  // 1. Login
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(new URL('/', frontendUrl).toString());

  // 2. Seed static site backup-app.local/index.html and create website in UI
  const siteRootHost = path.join(sitesDir, 'backup-app.local');
  fs.mkdirSync(siteRootHost, { recursive: true });
  fs.chmodSync(siteRootHost, 0o755);
  const indexHtmlHost = path.join(siteRootHost, 'index.html');
  fs.writeFileSync(indexHtmlHost, '<html><body>sprint6-site-marker</body></html>\n', { mode: 0o644 });

  await page.getByRole('link', { name: 'Websites', exact: true }).click();
  await page.getByRole('button', { name: 'Create website' }).click();
  await page.getByLabel('Domain').fill('backup-app.local');
  await page.getByLabel('Document root').fill('/srv/fusionpanel/sites/backup-app.local');
  await page.getByRole('dialog').getByRole('button', { name: 'Create website' }).click();

  const siteRow = page.locator('tbody tr').filter({ hasText: 'backup-app.local' });
  await expect(siteRow).toContainText(/Active|Failed/, { timeout: 60_000 });
  if ((await siteRow.innerText()).includes('Failed')) {
    const jobsData = await page.evaluate(async () => (await fetch('/v1/jobs')).json());
    const jobId = jobsData.jobs?.[0]?.id;
    const logs = jobId
      ? await page.evaluate(async (id) => (await fetch(`/v1/jobs/${id}/logs`)).json(), jobId)
      : null;
    throw new Error(`SiteCreate failed; jobs: ${JSON.stringify(jobsData)} logs: ${JSON.stringify(logs)}`);
  }

  // 3. Create PostgreSQL database backup_db and seed table + row
  await page.getByRole('link', { name: 'Databases', exact: true }).click();
  await page.getByRole('button', { name: 'Create Database' }).click();
  await page.getByLabel('Engine:').selectOption('postgresql');
  await page.getByLabel('Database Name:').fill('backup_db');
  await page.getByRole('button', { name: 'Create', exact: true }).click();

  const dbItem = page.locator('li').filter({ hasText: 'backup_db' });
  await expect(dbItem).toContainText('ready', { timeout: 60_000 });

  runPsqlAsAdmin(
    'backup_db',
    "CREATE TABLE items (val TEXT NOT NULL); INSERT INTO items (val) VALUES ('sprint6-db-row');",
  );
  expect(runPsqlAsAdmin('backup_db', 'SELECT val FROM items;').trim()).toBe('sprint6-db-row');

  // 4. Create S3 bundle backup via Backups UI
  await page.getByRole('link', { name: 'Backups', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Backups & Snapshots' })).toBeVisible();

  await page.getByRole('button', { name: 'Backup Now' }).click();
  await page.getByLabel('Target Scope').selectOption('bundle');
  await page.getByLabel('Destination').selectOption('s3');
  await page.getByLabel('S3 Endpoint').fill('http://minio:9000');
  await page.getByLabel('Bucket').fill('fusionpanel-backups');
  await page.getByLabel('Region').fill('us-east-1');
  await page.getByLabel('Access Key').fill(minioUser);
  await page.getByLabel('Secret Key').fill(minioPass);
  await page.getByLabel('Prefix (optional)').fill('sprint6');
  await page.getByRole('button', { name: 'Start Backup' }).click();

  const backupRow = page.locator('tbody tr').first();
  await expect(backupRow).toContainText('ready', { timeout: 60_000 });
  await expect(backupRow).toContainText('s3://fusionpanel-backups/sprint6');
  await expect(backupRow).toContainText('...');

  // 5. Destroy both live targets
  fs.unlinkSync(indexHtmlHost);
  expect(fs.existsSync(indexHtmlHost)).toBe(false);
  runPsqlAsAdmin('backup_db', 'DROP TABLE items;');

  // 6. Restore via RestoreWizard
  await backupRow.getByRole('button', { name: 'Restore' }).click();
  await expect(page.getByRole('heading', { name: 'Restore Backup Archive' })).toBeVisible();
  await page.getByLabel('S3 Endpoint').fill('http://minio:9000');
  await page.getByLabel('Access Key').fill(minioUser);
  await page.getByLabel('Secret Key').fill(minioPass);
  await page.getByPlaceholder('RESTORE').fill('RESTORE');
  await page.getByRole('button', { name: 'Confirm Restore' }).click();

  await expect(backupRow).toContainText('ready', { timeout: 60_000 });

  // 7. Verify both static site file and PostgreSQL row are restored
  expect(fs.existsSync(indexHtmlHost)).toBe(true);
  expect(fs.readFileSync(indexHtmlHost, 'utf8')).toContain('sprint6-site-marker');
  expect(runPsqlAsAdmin('backup_db', 'SELECT val FROM items;').trim()).toBe('sprint6-db-row');
});
