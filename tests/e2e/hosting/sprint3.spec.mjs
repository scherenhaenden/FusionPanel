import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const nginxUrl = process.env.FUSIONPANEL_S3_NGINX_URL;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const staticDomain = 'static.sprint3.test';
const proxyDomain = 'proxy.sprint3.test';
const failedDomain = 'failed.sprint3.test';

function apiUrl(path) {
  return new URL(path, frontendUrl).toString();
}

function exactKeys(value, expected) {
  expect(Object.keys(value).sort()).toEqual([...expected].sort());
}

function assertWebsiteRecord(site, kind) {
  const kindField = kind === 'static' ? 'document_root' : 'upstream_url';
  exactKeys(site, [
    'id', 'domain', 'kind', kindField, 'state', 'last_error', 'created_at', 'updated_at',
  ]);
  expect(site.id).toMatch(/^[0-9a-f-]{36}$/i);
  expect(site.kind).toBe(kind);
  expect(site.last_error === null || typeof site.last_error === 'string').toBe(true);
  expect(Date.parse(site.created_at)).not.toBeNaN();
  expect(Date.parse(site.updated_at)).not.toBeNaN();
}

function assertJobRecord(job, operation, operationPayload) {
  exactKeys(job, [
    'id', 'kind', 'payload', 'state', 'worker_id', 'error_text', 'created_at', 'updated_at',
  ]);
  expect(job.id).toMatch(/^[0-9a-f-]{36}$/i);
  expect(job.kind).toBe('agent_operation');
  expect(['queued', 'running', 'succeeded', 'failed', 'cancelled']).toContain(job.state);
  expect(job.worker_id === null || typeof job.worker_id === 'string').toBe(true);
  expect(job.error_text === null || typeof job.error_text === 'string').toBe(true);
  expect(Date.parse(job.created_at)).not.toBeNaN();
  expect(Date.parse(job.updated_at)).not.toBeNaN();
  exactKeys(job.payload, ['version', 'request_id', 'operation', 'payload']);
  expect(job.payload.version).toBe(1);
  expect(job.payload.request_id).toMatch(/^[0-9a-f-]{36}$/i);
  expect(job.payload.operation).toBe(operation);
  exactKeys(job.payload.payload, Object.keys(operationPayload));
  expect(job.payload.payload).toMatchObject(operationPayload);
  if (operationPayload.spec) {
    exactKeys(job.payload.payload.spec, Object.keys(operationPayload.spec));
  }
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

  const envelope = await response.json();
  exactKeys(envelope, ['site', 'job']);
  assertWebsiteRecord(envelope.site, kind);
  expect(envelope.site).toMatchObject({ domain, kind, state: 'provisioning' });
  const spec = kind === 'static'
    ? { kind, document_root: documentRoot }
    : { kind, upstream_url: upstreamUrl };
  assertJobRecord(envelope.job, 'site.apply', {
    site_id: envelope.site.id,
    domain,
    spec,
  });
  return envelope;
}

async function waitForWebsite(page, siteId, expectedState, kind) {
  let website;
  await expect.poll(async () => {
    const response = await page.request.get(apiUrl(`/v1/websites/${siteId}`));
    if (response.status() !== 200) return `http:${response.status()}`;
    const envelope = await response.json();
    exactKeys(envelope, ['website']);
    website = envelope.website;
    assertWebsiteRecord(website, kind);
    return website.state;
  }, { timeout: 90_000, intervals: [250, 500, 1_000, 2_000] }).toBe(expectedState);
  return website;
}

async function waitForJob(page, jobId, expectedState, operation, operationPayload) {
  let job;
  await expect.poll(async () => {
    const response = await page.request.get(apiUrl(`/v1/jobs/${jobId}`));
    if (response.status() !== 200) return `http:${response.status()}`;
    const envelope = await response.json();
    exactKeys(envelope, ['job']);
    job = envelope.job;
    assertJobRecord(job, operation, operationPayload);
    return job.state;
  }, { timeout: 90_000, intervals: [250, 500, 1_000, 2_000] }).toBe(expectedState);
  return job;
}

async function assertNginxResponse(page, domain, status, bodyText) {
  const response = await page.request.get(`${nginxUrl}/`, { headers: { Host: domain } });
  expect(response.status()).toBe(status);
  if (bodyText) expect(await response.text()).toContain(bodyText);
}

async function deleteSiteFromUi(page, domain) {
  const row = page.getByRole('row').filter({ hasText: domain });
  await expect(row).toBeVisible();
  const responsePromise = page.waitForResponse((response) => (
    response.request().method() === 'DELETE'
    && new URL(response.url()).pathname.startsWith('/v1/websites/')
  ));
  await row.getByRole('button', { name: `Delete ${domain}`, exact: true }).click();
  const confirmation = page.getByRole('group', { name: `Confirm deletion of ${domain}` });
  await confirmation.getByRole('button', { name: 'Confirm delete', exact: true }).click();
  const response = await responsePromise;
  expect(response.status()).toBe(202);

  const envelope = await response.json();
  exactKeys(envelope, ['site', 'job']);
  const kind = envelope.site.kind;
  assertWebsiteRecord(envelope.site, kind);
  expect(envelope.site).toMatchObject({ domain, state: 'deleting' });
  assertJobRecord(envelope.job, 'site.delete', { site_id: envelope.site.id, domain });
  await waitForJob(
    page,
    envelope.job.id,
    'succeeded',
    'site.delete',
    { site_id: envelope.site.id, domain },
  );
  await expect(row).toHaveCount(0, { timeout: 90_000 });
  return envelope.site.id;
}

test('applies and deletes real static/proxy sites and preserves unrelated Nginx config', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password, 'the disposable fixture must provide a synthetic admin password').toBeTruthy();
  expect(nginxUrl, 'the disposable fixture must publish Nginx on loopback').toBeTruthy();
  expect(frontendUrl, 'the disposable fixture must publish the API/UI on loopback').toBeTruthy();

  await assertNginxResponse(page, 'unrelated.sprint3.test', 200, 'unrelated-vhost-marker');

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(new URL('/', frontendUrl).toString());
  await page.getByRole('link', { name: 'Websites', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Websites', exact: true })).toBeVisible();

  const staticOperation = await createSite(page, {
    domain: staticDomain,
    kind: 'static',
    documentRoot: `/srv/fusionpanel/sites/${staticDomain}`,
  });
  const activeStatic = await waitForWebsite(page, staticOperation.site.id, 'active', 'static');
  expect(activeStatic).toMatchObject({ domain: staticDomain, kind: 'static', state: 'active' });
  await waitForJob(page, staticOperation.job.id, 'succeeded', 'site.apply', {
    site_id: staticOperation.site.id,
    domain: staticDomain,
    spec: { kind: 'static', document_root: '/srv/fusionpanel/sites/static.sprint3.test' },
  });
  const staticRow = page.getByRole('row').filter({ hasText: staticDomain });
  await expect(staticRow.getByRole('status')).toHaveAttribute('aria-label', `${staticDomain} state: Active`);
  await assertNginxResponse(page, staticDomain, 200, 'Sprint 3 static site marker');

  const proxyOperation = await createSite(page, {
    domain: proxyDomain,
    kind: 'reverse_proxy',
    upstreamUrl: 'http://upstream.sprint3.test:8080',
  });
  const activeProxy = await waitForWebsite(page, proxyOperation.site.id, 'active', 'reverse_proxy');
  expect(activeProxy).toMatchObject({ domain: proxyDomain, kind: 'reverse_proxy', state: 'active' });
  await waitForJob(page, proxyOperation.job.id, 'succeeded', 'site.apply', {
    site_id: proxyOperation.site.id,
    domain: proxyDomain,
    spec: { kind: 'reverse_proxy', upstream_url: 'http://upstream.sprint3.test:8080' },
  });
  const proxyRow = page.getByRole('row').filter({ hasText: proxyDomain });
  await expect(proxyRow.getByRole('status')).toHaveAttribute('aria-label', `${proxyDomain} state: Active`);
  await assertNginxResponse(page, proxyDomain, 200, 'Sprint 3 upstream service marker');

  const failedOperation = await createSite(page, {
    domain: failedDomain,
    kind: 'reverse_proxy',
    upstreamUrl: 'http://unresolvable.invalid',
  });
  const failedSite = await waitForWebsite(page, failedOperation.site.id, 'failed', 'reverse_proxy');
  expect(failedSite).toMatchObject({ domain: failedDomain, kind: 'reverse_proxy', state: 'failed' });
  expect(failedSite.last_error).toBeTruthy();
  const failedJob = await waitForJob(page, failedOperation.job.id, 'failed', 'site.apply', {
    site_id: failedOperation.site.id,
    domain: failedDomain,
    spec: { kind: 'reverse_proxy', upstream_url: 'http://unresolvable.invalid' },
  });
  expect(failedJob.error_text).toBeTruthy();
  const failedRow = page.getByRole('row').filter({ hasText: failedDomain });
  await expect(failedRow.getByRole('status')).toHaveAttribute('aria-label', `${failedDomain} state: Failed`);

  await assertNginxResponse(page, staticDomain, 200, 'Sprint 3 static site marker');
  await assertNginxResponse(page, proxyDomain, 200, 'Sprint 3 upstream service marker');
  await assertNginxResponse(page, failedDomain, 404);
  await assertNginxResponse(page, 'unrelated.sprint3.test', 200, 'unrelated-vhost-marker');

  const noFailedSiteConfig = await page.request.get(apiUrl(`/v1/websites/${failedOperation.site.id}`));
  expect(noFailedSiteConfig.status()).toBe(200);
  const failedDetailEnvelope = await noFailedSiteConfig.json();
  exactKeys(failedDetailEnvelope, ['website']);
  assertWebsiteRecord(failedDetailEnvelope.website, 'reverse_proxy');
  expect(failedDetailEnvelope.website.state).toBe('failed');

  const listResponse = await page.request.get(apiUrl('/v1/websites'));
  expect(listResponse.status()).toBe(200);
  const listEnvelope = await listResponse.json();
  exactKeys(listEnvelope, ['websites']);
  for (const site of listEnvelope.websites) assertWebsiteRecord(site, site.kind);
  expect(listEnvelope.websites.map((site) => site.id)).toEqual(
    expect.arrayContaining([staticOperation.site.id, proxyOperation.site.id, failedOperation.site.id]),
  );

  const deletedStaticId = await deleteSiteFromUi(page, staticDomain);
  const deletedProxyId = await deleteSiteFromUi(page, proxyDomain);
  await expect.poll(async () => (
    await page.request.get(apiUrl(`/v1/websites/${deletedStaticId}`))
  ).status(), { timeout: 30_000 }).toBe(404);
  await expect.poll(async () => (
    await page.request.get(apiUrl(`/v1/websites/${deletedProxyId}`))
  ).status(), { timeout: 30_000 }).toBe(404);

  await assertNginxResponse(page, staticDomain, 404);
  await assertNginxResponse(page, proxyDomain, 404);
  await assertNginxResponse(page, 'unrelated.sprint3.test', 200, 'unrelated-vhost-marker');
});
