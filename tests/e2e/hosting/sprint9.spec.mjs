import crypto from 'node:crypto';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;

function base32Decode(str) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let bits = 0;
  let value = 0;
  const out = [];
  for (const ch of str.toUpperCase()) {
    const idx = alphabet.indexOf(ch);
    if (idx === -1) continue;
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      out.push((value >>> bits) & 0xff);
    }
  }
  return Buffer.from(out);
}

function generateTotp(secretBase32) {
  const key = base32Decode(secretBase32);
  const counter = Math.floor(Date.now() / 1000 / 30);
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64BE(BigInt(counter));
  const hmac = crypto.createHmac('sha1', key).update(buf).digest();
  const offset = hmac[19] & 0x0f;
  const code =
    (((hmac[offset] & 0x7f) << 24) |
      (hmac[offset + 1] << 16) |
      (hmac[offset + 2] << 8) |
      hmac[offset + 3]) %
    1_000_000;
  return String(code).padStart(6, '0');
}

test('enforces multi-user RBAC, scoped API tokens, audit logging, and TOTP MFA', async ({ page }) => {
  test.setTimeout(180_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();

  // 1. Login as SuperAdmin
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);

  // 2. Create Viewer operator in Administration
  await page.getByRole('link', { name: 'Administration' }).click();
  await page.getByRole('button', { name: /Invite Operator/i }).click();
  await page.getByLabel('Username').fill('viewer-e2e');
  await page.getByLabel('Email Address').fill('viewer@e2e.local');
  await page.getByLabel('Password').fill('ViewerPass123!');
  await page.getByLabel('Role').selectOption('viewer');
  await page.getByRole('button', { name: 'Send Invite' }).click();
  await expect(page.getByText('viewer-e2e')).toBeVisible({ timeout: 15_000 });

  // 3. Create scoped read-only API token and verify 200 vs 403
  await page.getByRole('button', { name: /API Tokens/i }).click();
  await page.getByLabel('Token Name').fill('e2e-sites-ro');
  await page.getByLabel('Scopes (comma-separated)').fill('sites:read');
  await page.getByRole('button', { name: 'Create API Token' }).click();
  const tokenStatus = page.getByRole('status');
  await expect(tokenStatus).toContainText('fp_tok_', { timeout: 15_000 });
  const tokenText = await tokenStatus.locator('code').innerText();

  const readRes = await fetch(`${frontendUrl}/v1/websites`, {
    headers: { Authorization: `Bearer ${tokenText}` },
  });
  expect(readRes.status).toBe(200);

  const writeRes = await fetch(`${frontendUrl}/v1/websites`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${tokenText}`,
      'Idempotency-Key': 's9-ro-block',
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      domain: 'denied.local',
      kind: 'static',
      document_root: '/srv/fusionpanel/sites/denied.local',
    }),
  });
  expect(writeRes.status).toBe(403);

  // 4. Verify Audit Logs recorded mutations
  await page.getByRole('button', { name: /Audit Logs/i }).click();
  await expect(page.getByText('user.create').first()).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText('token.create').first()).toBeVisible();

  // 5. Enroll and verify TOTP MFA in Security
  await page.getByRole('link', { name: 'Security' }).click();
  await page.getByRole('button', { name: 'Enable MFA' }).click();
  const secretCodeEl = page.locator('form code').first();
  await expect(secretCodeEl).toBeVisible({ timeout: 15_000 });
  const secretBase32 = (await secretCodeEl.innerText()).trim();
  const totp = generateTotp(secretBase32);
  await page.getByLabel('TOTP Code').fill(totp);
  await page.getByRole('button', { name: 'Verify & Enable' }).click();
  await expect(page.getByRole('status')).toContainText('Two-factor authentication is now enabled.', {
    timeout: 15_000,
  });

  // 6. Logout, login as Viewer, and confirm 403 on Administration
  await page.getByTitle('Logout').click();
  await expect(page).toHaveURL(/\/login/);
  await page.getByPlaceholder('Username').fill('viewer-e2e');
  await page.getByPlaceholder('Password').fill('ViewerPass123!');
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);

  await page.getByRole('link', { name: 'Administration' }).click();
  await expect(page.getByRole('alert')).toContainText('Permission denied', { timeout: 15_000 });
});
