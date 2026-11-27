import { execFileSync } from 'node:child_process';
import crypto from 'node:crypto';
import dns from 'node:dns';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { createRequire } from 'node:module';
import tls from 'node:tls';

const require = createRequire(new URL('../../../../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const bind9Port = Number(process.env.FUSIONPANEL_S11_BIND9_PORT);
const smtpPort = Number(process.env.FUSIONPANEL_S11_SMTP_PORT);
const submissionPort = Number(process.env.FUSIONPANEL_S11_SUBMISSION_PORT);
const imapPort = Number(process.env.FUSIONPANEL_S11_IMAP_PORT);
const mailDir = process.env.FUSIONPANEL_S11_MAIL_DIR;
const composeProject = process.env.FUSIONPANEL_S11_PROJECT;
const composeFile = process.env.FUSIONPANEL_S11_COMPOSE_FILE;

const DOMAIN = 'mail-e2e.local';
const ALICE_PASS = 'AliceMailPass123!';
const BOB_PASS = 'BobMailPass123!';

function createBindResolver() {
  const resolver = new dns.promises.Resolver();
  resolver.setServers([`127.0.0.1:${bind9Port}`]);
  return resolver;
}

async function resolveTxt(name) {
  try {
    const records = await createBindResolver().resolveTxt(name);
    return records.map((chunks) => chunks.join('')).join('\n');
  } catch {
    return '';
  }
}

class LineSocket {
  constructor(socket) {
    this.socket = socket;
    this.buffer = '';
    this.waiters = [];
    this.closed = false;
    this.error = null;
    this.attach(socket);
  }

  attach(socket) {
    this.socket = socket;
    socket.setEncoding('utf8');
    socket.on('data', (chunk) => {
      this.buffer += chunk;
      this.flush();
    });
    socket.on('error', (err) => {
      this.error = err;
      this.flush();
    });
    socket.on('close', () => {
      this.closed = true;
      this.flush();
    });
  }

  upgradeTls() {
    return new Promise((resolve, reject) => {
      this.socket.removeAllListeners('data');
      this.socket.removeAllListeners('error');
      this.socket.removeAllListeners('close');
      const tlsSock = tls.connect(
        { socket: this.socket, rejectUnauthorized: false, servername: 'mail.mail-e2e.local' },
        () => {
          this.attach(tlsSock);
          resolve();
        },
      );
      tlsSock.once('error', reject);
    });
  }

  flush() {
    while (this.waiters.length > 0) {
      const idx = this.buffer.indexOf('\r\n');
      if (idx !== -1) {
        const line = this.buffer.slice(0, idx);
        this.buffer = this.buffer.slice(idx + 2);
        this.waiters.shift().resolve(line);
        continue;
      }
      if (this.error) {
        this.waiters.shift().reject(this.error);
        continue;
      }
      if (this.closed) {
        this.waiters.shift().reject(new Error('Socket closed unexpectedly'));
        continue;
      }
      break;
    }
  }

  readLine(timeoutMs = 15_000) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Socket readLine timed out')), timeoutMs);
      this.waiters.push({
        resolve: (val) => {
          clearTimeout(timer);
          resolve(val);
        },
        reject: (err) => {
          clearTimeout(timer);
          reject(err);
        },
      });
      this.flush();
    });
  }

  async readSmtpResponse() {
    const lines = [];
    while (true) {
      const line = await this.readLine();
      lines.push(line);
      if (/^\d{3} /.test(line)) {
        return { code: Number(line.slice(0, 3)), text: lines.join('\n') };
      }
    }
  }

  write(data) {
    return new Promise((resolve, reject) => {
      this.socket.write(data, (err) => (err ? reject(err) : resolve()));
    });
  }

  close() {
    this.socket.destroy();
  }
}

async function connectSocket(port) {
  const sock = await new Promise((resolve, reject) => {
    const s = net.connect({ host: '127.0.0.1', port }, () => resolve(s));
    s.once('error', reject);
  });
  return new LineSocket(sock);
}

async function submitSmtpAuth({ port, user, pass, from, to, subject, messageId, bodyText }) {
  const client = await connectSocket(port);
  try {
    const greet = await client.readSmtpResponse();
    expect(greet.code).toBe(220);

    await client.write('EHLO e2e-client.local\r\n');
    const ehlo1 = await client.readSmtpResponse();
    expect(ehlo1.code).toBe(250);

    await client.write('STARTTLS\r\n');
    const starttls = await client.readSmtpResponse();
    expect(starttls.code).toBe(220);

    await client.upgradeTls();
    await client.write('EHLO e2e-client.local\r\n');
    const ehlo2 = await client.readSmtpResponse();
    expect(ehlo2.code).toBe(250);

    const authToken = Buffer.from(`\0${user}\0${pass}`, 'utf8').toString('base64');
    await client.write(`AUTH PLAIN ${authToken}\r\n`);
    const authResp = await client.readSmtpResponse();
    if (authResp.code !== 235) {
      return { ok: false, stage: 'auth', ...authResp };
    }

    await client.write(`MAIL FROM:<${from}>\r\n`);
    const mailFrom = await client.readSmtpResponse();
    expect(mailFrom.code).toBe(250);

    await client.write(`RCPT TO:<${to}>\r\n`);
    const rcptTo = await client.readSmtpResponse();
    expect(rcptTo.code).toBe(250);

    await client.write('DATA\r\n');
    const dataResp = await client.readSmtpResponse();
    expect(dataResp.code).toBe(354);

    const rfc822 = [
      `From: <${from}>`,
      `To: <${to}>`,
      `Subject: ${subject}`,
      `Date: ${new Date().toUTCString()}`,
      `Message-ID: ${messageId}`,
      'MIME-Version: 1.0',
      'Content-Type: text/plain; charset=utf-8',
      '',
      bodyText,
      '.',
      '',
    ].join('\r\n');
    await client.write(rfc822);
    const endResp = await client.readSmtpResponse();
    expect(endResp.code).toBe(250);

    await client.write('QUIT\r\n');
    await client.readSmtpResponse();
    return { ok: true, ...endResp };
  } finally {
    client.close();
  }
}

async function attemptUnauthRelay({ port, from, to }) {
  const client = await connectSocket(port);
  try {
    const greet = await client.readSmtpResponse();
    expect(greet.code).toBe(220);
    await client.write('EHLO external.test\r\n');
    await client.readSmtpResponse();
    await client.write(`MAIL FROM:<${from}>\r\n`);
    await client.readSmtpResponse();
    await client.write(`RCPT TO:<${to}>\r\n`);
    const rcptResp = await client.readSmtpResponse();
    await client.write('QUIT\r\n');
    return rcptResp;
  } finally {
    client.close();
  }
}

async function imapCommand(client, tag, cmd) {
  await client.write(`${tag} ${cmd}\r\n`);
  const lines = [];
  while (true) {
    const line = await client.readLine();
    lines.push(line);
    if (line.startsWith(`${tag} `)) {
      const ok = line.startsWith(`${tag} OK`);
      return { ok, statusLine: line, lines };
    }
  }
}

async function imapFetchMessages({ port, user, pass, subjectFilter }) {
  const client = await connectSocket(port);
  try {
    const greet = await client.readLine();
    expect(greet).toContain('* OK');

    const starttls = await imapCommand(client, 'A1', 'STARTTLS');
    expect(starttls.ok).toBe(true);
    await client.upgradeTls();

    const login = await imapCommand(client, 'A2', `LOGIN "${user}" "${pass}"`);
    if (!login.ok) {
      return { ok: false, stage: 'login', statusLine: login.statusLine, messages: [] };
    }

    const select = await imapCommand(client, 'A3', 'SELECT INBOX');
    expect(select.ok).toBe(true);

    const search = await imapCommand(client, 'A4', `SEARCH SUBJECT "${subjectFilter}"`);
    expect(search.ok).toBe(true);
    const searchLine = search.lines.find((l) => l.startsWith('* SEARCH')) || '* SEARCH';
    const uids = searchLine
      .replace('* SEARCH', '')
      .trim()
      .split(/\s+/)
      .filter(Boolean);

    const messages = [];
    for (let i = 0; i < uids.length; i++) {
      const seq = uids[i];
      await client.write(`F${i} FETCH ${seq} RFC822\r\n`);
      const fetchedLines = [];
      let inLiteral = false;
      let literalRemaining = 0;
      let raw = '';
      while (true) {
        const line = await client.readLine();
        if (line.startsWith(`F${i} `)) {
          break;
        }
        if (!inLiteral && /\{\d+\}$/.test(line)) {
          literalRemaining = Number(line.match(/\{(\d+)\}$/)[1]);
          inLiteral = true;
          continue;
        }
        if (inLiteral) {
          if (raw.length > 0 || fetchedLines.length > 0) {
            raw += '\r\n';
          }
          raw += line;
          literalRemaining -= Buffer.byteLength(line, 'utf8') + 2;
          if (literalRemaining <= 0) {
            inLiteral = false;
          }
        }
      }
      messages.push(raw);
    }

    await imapCommand(client, 'A9', 'LOGOUT');
    return { ok: true, messages };
  } finally {
    client.close();
  }
}

function unfoldHeaders(rawHeaderBlock) {
  const lines = rawHeaderBlock.split(/\r?\n/);
  const headers = [];
  for (const line of lines) {
    if (/^[ \t]/.test(line) && headers.length > 0) {
      headers[headers.length - 1] += `\r\n${line}`;
    } else if (line.length > 0) {
      headers.push(line);
    }
  }
  return headers;
}

function canonicalizeHeaderRelaxed(headerLine) {
  const colon = headerLine.indexOf(':');
  const name = headerLine.slice(0, colon).trim().toLowerCase();
  const value = headerLine
    .slice(colon + 1)
    .replace(/\r?\n[ \t]+/g, ' ')
    .replace(/[ \t]+/g, ' ')
    .trim();
  return `${name}:${value}`;
}

function canonicalizeBodyRelaxed(rawBody) {
  const lines = rawBody.split(/\r?\n/).map((line) => line.replace(/[ \t]+$/g, '').replace(/[ \t]+/g, ' '));
  while (lines.length > 0 && lines[lines.length - 1] === '') {
    lines.pop();
  }
  return lines.length === 0 ? '\r\n' : `${lines.join('\r\n')}\r\n`;
}

async function verifyDkimAgainstFixtureDns(rawMessage) {
  const sep = rawMessage.indexOf('\r\n\r\n');
  expect(sep).toBeGreaterThan(0);
  const rawHeaders = rawMessage.slice(0, sep);
  const rawBody = rawMessage.slice(sep + 4);
  const headers = unfoldHeaders(rawHeaders);

  const dkimHeader = headers.find((h) => h.toLowerCase().startsWith('dkim-signature:'));
  expect(dkimHeader, 'Message must contain a DKIM-Signature header').toBeTruthy();

  const tagStr = dkimHeader.slice(dkimHeader.indexOf(':') + 1).replace(/\r?\n[ \t]*/g, '');
  const tags = Object.fromEntries(
    tagStr
      .split(';')
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => {
        const eq = part.indexOf('=');
        return [part.slice(0, eq).trim(), part.slice(eq + 1).trim()];
      }),
  );

  expect(tags.d).toBe(DOMAIN);
  expect(tags.s).toBe('default');
  expect(tags.a).toBe('rsa-sha256');

  // Verify body hash (bh)
  const canonBody = canonicalizeBodyRelaxed(rawBody);
  const computedBh = crypto.createHash('sha256').update(canonBody, 'utf8').digest('base64');
  expect(computedBh).toBe(tags.bh);

  // Query public key from fixture authoritative DNS (bind9)
  const txt = await resolveTxt(`${tags.s}._domainkey.${tags.d}`);
  expect(txt).toContain('v=DKIM1');
  const pMatch = txt.match(/p=([A-Za-z0-9+/=]+)/);
  expect(pMatch).toBeTruthy();
  const pubPem = `-----BEGIN PUBLIC KEY-----\n${pMatch[1].match(/.{1,64}/g).join('\n')}\n-----END PUBLIC KEY-----\n`;

  // Reconstruct signed header list in h= order (consuming from bottom-up per RFC 6376)
  const signedNames = tags.h.split(':').map((s) => s.trim().toLowerCase());
  const usedIndices = new Set();
  const signingLines = [];
  for (const name of signedNames) {
    let foundIdx = -1;
    for (let i = headers.length - 1; i >= 0; i--) {
      if (usedIndices.has(i)) continue;
      const hName = headers[i].slice(0, headers[i].indexOf(':')).trim().toLowerCase();
      if (hName === name) {
        foundIdx = i;
        break;
      }
    }
    expect(foundIdx, `Signed header ${name} missing`).toBeGreaterThanOrEqual(0);
    usedIndices.add(foundIdx);
    signingLines.push(canonicalizeHeaderRelaxed(headers[foundIdx]));
  }

  // Append DKIM-Signature header with b= emptied
  const dkimWithEmptyB = dkimHeader.replace(/([;\s]b=)[^;]*/, '$1');
  signingLines.push(canonicalizeHeaderRelaxed(dkimWithEmptyB));

  const verifier = crypto.createVerify('RSA-SHA256');
  verifier.update(signingLines.join('\r\n'), 'utf8');
  const valid = verifier.verify(pubPem, tags.b.replace(/\s+/g, ''), 'base64');
  expect(valid, 'DKIM RSA-SHA256 signature must verify against fixture DNS public key').toBe(true);
}

test('Sprint 11 mail E2E: domain, mailboxes, alias, DKIM DNS, SMTP AUTH, IMAP, security, failure/retry, restart durability', async ({
  page,
}) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(bind9Port).toBeGreaterThan(0);
  expect(smtpPort).toBeGreaterThan(0);
  expect(submissionPort).toBeGreaterThan(0);
  expect(imapPort).toBeGreaterThan(0);

  // 1. Sign in as SuperAdmin
  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);

  // 2. Create DNS zone mail-e2e.local so DKIM TXT can be published to authoritative BIND9
  await page.getByRole('link', { name: 'DNS', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'DNS Zones & DKIM Sync' })).toBeVisible();
  await page.getByRole('button', { name: 'Create Zone' }).click();
  await page.getByLabel('Domain').fill(DOMAIN);
  await page.getByRole('button', { name: 'Save Zone' }).click();
  await expect(page.getByRole('heading', { name: DOMAIN })).toBeVisible();
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // 3. Navigate to /mail, create domain, 2 mailboxes, and 1 local alias
  await page.getByRole('link', { name: 'Mail', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Mail Domains & DKIM' })).toBeVisible();

  await page.getByRole('button', { name: 'Create Mail Domain' }).click();
  await page.locator('#mail-domain-name').fill(DOMAIN);
  const nodeSelect = page.locator('#mail-domain-node');
  await expect(nodeSelect.locator('option')).toHaveCount(2, { timeout: 30_000 });
  const nodeVal = await nodeSelect.locator('option').nth(1).getAttribute('value');
  await nodeSelect.selectOption(nodeVal);
  const dnsZoneSelect = page.locator('#mail-domain-dns-zone');
  const zoneVal = await dnsZoneSelect.locator('option', { hasText: DOMAIN }).getAttribute('value');
  await dnsZoneSelect.selectOption(zoneVal);
  await page.getByRole('button', { name: 'Save Domain' }).click();

  await expect(page.getByRole('heading', { name: DOMAIN })).toBeVisible({ timeout: 15_000 });
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // Mailbox 1: alice@mail-e2e.local
  await page.locator('#mailbox-local-part').fill('alice');
  await page.locator('#mailbox-password').fill(ALICE_PASS);
  await page.getByRole('button', { name: 'Add Mailbox' }).click();
  await expect(page.locator('tbody')).toContainText(`alice@${DOMAIN}`, { timeout: 30_000 });
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // Mailbox 2: bob@mail-e2e.local
  await page.locator('#mailbox-local-part').fill('bob');
  await page.locator('#mailbox-password').fill(BOB_PASS);
  await page.getByRole('button', { name: 'Add Mailbox' }).click();
  await expect(page.locator('tbody')).toContainText(`bob@${DOMAIN}`, { timeout: 30_000 });
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // Local Alias: support@mail-e2e.local -> bob@mail-e2e.local
  await page.locator('#alias-local-part').fill('support');
  await page.locator('#alias-destinations').fill(`bob@${DOMAIN}`);
  await page.getByRole('button', { name: 'Add Alias' }).click();
  await expect(page.locator('tbody')).toContainText(`support@${DOMAIN}`, { timeout: 30_000 });
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // 4. Configure DKIM (selector: default) and confirm TXT record
  await page.locator('#dkim-selector').fill('default');
  await page.locator('#dkim-dns-zone').selectOption(zoneVal);
  await page.getByRole('button', { name: 'Save DKIM' }).click();
  const dkimPre = page.locator('pre').filter({ hasText: 'v=DKIM1;' });
  await expect(dkimPre).toBeVisible({ timeout: 60_000 });
  const dkimTxt = (await dkimPre.innerText()).trim();

  // Publish DKIM TXT record via /dns Sync DKIM and verify authoritative BIND9 query
  await page.getByRole('link', { name: 'DNS', exact: true }).click();
  await page.getByLabel('DKIM Selector').fill('default');
  await page.getByLabel('DKIM TXT Record').fill(dkimTxt);
  await page.getByRole('button', { name: 'Sync DKIM' }).click();
  await expect(page.locator('tbody')).toContainText('default._domainkey', { timeout: 30_000 });

  await expect
    .poll(() => resolveTxt(`default._domainkey.${DOMAIN}`), {
      timeout: 30_000,
      intervals: [500, 1000],
    })
    .toContain('v=DKIM1;');

  // 5. SMTP AUTH submission to alias -> IMAP retrieval in bob@mail-e2e.local -> DKIM verification
  const nonce = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const uniqueSubject = `Sprint 11 Mail E2E ${nonce}`;
  const submitRes = await submitSmtpAuth({
    port: submissionPort,
    user: `alice@${DOMAIN}`,
    pass: ALICE_PASS,
    from: `alice@${DOMAIN}`,
    to: `support@${DOMAIN}`,
    subject: uniqueSubject,
    messageId: `<sprint11-${nonce}@${DOMAIN}>`,
    bodyText: `Hello Bob via support alias (${nonce}).`,
  });
  expect(submitRes.ok).toBe(true);

  let fetchedMessages = [];
  await expect
    .poll(
      async () => {
        const res = await imapFetchMessages({
          port: imapPort,
          user: `bob@${DOMAIN}`,
          pass: BOB_PASS,
          subjectFilter: uniqueSubject,
        });
        fetchedMessages = res.messages;
        return res.ok ? res.messages.length : 0;
      },
      { timeout: 30_000, intervals: [500, 1000] },
    )
    .toBe(1);

  await verifyDkimAgainstFixtureDns(fetchedMessages[0]);

  // 6. Negative security checks: bad credentials & unauthenticated relay
  const badSmtp = await submitSmtpAuth({
    port: submissionPort,
    user: `alice@${DOMAIN}`,
    pass: 'WrongPassword999!',
    from: `alice@${DOMAIN}`,
    to: `bob@${DOMAIN}`,
    subject: 'should-fail',
    messageId: `<bad-${nonce}@${DOMAIN}>`,
    bodyText: 'nope',
  });
  expect(badSmtp.ok).toBe(false);
  expect(badSmtp.code).toBe(535);

  const badImap = await imapFetchMessages({
    port: imapPort,
    user: `bob@${DOMAIN}`,
    pass: 'WrongPassword999!',
    subjectFilter: uniqueSubject,
  });
  expect(badImap.ok).toBe(false);

  const relayResp = await attemptUnauthRelay({
    port: smtpPort,
    from: 'attacker@external.test',
    to: 'victim@external.test',
  });
  expect(relayResp.code).toBeGreaterThanOrEqual(500);

  // 7. Denied RBAC / cross-tenant-style API & UI access (Viewer + scoped token)
  await page.getByRole('link', { name: 'Administration' }).click();
  await page.getByRole('button', { name: /Invite Operator/i }).click();
  await page.getByLabel('Username').fill('viewer-mail-e2e');
  await page.getByLabel('Email Address').fill('viewer-mail@e2e.local');
  await page.getByLabel('Password').fill('ViewerPass123!');
  await page.getByLabel('Role').selectOption('viewer');
  await page.getByRole('button', { name: 'Send Invite' }).click();
  await expect(page.getByText('viewer-mail-e2e')).toBeVisible({ timeout: 15_000 });

  await page.getByRole('button', { name: /API Tokens/i }).click();
  await page.getByLabel('Token Name').fill('s11-sites-only');
  await page.getByLabel('Scopes (comma-separated)').fill('sites:read');
  await page.getByRole('button', { name: 'Create API Token' }).click();
  const tokenStatus = page.getByRole('status');
  await expect(tokenStatus).toContainText('fp_tok_', { timeout: 15_000 });
  const scopedToken = await tokenStatus.locator('code').innerText();

  const deniedApiRes = await fetch(`${frontendUrl}/v1/mail/domains`, {
    headers: { Authorization: `Bearer ${scopedToken}` },
  });
  expect(deniedApiRes.status).toBe(403);

  const unauthApiRes = await fetch(`${frontendUrl}/v1/mail/domains`);
  expect(unauthApiRes.status).toBe(401);

  await page.getByTitle('Logout').click();
  await expect(page).toHaveURL(/\/login/);
  await page.getByPlaceholder('Username').fill('viewer-mail-e2e');
  await page.getByPlaceholder('Password').fill('ViewerPass123!');
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.goto('/mail');
  await expect(page.getByRole('alert')).toContainText(/Permission denied|403/i, { timeout: 15_000 });

  // Log back in as SuperAdmin for staged failure/retry + restart durability
  await page.getByTitle('Logout').click();
  await expect(page).toHaveURL(/\/login/);
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: /sign in/i }).click();
  await expect(page).toHaveURL(/\/$/);
  await page.getByRole('link', { name: 'Mail', exact: true }).click();

  // 8. Staged config failure & unchanged delivery, then retry
  fs.writeFileSync(path.join(mailDir, 'state', 'fail-next-validate'), '1\n');
  await page.locator('#mailbox-local-part').fill('charlie');
  await page.locator('#mailbox-password').fill('CharliePass123!');
  await page.getByRole('button', { name: 'Add Mailbox' }).click();
  await expect(page.locator('text=failed').first()).toBeVisible({ timeout: 60_000 });

  // Confirm existing delivery still works unchanged despite staged failure
  const postFailNonce = `${nonce}-postfail`;
  const postFailSubmit = await submitSmtpAuth({
    port: submissionPort,
    user: `alice@${DOMAIN}`,
    pass: ALICE_PASS,
    from: `alice@${DOMAIN}`,
    to: `support@${DOMAIN}`,
    subject: `Sprint 11 Post-Fail ${postFailNonce}`,
    messageId: `<sprint11-${postFailNonce}@${DOMAIN}>`,
    bodyText: 'Delivery remains intact after staged validation failure.',
  });
  expect(postFailSubmit.ok).toBe(true);

  // Retry (sentinel was consumed on the failed validation)
  await page.locator('#mailbox-local-part').fill('charlie');
  await page.locator('#mailbox-password').fill('CharliePass123!');
  await page.getByRole('button', { name: 'Add Mailbox' }).click();
  await expect(page.locator('tbody')).toContainText(`charlie@${DOMAIN}`, { timeout: 30_000 });
  await expect(page.locator('text=ready').first()).toBeVisible({ timeout: 60_000 });

  // 9. API & agent restart durability + single copy of original message
  execFileSync(
    'docker',
    ['compose', '-p', composeProject, '-f', composeFile, 'restart', 'api', 'node'],
    { stdio: 'inherit' },
  );

  await expect
    .poll(
      async () => {
        try {
          const r = await fetch(`${frontendUrl}/v1/health`);
          return r.status;
        } catch {
          return 0;
        }
      },
      { timeout: 60_000, intervals: [500, 1000] },
    )
    .toBe(200);

  await page.goto('/mail');
  await expect(page.getByRole('heading', { name: DOMAIN })).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('tbody')).toContainText(`alice@${DOMAIN}`);
  await expect(page.locator('tbody')).toContainText(`bob@${DOMAIN}`);
  await expect(page.locator('tbody')).toContainText(`support@${DOMAIN}`);

  const durableImap = await imapFetchMessages({
    port: imapPort,
    user: `bob@${DOMAIN}`,
    pass: BOB_PASS,
    subjectFilter: uniqueSubject,
  });
  expect(durableImap.ok).toBe(true);
  expect(durableImap.messages).toHaveLength(1);
});
