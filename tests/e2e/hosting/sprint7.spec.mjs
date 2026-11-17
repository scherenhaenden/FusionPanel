import dns from 'node:dns';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const bind9Port = process.env.FUSIONPANEL_S7_BIND9_PORT;

function createBindResolver() {
  const resolver = new dns.promises.Resolver();
  resolver.setServers([`127.0.0.1:${bind9Port}`]);
  return resolver;
}

async function resolveA(name) {
  try {
    const addrs = await createBindResolver().resolve4(name);
    return addrs.join(',');
  } catch {
    return '';
  }
}

async function resolveTxt(name) {
  try {
    const records = await createBindResolver().resolveTxt(name);
    return records.map((chunks) => chunks.join('')).join('\n');
  } catch {
    return '';
  }
}

test('creates DNS zone, adds A + DKIM TXT records, verifies BIND9 via DNS query & UI probe, updates and deletes record', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(bind9Port).toBeTruthy();

  // 1. Login and navigate to DNS
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(new URL('/', frontendUrl).toString());

  await page.getByRole('link', { name: 'DNS', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'DNS Zones & DKIM Sync' })).toBeVisible();

  // 2. Create DNS zone dns-e2e.local
  await page.getByRole('button', { name: 'Create Zone' }).click();
  await page.getByLabel('Domain').fill('dns-e2e.local');
  await page.getByRole('button', { name: 'Save Zone' }).click();

  await expect(page.getByRole('heading', { name: 'dns-e2e.local' })).toBeVisible();
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // 3. Add A record www -> 203.0.113.10
  await page.getByLabel('Record Name').fill('www');
  await page.getByLabel('Record Type').selectOption('A');
  await page.getByLabel('Record Value').fill('203.0.113.10');
  await page.getByRole('button', { name: 'Add Record' }).click();
  await expect(page.locator('tbody')).toContainText('203.0.113.10', { timeout: 30_000 });

  // 4. Sync DKIM TXT record
  await page.getByLabel('DKIM Selector').fill('default');
  await page.getByLabel('DKIM TXT Record').fill('v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG');
  await page.getByRole('button', { name: 'Sync DKIM' }).click();
  await expect(page.locator('tbody')).toContainText('default._domainkey', { timeout: 30_000 });

  // 5. Verify authoritative BIND9 serves both records
  await expect.poll(() => resolveA('www.dns-e2e.local'), {
    timeout: 30_000,
    intervals: [500, 1000],
  }).toBe('203.0.113.10');

  await expect.poll(() => resolveTxt('default._domainkey.dns-e2e.local'), {
    timeout: 30_000,
    intervals: [500, 1000],
  }).toContain('v=DKIM1; k=rsa; p=MIIBIjANBgkqhkiG');

  // 6. Trigger Propagation Probe from UI and verify Last Probe banner
  await page.getByRole('button', { name: 'Probe Propagation' }).click();
  await expect(page.locator('text=Last Probe: OK: default._domainkey.dns-e2e.local')).toBeVisible({
    timeout: 30_000,
  });

  // 7. Update A record to 203.0.113.20 and verify BIND9 updates
  const wwwRow = page.locator('tbody tr').filter({ hasText: 'www' });
  await wwwRow.getByRole('button', { name: 'Edit' }).click();
  await page.getByLabel('Record Value').fill('203.0.113.20');
  await page.getByRole('button', { name: 'Save Record' }).click();

  await expect.poll(() => resolveA('www.dns-e2e.local'), {
    timeout: 30_000,
    intervals: [500, 1000],
  }).toBe('203.0.113.20');

  // 8. Delete A record and verify BIND9 removes it
  await wwwRow.getByRole('button', { name: 'Delete' }).click();
  await expect.poll(() => resolveA('www.dns-e2e.local'), {
    timeout: 30_000,
    intervals: [500, 1000],
  }).toBe('');
});
