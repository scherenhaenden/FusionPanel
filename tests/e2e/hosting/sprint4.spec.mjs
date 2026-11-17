import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import tls from 'node:tls';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const nginxUrl = process.env.FUSIONPANEL_S3_NGINX_URL;
const httpsPort = process.env.FUSIONPANEL_S4_HTTPS_PORT;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;

const acmeDomain = 'acme-site.test';
const customDomain = 'custom-site.test';

function apiUrl(path) {
  return new URL(path, frontendUrl).toString();
}

async function createSite(page, { domain, kind, documentRoot, upstreamUrl }) {
  await page.getByRole('button', { name: 'Create website', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Create website' });
  await dialog.getByLabel('Domain name').fill(domain);
  await dialog.getByRole('radio', { name: kind === 'static' ? 'Static site' : 'Reverse proxy' }).check();

  if (kind === 'static') {
    await dialog.getByLabel('Document root').fill(documentRoot);
  } else {
    await dialog.getByLabel('Upstream URL').fill(upstreamUrl);
  }

  const responsePromise = page.waitForResponse((response) => (
    response.request().method() === 'POST'
    && new URL(response.url()).pathname === '/v1/websites'
  ));
  await dialog.getByRole('button', { name: 'Create website', exact: true }).click();
  const response = await responsePromise;
  expect(response.status()).toBe(202);
  return response.json();
}

async function waitForWebsite(page, siteId, expectedState) {
  let website;
  await expect.poll(async () => {
    const response = await page.request.get(apiUrl(`/v1/websites/${siteId}`));
    if (response.status() !== 200) return `http:${response.status()}`;
    const envelope = await response.json();
    website = envelope.website;
    return website.state;
  }, { timeout: 90_000, intervals: [250, 500, 1_000, 2_000] }).toBe(expectedState);
  return website;
}

async function assertNginxHttpResponse(page, domain, status, bodyText) {
  const response = await page.request.get(`${nginxUrl}/`, { headers: { Host: domain } });
  expect(response.status()).toBe(status);
  if (bodyText) expect(await response.text()).toContain(bodyText);
}

function fetchHttpsWithSni(domain) {
  const output = execFileSync('curl', [
    '-kfsS',
    '--resolve',
    `${domain}:${httpsPort}:127.0.0.1`,
    `https://${domain}:${httpsPort}/`,
  ], { encoding: 'utf8' });
  return output;
}

function getServerCertificate(domain) {
  return new Promise((resolve, reject) => {
    const socket = tls.connect({
      host: '127.0.0.1',
      port: Number(httpsPort),
      servername: domain,
      rejectUnauthorized: false,
    }, () => {
      const cert = socket.getPeerCertificate(true);
      socket.end();
      resolve(cert);
    });
    socket.on('error', reject);
  });
}

function generateSelfSignedPem(domain) {
  const dir = mkdtempSync(join(tmpdir(), 'fusionpanel-s4-cert-'));
  try {
    const keyPath = join(dir, 'key.pem');
    const certPath = join(dir, 'cert.pem');
    execFileSync('openssl', [
      'req', '-x509', '-newkey', 'ec',
      '-pkeyopt', 'ec_paramgen_curve:prime256v1',
      '-keyout', keyPath,
      '-out', certPath,
      '-days', '2',
      '-nodes',
      '-subj', `/CN=${domain}`,
      '-addext', `subjectAltName=DNS:${domain}`,
    ], { stdio: 'ignore' });
    return {
      certPem: readFileSync(certPath, 'utf8'),
      keyPem: readFileSync(keyPath, 'utf8'),
    };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('provisions ACME HTTP-01 and Custom PEM TLS via UI, rolls back invalid PEM, and removes TLS', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password, 'the disposable fixture must provide a synthetic admin password').toBeTruthy();
  expect(nginxUrl, 'the disposable fixture must publish Nginx HTTP on loopback').toBeTruthy();
  expect(httpsPort, 'the disposable fixture must publish Nginx HTTPS on loopback').toBeTruthy();
  expect(frontendUrl, 'the disposable fixture must publish the API/UI on loopback').toBeTruthy();

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(new URL('/', frontendUrl).toString());
  await page.getByRole('link', { name: 'Websites', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Websites', exact: true })).toBeVisible();

  // 1. Create static site for ACME HTTP-01 via Pebble
  const acmeSiteEnv = await createSite(page, {
    domain: acmeDomain,
    kind: 'static',
    documentRoot: `/srv/fusionpanel/sites/${acmeDomain}`,
  });
  await waitForWebsite(page, acmeSiteEnv.site.id, 'active');
  await assertNginxHttpResponse(page, acmeDomain, 200, 'Sprint 4 ACME static site marker');

  await page.getByRole('button', { name: `View details for ${acmeDomain}` }).click();
  await expect(page.getByRole('heading', { name: acmeDomain })).toBeVisible();
  await page.getByLabel('ACME directory URL').fill('https://pebble:14000/dir');
  await page.getByLabel('Contact email (optional)').fill('ops@acme-site.test');
  await page.getByRole('button', { name: 'Enable TLS' }).click();

  await expect(page.getByText('Active (ACME HTTP-01)')).toBeVisible({ timeout: 90_000 });
  expect(fetchHttpsWithSni(acmeDomain)).toContain('Sprint 4 ACME static site marker');

  const acmeCert = await getServerCertificate(acmeDomain);
  expect(acmeCert.subjectaltname).toContain(`DNS:${acmeDomain}`);
  expect(JSON.stringify(acmeCert.issuer)).toContain('Pebble');

  await page.getByRole('button', { name: 'Back to websites' }).click();

  // 2. Create reverse proxy site for Custom PEM + invalid rollback + Remove TLS
  const customSiteEnv = await createSite(page, {
    domain: customDomain,
    kind: 'reverse_proxy',
    upstreamUrl: 'http://upstream.sprint3.test:8080',
  });
  await waitForWebsite(page, customSiteEnv.site.id, 'active');
  await assertNginxHttpResponse(page, customDomain, 200, 'Sprint 3 upstream service marker');

  await page.getByRole('button', { name: `View details for ${customDomain}` }).click();
  await expect(page.getByRole('heading', { name: customDomain })).toBeVisible();

  // 2a. Invalid PEM should fail TLS provisioning without breaking existing HTTP serving
  await page.getByLabel('Custom PEM').check();
  await page.getByLabel('Certificate PEM').fill('-----BEGIN CERTIFICATE-----\nINVALID\n-----END CERTIFICATE-----');
  await page.getByLabel('Private key PEM').fill('-----BEGIN PRIVATE KEY-----\nINVALID\n-----END PRIVATE KEY-----');
  await page.getByRole('button', { name: 'Enable TLS' }).click();
  await expect(page.getByText('Failed (Custom PEM)')).toBeVisible({ timeout: 60_000 });
  await assertNginxHttpResponse(page, customDomain, 200, 'Sprint 3 upstream service marker');

  // 2b. Valid Custom PEM succeeds and serves upstream over HTTPS :443
  const { certPem, keyPem } = generateSelfSignedPem(customDomain);
  await page.getByLabel('Certificate PEM').fill(certPem);
  await page.getByLabel('Private key PEM').fill(keyPem);
  await page.getByRole('button', { name: 'Enable TLS' }).click();
  await expect(page.getByText('Active (Custom PEM)')).toBeVisible({ timeout: 60_000 });
  expect(fetchHttpsWithSni(customDomain)).toContain('Sprint 3 upstream service marker');

  const customCert = await getServerCertificate(customDomain);
  expect(customCert.subject?.CN).toBe(customDomain);

  // 2c. Remove TLS reverts site to HTTP-only cleanly
  await page.getByRole('button', { name: 'Remove TLS' }).click();
  await expect(page.getByText('Disabled')).toBeVisible({ timeout: 60_000 });
  await assertNginxHttpResponse(page, customDomain, 200, 'Sprint 3 upstream service marker');
});
