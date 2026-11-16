import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

export function assertDisposableFixture(env = process.env) {
  if (env.FUSIONPANEL_E2E_DISPOSABLE !== '1') {
    throw new Error('BLOCKED: this test requires the owned Sprint 2 disposable Docker fixture.');
  }

  const repoRoot = env.FUSIONPANEL_E2E_REPO_ROOT;
  const composeProject = env.FUSIONPANEL_E2E_COMPOSE_PROJECT;
  const composeFile = env.FUSIONPANEL_E2E_COMPOSE_FILE;
  const port = env.FUSIONPANEL_E2E_PORT;
  const frontendUrl = env.FUSIONPANEL_FRONTEND_URL;

  if (!repoRoot || !composeProject || !composeFile || !port || !frontendUrl) {
    throw new Error('BLOCKED: disposable fixture metadata is incomplete.');
  }

  const parsedPort = Number(port);
  if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65_535) {
    throw new Error('BLOCKED: disposable fixture port is invalid.');
  }

  const expectedComposeFile = resolve(
    repoRoot,
    'tests/e2e/browser/fixtures/sprint2/docker-compose.yml',
  );
  if (resolve(composeFile) !== expectedComposeFile) {
    throw new Error('BLOCKED: refusing to operate on an unexpected Compose file.');
  }
  if (!/^fp-s2-e2e-[A-Za-z0-9_-]+$/.test(composeProject)) {
    throw new Error('BLOCKED: refusing to operate on a non-disposable Compose project.');
  }
  if (env.FUSIONPANEL_E2E_IMAGE !== `fusionpanel-sprint2-e2e:${composeProject}`) {
    throw new Error('BLOCKED: disposable fixture image does not match its Compose project.');
  }
  if (frontendUrl !== `http://127.0.0.1:${parsedPort}`) {
    throw new Error('BLOCKED: the Sprint 2 fixture must use its configured loopback URL.');
  }

  return { repoRoot, composeProject, composeFile, frontendUrl, env };
}

export function restartDisposableServices(fixture, runCommand = execFileSync) {
  try {
    runCommand(
      'docker',
      [
        'compose',
        '--project-name',
        fixture.composeProject,
        '--file',
        fixture.composeFile,
        'restart',
        'api',
        'agent',
      ],
      {
        cwd: fixture.repoRoot,
        env: fixture.env,
        stdio: 'pipe',
        timeout: 45_000,
      },
    );
  } catch {
    throw new Error('The disposable Sprint 2 API and agent services did not restart successfully.');
  }
}
