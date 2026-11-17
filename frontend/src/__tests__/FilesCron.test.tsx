import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import FilesView from '../views/Files';
import CronView from '../views/Cron';
import { apiClient } from '../api/client';

describe('FilesView and CronView (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('lists, reads, writes files and surfaces path traversal errors', async () => {
    mock.onGet('/v1/websites').reply(200, {
      websites: [
        {
          id: 'site-1',
          domain: 'files.example.com',
          kind: 'static',
          document_root: '/srv/fusionpanel/sites/files.example.com',
          state: 'active',
          last_error: null,
          created_at: '2026-11-17T15:00:00Z',
          updated_at: '2026-11-17T15:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/files/site-1/list').reply(200, {
      entries: [{ name: 'index.html', path: 'index.html', kind: 'file', size: 12 }],
    });
    mock.onGet('/v1/files/site-1/read').reply(200, {
      path: 'index.html',
      content: '<h1>hello</h1>',
    });
    mock.onPut('/v1/files/site-1/write').reply((config) => {
      const body = JSON.parse(config.data as string);
      if (body.path.includes('..')) {
        return [400, { error: 'invalid_path', message: 'path traversal is not allowed' }];
      }
      return [200, { path: body.path, size: body.content.length }];
    });

    render(<FilesView />);

    await waitFor(() => {
      expect(screen.getByText('index.html')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('index.html'));
    await waitFor(() => {
      expect(screen.getByLabelText('File Content')).toHaveValue('<h1>hello</h1>');
    });

    fireEvent.change(screen.getByLabelText('File Path'), {
      target: { value: '../escape.txt' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save File' }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('path traversal is not allowed');
    });
  });

  it('creates, runs now, and deletes cron jobs', async () => {
    const jobs = [
      {
        id: 'cron-1',
        tenant_id: 'tenant-1',
        name: 'heartbeat',
        schedule: '* * * * *',
        command: 'echo ok',
        enabled: true,
        state: 'active',
        last_run_at: null,
        last_run_status: null,
        last_error: null,
        created_at: '2026-11-17T15:00:00Z',
        updated_at: '2026-11-17T15:00:00Z',
      },
    ];

    mock.onGet('/v1/cron').reply(() => [200, { cron_jobs: jobs }]);
    mock.onPost('/v1/cron').reply(202, {
      cron_job: {
        ...jobs[0],
        id: 'cron-2',
        name: 'nightly',
      },
      job: { id: 'job-1', status: 'queued' },
    });
    mock.onPost('/v1/cron/cron-1/run').reply(202, {
      cron_job: jobs[0],
      job: { id: 'job-2', status: 'queued' },
    });
    mock.onDelete('/v1/cron/cron-1').reply(202, {
      cron_job: { ...jobs[0], state: 'deleting' },
      job: { id: 'job-3', status: 'queued' },
    });

    render(<CronView />);

    await waitFor(() => {
      expect(screen.getByText('heartbeat')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /New Cron Job/i }));
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'nightly' } });
    fireEvent.change(screen.getByLabelText('Command'), { target: { value: 'echo nightly' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create Cron Job' }));

    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/cron')).toBe(true);
      expect(screen.queryByRole('button', { name: 'Create Cron Job' })).not.toBeInTheDocument();
      expect(screen.getByRole('button', { name: /Run Now/i })).not.toBeDisabled();
    });

    fireEvent.click(screen.getByRole('button', { name: /Run Now/i }));
    await waitFor(() => {
      expect(mock.history.post.some((req) => req.url === '/v1/cron/cron-1/run')).toBe(true);
      expect(screen.getByRole('button', { name: /Delete/i })).not.toBeDisabled();
    });

    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));
    await waitFor(() => {
      expect(mock.history.delete.some((req) => req.url === '/v1/cron/cron-1')).toBe(true);
    });
  });
});
