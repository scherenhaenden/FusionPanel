import React from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import ContainersView from '../views/Containers';
import { apiClient } from '../api/client';
import { ContainerCatalogApp, ContainerRecord } from '../api';

const catalogApps: ContainerCatalogApp[] = [
  {
    id: 'nginx-alpine',
    name: 'Nginx Web',
    image: 'nginx:1.27-alpine@sha256:1234',
    description: 'Pinned Nginx reverse proxy image',
    default_port: 8080,
    pinned: true,
  },
  {
    id: 'redis-7',
    name: 'Redis Cache',
    image: 'redis:7.4-alpine@sha256:5678',
    description: 'Pinned in-memory cache',
    default_port: 6379,
    pinned: true,
  },
];

const runningContainer: ContainerRecord = {
  id: 'ctr-1',
  name: 'web-proxy',
  app_id: 'nginx-alpine',
  image: 'nginx:1.27-alpine@sha256:1234',
  state: 'running',
  status_text: 'Up 15 minutes',
  ports: [{ host_port: 8080, container_port: 80, protocol: 'tcp' }],
  stale: false,
  status_updated_at: new Date().toISOString(),
  created_at: '2026-11-27T20:00:00Z',
  updated_at: '2026-11-27T20:15:00Z',
};

const stoppedStaleContainer: ContainerRecord = {
  id: 'ctr-2',
  name: 'cache-node',
  app_id: 'redis-7',
  image: 'redis:7.4-alpine@sha256:5678',
  state: 'stopped',
  status_text: 'Exited (0)',
  ports: ['6379:6379'],
  stale: true,
  status_updated_at: '2026-11-27T10:00:00Z',
  created_at: '2026-11-27T10:00:00Z',
  updated_at: '2026-11-27T10:00:00Z',
};

describe('ContainersView', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient, { delayResponse: 0 });
    mock.onGet('/v1/auth/me').reply(200, {
      user_id: 'user-1',
      tenant_id: 'tenant-1',
      roles: ['Admin'],
    });
    mock.onGet('/v1/containers/catalog').reply(200, { apps: catalogApps });
    mock.onGet('/v1/containers').reply(200, { containers: [runningContainer, stoppedStaleContainer] });
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders containers, pinned images, and stale status badge', async () => {
    render(<ContainersView />);

    expect(await screen.findByText('web-proxy')).toBeInTheDocument();
    expect(screen.getByText('cache-node')).toBeInTheDocument();
    expect(screen.getByText('nginx:1.27-alpine@sha256:1234')).toBeInTheDocument();
    expect(screen.getByText('8080:80/tcp')).toBeInTheDocument();
    expect(screen.getByText('Stale')).toBeInTheDocument();
  });

  it('shows empty state when no containers exist and error state on initial load failure', async () => {
    mock.resetHandlers();
    mock.onGet('/v1/auth/me').reply(200, {
      user_id: 'user-1',
      tenant_id: 'tenant-1',
      roles: ['Admin'],
    });
    mock.onGet('/v1/containers/catalog').reply(200, { apps: catalogApps });
    mock.onGet('/v1/containers').replyOnce(500, {
      error: { message: 'Container runtime unreachable' },
    });

    const { unmount } = render(<ContainersView />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Container runtime unreachable');
    expect(screen.queryByText('No containers deployed yet.')).not.toBeInTheDocument();
    unmount();

    mock.onGet('/v1/containers').replyOnce(200, { containers: [] });
    render(<ContainersView />);
    expect(await screen.findByText('No containers deployed yet.')).toBeInTheDocument();
  });

  it('keeps previously loaded containers and shows stale banner when refresh fails', async () => {
    render(<ContainersView />);
    expect(await screen.findByText('web-proxy')).toBeInTheDocument();

    mock.resetHandlers();
    mock.onGet('/v1/containers').replyOnce(503, {
      error: { message: 'Worker heartbeat timeout' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Worker heartbeat timeout');
    expect(screen.getByText(/status may be stale/i)).toBeInTheDocument();
    expect(screen.getByText('web-proxy')).toBeInTheDocument();
  });

  it('deploys a pinned app from the catalog', async () => {
    mock.onPost('/v1/containers').reply(201, {
      container: {
        ...runningContainer,
        id: 'ctr-3',
        name: 'my-redis',
        app_id: 'redis-7',
        image: 'redis:7.4-alpine@sha256:5678',
      },
    });

    render(<ContainersView />);
    await screen.findByText('web-proxy');

    fireEvent.click(screen.getByRole('button', { name: 'Deploy App' }));
    const dialog = screen.getByRole('dialog');

    fireEvent.change(within(dialog).getByLabelText(/Catalog App/i), {
      target: { value: 'redis-7' },
    });
    fireEvent.change(within(dialog).getByLabelText('Container Name'), {
      target: { value: 'my-redis' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Deploy Container' }));

    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/containers')).toBe(true);
    });

    const deployReq = mock.history.post.find((req) => req.url === '/v1/containers');
    expect(deployReq?.headers?.['Idempotency-Key']).toBeTruthy();
    expect(JSON.parse(deployReq?.data)).toEqual({
      name: 'my-redis',
      app_id: 'redis-7',
      image: 'redis:7.4-alpine@sha256:5678',
      port: 6379,
    });
  });

  it('performs start, stop, and restart lifecycle actions', async () => {
    mock.onPost('/v1/containers/ctr-1/stop').reply(200, { container: { ...runningContainer, state: 'stopped' } });
    mock.onPost('/v1/containers/ctr-1/restart').reply(200, { container: runningContainer });
    mock.onPost('/v1/containers/ctr-2/start').reply(200, { container: { ...stoppedStaleContainer, state: 'running' } });

    render(<ContainersView />);
    await screen.findByText('web-proxy');

    fireEvent.click(screen.getByRole('button', { name: 'Stop web-proxy' }));
    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/containers/ctr-1/stop')).toBe(true);
      expect(screen.getByRole('button', { name: 'Restart web-proxy' })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Restart web-proxy' }));
    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/containers/ctr-1/restart')).toBe(true);
      expect(screen.getByRole('button', { name: 'Start cache-node' })).toBeEnabled();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Start cache-node' }));
    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/containers/ctr-2/start')).toBe(true);
    });
  });

  it('requires confirmation before removing a container', async () => {
    mock.onDelete('/v1/containers/ctr-1').reply(200, {});

    render(<ContainersView />);
    await screen.findByText('web-proxy');

    fireEvent.click(screen.getByRole('button', { name: 'Remove web-proxy' }));
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getByText('Confirm Container Removal')).toBeInTheDocument();
    expect(mock.history.delete).toHaveLength(0);

    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(mock.history.delete).toHaveLength(0);

    fireEvent.click(screen.getByRole('button', { name: 'Remove web-proxy' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm Remove' }));

    await waitFor(() => {
      expect(mock.history.delete).toHaveLength(1);
    });
    expect(mock.history.delete[0].url).toBe('/v1/containers/ctr-1');
  });

  it('disables mutations for read-only role and handles 403 permission denial', async () => {
    mock.onGet('/v1/auth/me').reply(200, {
      user_id: 'viewer-1',
      tenant_id: 'tenant-1',
      roles: ['viewer'],
    });

    const { unmount } = render(<ContainersView />);
    expect(await screen.findByText(/Read-only access \(viewer\)/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Deploy App' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Stop web-proxy' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Restart web-proxy' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Remove web-proxy' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Logs web-proxy' })).toBeEnabled();
    unmount();

    mock.onGet('/v1/auth/me').reply(200, {
      user_id: 'op-1',
      tenant_id: 'tenant-1',
      roles: ['Operator'],
    });
    mock.onPost('/v1/containers/ctr-1/restart').reply(403, {
      error: { code: 'forbidden', message: 'Insufficient container permissions' },
    });

    render(<ContainersView />);
    await screen.findByText('web-proxy');
    fireEvent.click(screen.getByRole('button', { name: 'Restart web-proxy' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Insufficient container permissions');
    expect(screen.getByRole('button', { name: 'Deploy App' })).toBeDisabled();
  });

  it('enforces bounded log limits and handles log fetch errors', async () => {
    const manyLines = Array.from({ length: 80 }, (_, idx) => ({
      sequence: idx + 1,
      stream: 'stdout',
      text: `log-line-${idx + 1}`,
      timestamp: '2026-11-27T21:00:00Z',
    }));

    mock.onGet('/v1/containers/ctr-1/logs').reply((config) => {
      if (config.params?.tail === 50) {
        return [200, { logs: manyLines, truncated: true }];
      }
      return [200, { logs: manyLines.slice(-2), truncated: false }];
    });

    mock.onGet('/v1/containers/ctr-2/logs').reply(500, {
      error: { message: 'Container logs unavailable' },
    });

    render(<ContainersView />);
    await screen.findByText('web-proxy');

    fireEvent.click(screen.getByRole('button', { name: 'Logs web-proxy' }));
    expect(await screen.findByText(/log-line-80/)).toBeInTheDocument();
    expect(mock.history.get.some((r) => r.url === '/v1/containers/ctr-1/logs' && r.params?.tail === 200)).toBe(true);

    fireEvent.change(screen.getByLabelText('Line limit'), { target: { value: '50' } });
    expect(await screen.findByText(/Log output truncated to the latest 50 lines/i)).toBeInTheDocument();
    expect(screen.getByText(/log-line-31/)).toBeInTheDocument();
    expect(screen.queryByText(/log-line-30\b/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Logs cache-node' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Container logs unavailable');
    expect(screen.queryByText(/log-line-80/)).not.toBeInTheDocument();
  });
});
