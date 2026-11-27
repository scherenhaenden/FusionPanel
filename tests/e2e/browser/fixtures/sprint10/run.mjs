#!/usr/bin/env node
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const browserDir = path.resolve(__dirname, '../..');
const repoRoot = path.resolve(browserDir, '../../..');

const apiBin = process.env.FUSIONPANEL_API_BINARY || path.join(repoRoot, 'target/debug/fusionpanel-api');
const agentBin = process.env.FUSIONPANEL_AGENT_BINARY || path.join(repoRoot, 'crates/agent/target/debug/fusionpanel-agent');
const cliBin = process.env.FUSIONPANEL_CLI_BINARY || path.join(repoRoot, 'target/debug/fusionpanel-cli');
const frontendDir = process.env.FUSIONPANEL_FRONTEND_DIR || path.join(repoRoot, 'frontend/dist');

for (const [label, target, isDir] of [
  ['API binary', apiBin, false],
  ['Agent binary', agentBin, false],
  ['CLI binary', cliBin, false],
  ['Frontend dist', path.join(frontendDir, 'index.html'), false],
]) {
  if (!fs.existsSync(target) || (isDir && !fs.statSync(target).isDirectory())) {
    console.error(`BLOCKED: missing ${label} at ${target}`);
    process.exit(2);
  }
}

function reservePort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, '127.0.0.1', () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
    srv.on('error', reject);
  });
}

function verifySignature(secret, body, header) {
  const parts = Object.fromEntries(
    String(header || '')
      .split(',')
      .map((item) => item.split('=').map((s) => s.trim())),
  );
  if (!parts.t || !parts.v1) return false;
  const expected = crypto
    .createHmac('sha256', secret)
    .update(`${parts.t}.${body}`)
    .digest('hex');
  try {
    return crypto.timingSafeEqual(Buffer.from(parts.v1, 'hex'), Buffer.from(expected, 'hex'));
  } catch {
    return false;
  }
}

async function waitForHealth(apiUrl) {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${apiUrl}/v1/health`);
      if (res.ok) return;
    } catch {
      // retry
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error('BLOCKED: fusionpanel-api did not become healthy');
}

const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fusionpanel-s10-browser-'));
const dbPath = path.join(tempDir, 'fusionpanel-s10-browser.sqlite');
const receiptFile = path.join(tempDir, 'webhooks.json');
fs.writeFileSync(receiptFile, '[]');

const adminToken = `s10-browser-admin-${crypto.randomBytes(16).toString('hex')}`;
const encryptionKey = crypto.randomBytes(32).toString('hex');
const webhookSecret = `s10-browser-webhook-secret-${crypto.randomBytes(16).toString('hex')}`;
const username = 'e2e-admin';
const password = `s10-browser-pass-${crypto.randomBytes(12).toString('hex')}`;
const nodeName = 'sprint10-disposable-agent';

const receipts = [];
let webhookServer;
let apiProc;
let agentProc;

try {
  execFileSync(
    cliBin,
    ['--json', 'bootstrap', 'operator', '--database-url', dbPath, '--username', username, '--password', password],
    { stdio: 'pipe' },
  );

  const apiPort = await reservePort();
  const webhookPort = await reservePort();
  const apiUrl = `http://127.0.0.1:${apiPort}`;
  const webhookUrl = `http://127.0.0.1:${webhookPort}/hook`;

  webhookServer = http.createServer((req, res) => {
    const chunks = [];
    req.on('data', (c) => chunks.push(c));
    req.on('end', () => {
      const body = Buffer.concat(chunks).toString('utf8');
      const sig = req.headers['x-fusionpanel-signature'];
      const deliveryId = req.headers['x-fusionpanel-delivery-id'];
      let event = null;
      try {
        event = JSON.parse(body).event;
      } catch {
        // ignore
      }
      receipts.push({
        event,
        deliveryId: deliveryId || '',
        verified: verifySignature(webhookSecret, body, sig),
      });
      fs.writeFileSync(receiptFile, JSON.stringify(receipts, null, 2));
      res.writeHead(204);
      res.end();
    });
  });
  await new Promise((resolve) => webhookServer.listen(webhookPort, '127.0.0.1', resolve));

  apiProc = spawn(apiBin, [], {
    env: {
      PATH: process.env.PATH || '',
      TMPDIR: tempDir,
      FUSIONPANEL_ADMIN_TOKEN: adminToken,
      FUSIONPANEL_DATABASE_URL: dbPath,
      FUSIONPANEL_WEBHOOK_ENCRYPTION_KEY: encryptionKey,
      FUSIONPANEL_FRONTEND_DIR: frontendDir,
      FUSIONPANEL_LISTEN: `127.0.0.1:${apiPort}`,
    },
    stdio: 'ignore',
  });
  await waitForHealth(apiUrl);

  const regRes = await fetch(`${apiUrl}/v1/nodes/register`, {
    method: 'POST',
    headers: {
      authorization: `Bearer ${adminToken}`,
      'content-type': 'application/json',
    },
    body: JSON.stringify({ name: nodeName }),
  });
  if (!regRes.ok) {
    throw new Error(`Node registration failed: ${regRes.status}`);
  }
  const { node_id: nodeId, token: nodeToken } = await regRes.json();

  agentProc = spawn(agentBin, [], {
    env: {
      PATH: process.env.PATH || '',
      TMPDIR: tempDir,
      FUSIONPANEL_AGENT_MODE: 'http',
      FUSIONPANEL_API_URL: apiUrl,
      FUSIONPANEL_NODE_ID: nodeId,
      FUSIONPANEL_NODE_TOKEN: nodeToken,
    },
    stdio: 'ignore',
  });

  const playwrightBin = path.join(browserDir, 'node_modules/.bin/playwright');
  const exitCode = await new Promise((resolve, reject) => {
    const child = spawn(
      playwrightBin,
      ['test', '--reporter=list', 'tests/observability-dashboard.spec.js'],
      {
        cwd: browserDir,
        stdio: 'inherit',
        env: {
          ...process.env,
          FUSIONPANEL_FRONTEND_URL: apiUrl,
          FUSIONPANEL_E2E_USERNAME: username,
          FUSIONPANEL_E2E_PASSWORD: password,
          FUSIONPANEL_E2E_NODE_NAME: nodeName,
          FUSIONPANEL_E2E_WEBHOOK_URL: webhookUrl,
          FUSIONPANEL_E2E_WEBHOOK_SECRET: webhookSecret,
          FUSIONPANEL_E2E_WEBHOOK_RECEIPT_FILE: receiptFile,
        },
      },
    );
    child.on('error', reject);
    child.on('exit', (code) => resolve(code ?? 1));
  });
  if (exitCode !== 0) {
    process.exitCode = exitCode;
  }
} finally {
  agentProc?.kill('SIGTERM');
  apiProc?.kill('SIGTERM');
  webhookServer?.close();
  fs.rmSync(tempDir, { recursive: true, force: true });
}
