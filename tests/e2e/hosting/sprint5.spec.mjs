import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../browser/package.json', import.meta.url));
const { expect, test } = require('@playwright/test');

const username = process.env.FUSIONPANEL_E2E_USERNAME || 'e2e-admin';
const password = process.env.FUSIONPANEL_E2E_PASSWORD;
const frontendUrl = process.env.FUSIONPANEL_FRONTEND_URL;
const pgContainer = process.env.FUSIONPANEL_S5_PG_CONTAINER;
const mariadbContainer = process.env.FUSIONPANEL_S5_MARIADB_CONTAINER;

function runPsql(dbUser, dbPassword, dbName, sql) {
  return execFileSync('docker', [
    'exec', '-e', `PGPASSWORD=${dbPassword}`, pgContainer,
    'psql', '-h', '127.0.0.1', '-U', dbUser, '-d', dbName, '-t', '-A', '-c', sql,
  ], { encoding: 'utf8' });
}

function runMariaDb(dbUser, dbPassword, dbName, sql) {
  return execFileSync('docker', [
    'exec', '-e', `MYSQL_PWD=${dbPassword}`, mariadbContainer,
    'mariadb', '-h', '127.0.0.1', '-u', dbUser, '-N', '-B', '-D', dbName, '-e', sql,
  ], { encoding: 'utf8' });
}

async function createDbFromUi(page, name, engine) {
  await page.getByRole('button', { name: 'Create Database' }).click();
  await page.getByLabel('Engine:').selectOption(engine);
  await page.getByLabel('Database Name:').fill(name);
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  const item = page.locator('li').filter({ hasText: name });
  await expect(item).toContainText('ready', { timeout: 60_000 });
  return item;
}

async function createAccountAndReadCreds(page, item) {
  await item.getByRole('button', { name: 'Create SQL Account' }).click();
  await expect(page.getByRole('heading', { name: 'Database Credentials' })).toBeVisible();
  const modal = page.locator('div.fixed').filter({ hasText: 'Database Credentials' });
  const text = await modal.innerText();
  const usernameMatch = text.match(/Username:\s*(\S+)/);
  const passwordMatch = text.match(/Fp![A-Za-z0-9]+/);
  expect(usernameMatch).toBeTruthy();
  expect(passwordMatch).toBeTruthy();
  await page.getByRole('button', { name: 'Close' }).click();
  return { dbUser: usernameMatch[1], dbPassword: passwordMatch[0] };
}

test('provisions PG and MariaDB databases and users, enforces isolation, and drops cleanly', async ({ page }) => {
  test.setTimeout(300_000);
  expect(password).toBeTruthy();
  expect(frontendUrl).toBeTruthy();
  expect(pgContainer).toBeTruthy();
  expect(mariadbContainer).toBeTruthy();

  await page.goto('/login');
  await page.getByPlaceholder('Username').fill(username);
  await page.getByPlaceholder('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(new URL('/', frontendUrl).toString());
  await page.getByRole('link', { name: 'Databases', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Databases', exact: true })).toBeVisible();

  // 1. PostgreSQL: create two DBs, issue user on tenant_pg_a, verify access + cross-tenant isolation
  const pgA = await createDbFromUi(page, 'tenant_pg_a', 'postgresql');
  await createDbFromUi(page, 'tenant_pg_b', 'postgresql');
  const pgCreds = await createAccountAndReadCreds(page, pgA);

  await expect.poll(() => {
    try {
      return runPsql(pgCreds.dbUser, pgCreds.dbPassword, 'tenant_pg_a', 'SELECT 1;').trim();
    } catch {
      return 'retry';
    }
  }, { timeout: 30_000, intervals: [500, 1000] }).toBe('1');

  expect(() => runPsql(pgCreds.dbUser, pgCreds.dbPassword, 'tenant_pg_b', 'SELECT 1;')).toThrow();

  // 2. MariaDB: create two DBs, issue user on tenant_mdb_a, verify access + cross-tenant isolation
  const mdbA = await createDbFromUi(page, 'tenant_mdb_a', 'mariadb');
  await createDbFromUi(page, 'tenant_mdb_b', 'mariadb');
  const mdbCreds = await createAccountAndReadCreds(page, mdbA);

  await expect.poll(() => {
    try {
      return runMariaDb(mdbCreds.dbUser, mdbCreds.dbPassword, 'tenant_mdb_a', 'SELECT 1;').trim();
    } catch {
      return 'retry';
    }
  }, { timeout: 30_000, intervals: [500, 1000] }).toBe('1');

  expect(() => runMariaDb(mdbCreds.dbUser, mdbCreds.dbPassword, 'tenant_mdb_b', 'SELECT 1;')).toThrow();

  // 3. Clean drop via UI
  await page.getByRole('button', { name: 'Delete tenant_pg_a' }).click();
  await page.getByRole('button', { name: 'Confirm delete' }).click();
  await expect(page.locator('li').filter({ hasText: 'tenant_pg_a' })).toHaveCount(0, { timeout: 60_000 });
  expect(() => runPsql(pgCreds.dbUser, pgCreds.dbPassword, 'tenant_pg_a', 'SELECT 1;')).toThrow();

  await page.getByRole('button', { name: 'Delete tenant_mdb_a' }).click();
  await page.getByRole('button', { name: 'Confirm delete' }).click();
  await expect(page.locator('li').filter({ hasText: 'tenant_mdb_a' })).toHaveCount(0, { timeout: 60_000 });
  expect(() => runMariaDb(mdbCreds.dbUser, mdbCreds.dbPassword, 'tenant_mdb_a', 'SELECT 1;')).toThrow();
});
