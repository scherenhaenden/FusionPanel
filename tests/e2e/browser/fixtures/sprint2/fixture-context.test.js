import assert from 'node:assert/strict';
import test from 'node:test';
import { assertDisposableFixture, restartDisposableServices } from './fixture-context.js';

function fixtureEnv(overrides = {}) {
  const repoRoot = '/tmp/fusionpanel-sprint2-checkout';
  const composeProject = 'fp-s2-e2e-local-1234';
  return {
    FUSIONPANEL_E2E_DISPOSABLE: '1',
    FUSIONPANEL_E2E_REPO_ROOT: repoRoot,
    FUSIONPANEL_E2E_COMPOSE_PROJECT: composeProject,
    FUSIONPANEL_E2E_COMPOSE_FILE: `${repoRoot}/tests/e2e/browser/fixtures/sprint2/docker-compose.yml`,
    FUSIONPANEL_E2E_PORT: '18080',
    FUSIONPANEL_E2E_IMAGE: `fusionpanel-sprint2-e2e:${composeProject}`,
    FUSIONPANEL_FRONTEND_URL: 'http://127.0.0.1:18080',
    ...overrides,
  };
}

test('fixture metadata blocks runs without the disposable marker', () => {
  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_E2E_DISPOSABLE: undefined })),
    /BLOCKED: this test requires the owned Sprint 2 disposable Docker fixture\./,
  );
});

test('fixture metadata accepts only its unique project, image, compose file, and loopback URL', () => {
  const env = fixtureEnv();
  const fixture = assertDisposableFixture(env);
  assert.equal(fixture.composeProject, env.FUSIONPANEL_E2E_COMPOSE_PROJECT);

  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_E2E_COMPOSE_FILE: '/tmp/other/docker-compose.yml' })),
    /BLOCKED: refusing to operate on an unexpected Compose file\./,
  );
  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_E2E_COMPOSE_PROJECT: 'default' })),
    /BLOCKED: refusing to operate on a non-disposable Compose project\./,
  );
  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_E2E_IMAGE: 'fusionpanel-api:latest' })),
    /BLOCKED: disposable fixture image does not match its Compose project\./,
  );
  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_E2E_PORT: '70000' })),
    /BLOCKED: disposable fixture port is invalid\./,
  );
  assert.throws(
    () => assertDisposableFixture(fixtureEnv({ FUSIONPANEL_FRONTEND_URL: 'https://prod.example.test' })),
    /BLOCKED: the Sprint 2 fixture must use its configured loopback URL\./,
  );
});

test('restart targets only the fixture API and agent with no shell', () => {
  const env = fixtureEnv();
  const fixture = assertDisposableFixture(env);
  let invocation;

  restartDisposableServices(fixture, (...args) => {
    invocation = args;
  });

  assert.deepEqual(invocation, [
    'docker',
    [
      'compose',
      '--project-name',
      env.FUSIONPANEL_E2E_COMPOSE_PROJECT,
      '--file',
      env.FUSIONPANEL_E2E_COMPOSE_FILE,
      'restart',
      'api',
      'agent',
    ],
    { cwd: env.FUSIONPANEL_E2E_REPO_ROOT, env, stdio: 'pipe', timeout: 45_000 },
  ]);
});

test('restart errors are sanitized', () => {
  const fixture = assertDisposableFixture(fixtureEnv());
  assert.throws(
    () => restartDisposableServices(fixture, () => { throw new Error('secret-bearing output'); }),
    (error) => {
      assert.equal(error.message, 'The disposable Sprint 2 API and agent services did not restart successfully.');
      assert.equal(error.message.includes('secret-bearing'), false);
      return true;
    },
  );
});
