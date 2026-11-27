import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const imageRef = process.env.FUSIONPANEL_S13_IMAGE_REF;
const imageDigest = process.env.FUSIONPANEL_S13_IMAGE_DIGEST;
const sentinelId = process.env.FUSIONPANEL_S13_SENTINEL_ID;
const sentinelUrl = process.env.FUSIONPANEL_S13_SENTINEL_URL;
const composeProject = process.env.FUSIONPANEL_S13_COMPOSE_PROJECT;
const composeFile = process.env.FUSIONPANEL_S13_COMPOSE_FILE;
const suffix = process.env.FUSIONPANEL_S13_SUFFIX || 'local';
const workloadPort = Number(process.env.FUSIONPANEL_S13_WORKLOAD_PORT);
const collisionPort = Number(process.env.FUSIONPANEL_S13_COLLISION_PORT);
const evidencePath = process.env.FUSIONPANEL_S13_EVIDENCE_PATH;

function httpGet(url) {
  return new Promise((resolve, reject) => {
    const req = http.get(url, (res) => {
      let body = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => {
        body += chunk;
      });
      res.on('end', () => resolve({ status: res.statusCode, body }));
    });
    req.on('error', reject);
  });
}

async function expectHttpDown(url, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      await httpGet(url);
    } catch {
      return;
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`Expected ${url} to stop accepting connections within ${timeoutMs}ms`);
}

function assertPortReleased(port) {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once('error', (err) => {
      reject(new Error(`Expected host port ${port} to be released, got ${err.code || err.message}`));
    });
    server.listen(port, '127.0.0.1', () => {
      server.close(() => resolve());
    });
  });
}

async function waitForApiHealth(baseUrl, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${baseUrl}/health`);
      if (res.ok) return;
    } catch {
      // Retry until API is back up after restart.
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`API at ${baseUrl} did not become healthy within ${timeoutMs}ms`);
}

test('launches digest-pinned container, verifies lifecycle, isolation, restart identity, and sentinel preservation', async ({
  page,
}) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(imageRef).toMatch(/@sha256:[a-f0-9]{64}$/);
  expect(imageDigest).toMatch(/^sha256:[a-f0-9]{64}$/);
  expect(sentinelId).toBeTruthy();
  expect(sentinelUrl).toBeTruthy();
  expect(workloadPort).toBeGreaterThan(0);
  expect(collisionPort).toBeGreaterThan(0);

  const primaryName = `ctr-primary-${suffix}`;
  const persistentName = `ctr-persist-${suffix}`;
  const workloadUrl = `http://127.0.0.1:${workloadPort}/`;

  // 1. Login and verify node online
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);

  await page.getByRole('link', { name: 'Nodes' }).click();
  await expect(page.getByText('online', { exact: false }).first()).toBeVisible({ timeout: 30_000 });

  // 2. Launch digest-pinned container via Containers UI
  await page.getByRole('link', { name: 'Containers' }).click();
  await page.getByRole('button', { name: /Launch Container|Create Container/i }).click();
  await page.getByLabel('Name').fill(primaryName);
  await page.getByLabel('Image').fill(imageRef);
  await page.getByLabel(/Host Port/i).fill(String(workloadPort));
  await page.getByLabel(/Container Port/i).fill('8080');
  await page.getByRole('dialog').getByRole('button', { name: /Launch|Create/i }).click();

  const primaryRow = page.locator('tbody tr').filter({ hasText: primaryName });
  await expect(primaryRow).toBeVisible({ timeout: 20_000 });
  await expect(primaryRow).toContainText(/running/i, { timeout: 60_000 });

  // 3. Read real HTTP content from the launched container
  const firstRes = await httpGet(workloadUrl);
  expect(firstRes.status).toBe(200);
  expect(firstRes.body).toContain('fusionpanel-s13-container-fixture-ok');

  // 4. Stop, Restart, Remove + Port Release
  await primaryRow.getByRole('button', { name: /Stop/i }).click();
  await expect(primaryRow).toContainText(/stopped|exited/i, { timeout: 30_000 });
  await expectHttpDown(workloadUrl);

  await primaryRow.getByRole('button', { name: /Start|Restart/i }).click();
  await expect(primaryRow).toContainText(/running/i, { timeout: 30_000 });
  const restartedRes = await httpGet(workloadUrl);
  expect(restartedRes.status).toBe(200);
  expect(restartedRes.body).toContain('fusionpanel-s13-container-fixture-ok');

  await primaryRow.getByRole('button', { name: /Remove|Delete/i }).click();
  await expect(primaryRow).toHaveCount(0, { timeout: 30_000 });
  await assertPortReleased(workloadPort);

  // 5. Launch persistent workload, then verify collision and foreign-tenant rejection
  await page.getByRole('button', { name: /Launch Container|Create Container/i }).click();
  await page.getByLabel('Name').fill(persistentName);
  await page.getByLabel('Image').fill(imageRef);
  await page.getByLabel(/Host Port/i).fill(String(workloadPort));
  await page.getByLabel(/Container Port/i).fill('8080');
  await page.getByRole('dialog').getByRole('button', { name: /Launch|Create/i }).click();

  const persistRow = page.locator('tbody tr').filter({ hasText: persistentName });
  await expect(persistRow).toBeVisible({ timeout: 20_000 });
  await expect(persistRow).toContainText(/running/i, { timeout: 60_000 });

  // Create scoped admin API token for direct API collision & tenant checks
  await page.getByRole('link', { name: 'Administration' }).click();
  await page.getByRole('button', { name: /API Tokens/i }).click();
  await page.getByLabel('Token Name').fill(`s13-owner-${suffix}`);
  await page.getByLabel('Scopes (comma-separated)').fill('containers:read,containers:write');
  await page.getByRole('button', { name: 'Create API Token' }).click();
  const ownerTokenStatus = page.getByRole('status');
  await expect(ownerTokenStatus).toContainText('fp_tok_', { timeout: 15_000 });
  const ownerToken = (await ownerTokenStatus.locator('code').innerText()).trim();

  const listRes = await fetch(`${frontendUrl}/v1/containers`, {
    headers: { Authorization: `Bearer ${ownerToken}` },
  });
  expect(listRes.status).toBe(200);
  const listJson = await listRes.json();
  const items = Array.isArray(listJson) ? listJson : listJson.items || listJson.containers || [];
  const persisted = items.find((c) => c.name === persistentName);
  expect(persisted).toBeTruthy();

  // Reject name/port collision
  const nameCollisionRes = await fetch(`${frontendUrl}/v1/containers`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${ownerToken}`,
      'Idempotency-Key': `s13-name-collision-${suffix}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: persistentName,
      image: imageRef,
      host_port: collisionPort,
      container_port: 8080,
    }),
  });
  expect(nameCollisionRes.status).toBe(409);

  const portCollisionRes = await fetch(`${frontendUrl}/v1/containers`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${ownerToken}`,
      'Idempotency-Key': `s13-port-collision-${suffix}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      name: `ctr-port-collide-${suffix}`,
      image: imageRef,
      host_port: workloadPort,
      container_port: 8080,
    }),
  });
  expect(portCollisionRes.status).toBe(409);

  // Create foreign-tenant operator and verify cross-tenant access is rejected
  await page.getByRole('button', { name: /Operators|Users/i }).click();
  await page.getByRole('button', { name: /Invite Operator/i }).click();
  const foreignUser = `foreign-${suffix}`;
  await page.getByLabel('Username').fill(foreignUser);
  await page.getByLabel('Email Address').fill(`${foreignUser}@e2e.local`);
  await page.getByLabel('Password').fill('ForeignPass123!');
  await page.getByLabel('Role').selectOption('viewer');
  await page.getByRole('button', { name: 'Send Invite' }).click();
  await expect(page.getByText(foreignUser)).toBeVisible({ timeout: 15_000 });

  const loginRes = await fetch(`${frontendUrl}/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: foreignUser, password: 'ForeignPass123!' }),
  });
  expect(loginRes.status).toBe(200);
  const foreignCookie = loginRes.headers.get('set-cookie') || '';

  const foreignReadRes = await fetch(`${frontendUrl}/v1/containers/${persisted.id}`, {
    headers: { Cookie: foreignCookie },
  });
  expect([403, 404]).toContain(foreignReadRes.status);

  const foreignStopRes = await fetch(`${frontendUrl}/v1/containers/${persisted.id}/stop`, {
    method: 'POST',
    headers: {
      Cookie: foreignCookie,
      'Idempotency-Key': `s13-foreign-stop-${suffix}`,
    },
  });
  expect([403, 404]).toContain(foreignStopRes.status);

  const foreignDeleteRes = await fetch(`${frontendUrl}/v1/containers/${persisted.id}`, {
    method: 'DELETE',
    headers: {
      Cookie: foreignCookie,
      'Idempotency-Key': `s13-foreign-del-${suffix}`,
    },
  });
  expect([403, 404]).toContain(foreignDeleteRes.status);

  // 6. Restart API and Agent, verify workload identity is preserved
  const beforeRestartInspect = JSON.parse(
    execFileSync('docker', ['inspect', persisted.runtime_id || persisted.container_id], {
      encoding: 'utf8',
    }),
  )[0];
  expect(beforeRestartInspect.State.Running).toBe(true);

  execFileSync('docker', ['compose', '-p', composeProject, '-f', composeFile, 'restart', 'api', 'agent'], {
    stdio: 'inherit',
  });
  await waitForApiHealth(frontendUrl);

  const afterRestartRes = await fetch(`${frontendUrl}/v1/containers/${persisted.id}`, {
    headers: { Authorization: `Bearer ${ownerToken}` },
  });
  expect(afterRestartRes.status).toBe(200);
  const afterRestartItem = await afterRestartRes.json();
  expect(afterRestartItem.id).toBe(persisted.id);
  expect(afterRestartItem.runtime_id || afterRestartItem.container_id).toBe(
    persisted.runtime_id || persisted.container_id,
  );
  expect(afterRestartItem.image_digest || afterRestartItem.image).toContain(imageDigest);

  await page.getByRole('link', { name: 'Containers' }).click();
  await expect(page.locator('tbody tr').filter({ hasText: persistentName })).toContainText(/running/i, {
    timeout: 30_000,
  });

  // 7. Verify unmanaged sentinel container was untouched
  const sentinelInspect = JSON.parse(
    execFileSync('docker', ['inspect', sentinelId], { encoding: 'utf8' }),
  )[0];
  expect(sentinelInspect.Id).toBe(sentinelId);
  expect(sentinelInspect.State.Running).toBe(true);
  const sentinelHttp = await httpGet(sentinelUrl);
  expect(sentinelHttp.status).toBe(200);
  expect(sentinelHttp.body).toContain('fusionpanel-s13-sentinel-unmanaged-ok');

  if (evidencePath) {
    fs.writeFileSync(
      evidencePath,
      JSON.stringify(
        {
          sprint: '13',
          suffix,
          imageRef,
          imageDigest,
          persistedWorkloadId: persisted.id,
          persistedRuntimeId: persisted.runtime_id || persisted.container_id,
          sentinelId,
          status: 'PASS',
        },
        null,
        2,
      ),
    );
  }
});
