import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import { BackupsView } from '../views/Backups';
import { apiClient } from '../api/client';

describe('BackupsView (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders backups with resolved site/database targets and SHA-256 manifest', async () => {
    mock.onGet('/v1/backups').reply(200, {
      backups: [
        {
          id: 'bkp-1',
          archive_key: 'backup-2026-001',
          target: { kind: 'bundle', site_id: 'site-1', database_id: 'db-1' },
          destination_kind: 's3',
          destination_summary: 's3://fusionpanel-backups/prod',
          state: 'ready',
          manifest_sha256: 'abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789',
          last_error: null,
          created_at: '2026-11-17T10:00:00Z',
          updated_at: '2026-11-17T10:01:00Z',
        },
      ],
    });
    mock.onGet('/v1/websites').reply(200, {
      websites: [
        {
          id: 'site-1',
          domain: 'example.com',
          kind: 'static',
          document_root: '/srv/fusionpanel/sites/example.com',
          state: 'active',
          last_error: null,
          created_at: '2026-11-17T09:00:00Z',
          updated_at: '2026-11-17T09:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/databases').reply(200, {
      databases: [{ id: 'db-1', name: 'app_db', engine: 'postgresql', state: 'ready' }],
    });

    render(<BackupsView />);

    await waitFor(() => {
      expect(screen.getByText('backup-2026-001')).toBeInTheDocument();
    });

    expect(screen.getByText('Bundle: example.com + app_db')).toBeInTheDocument();
    expect(screen.getByText('s3://fusionpanel-backups/prod')).toBeInTheDocument();
    expect(screen.getByText('ready')).toBeInTheDocument();
    expect(screen.getByText('abcdef012345...')).toBeInTheDocument();
  });

  it('creates a local bundle backup with Idempotency-Key header', async () => {
    mock.onGet('/v1/backups').reply(200, { backups: [] });
    mock.onGet('/v1/websites').reply(200, {
      websites: [
        {
          id: 'site-1',
          domain: 'alpha.local',
          kind: 'static',
          document_root: '/srv/fusionpanel/sites/alpha.local',
          state: 'active',
          last_error: null,
          created_at: '2026-11-17T09:00:00Z',
          updated_at: '2026-11-17T09:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/databases').reply(200, {
      databases: [{ id: 'db-1', name: 'alpha_db', engine: 'postgresql', state: 'ready' }],
    });
    mock.onPost('/v1/backups').reply(202, {
      backup: {
        id: 'bkp-2',
        archive_key: 'backup-2026-002',
        target: { kind: 'bundle', site_id: 'site-1', database_id: 'db-1' },
        destination_kind: 'local',
        destination_summary: '/var/backups/fusionpanel',
        state: 'creating',
        manifest_sha256: null,
        last_error: null,
        created_at: '2026-11-17T10:00:00Z',
        updated_at: '2026-11-17T10:00:00Z',
      },
      job: {
        id: 'job-2',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    render(<BackupsView />);
    await waitFor(() => expect(screen.getByText('No backups found.')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: 'Backup Now' }));
    fireEvent.change(screen.getByLabelText('Target Scope'), { target: { value: 'bundle' } });
    fireEvent.click(screen.getByRole('button', { name: 'Start Backup' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(1);
    });

    expect(mock.history.post[0].url).toBe('/v1/backups');
    expect(mock.history.post[0].headers?.['Idempotency-Key']).toBeTruthy();
    expect(JSON.parse(mock.history.post[0].data)).toEqual({
      target: { kind: 'bundle', site_id: 'site-1', database_id: 'db-1' },
      destination: { kind: 'local', base_dir: '/var/backups/fusionpanel' },
    });
  });

  it('restores an S3 backup via RestoreWizard with RESTORE confirmation and Idempotency-Key', async () => {
    mock.onGet('/v1/backups').reply(200, {
      backups: [
        {
          id: 'bkp-s3',
          archive_key: 'backup-s3-001',
          target: { kind: 'site', site_id: 'site-1' },
          destination_kind: 's3',
          destination_summary: 's3://fusionpanel-backups/sprint6',
          state: 'ready',
          manifest_sha256: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          last_error: null,
          created_at: '2026-11-17T10:00:00Z',
          updated_at: '2026-11-17T10:01:00Z',
        },
      ],
    });
    mock.onGet('/v1/websites').reply(200, {
      websites: [
        {
          id: 'site-1',
          domain: 'restore.local',
          kind: 'static',
          document_root: '/srv/fusionpanel/sites/restore.local',
          state: 'active',
          last_error: null,
          created_at: '2026-11-17T09:00:00Z',
          updated_at: '2026-11-17T09:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/databases').reply(200, { databases: [] });
    mock.onPost('/v1/backups/bkp-s3/restore').reply(202, {
      backup: {
        id: 'bkp-s3',
        archive_key: 'backup-s3-001',
        target: { kind: 'site', site_id: 'site-1' },
        destination_kind: 's3',
        destination_summary: 's3://fusionpanel-backups/sprint6',
        state: 'restoring',
        manifest_sha256: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        last_error: null,
        created_at: '2026-11-17T10:00:00Z',
        updated_at: '2026-11-17T10:02:00Z',
      },
      job: {
        id: 'job-restore-1',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    render(<BackupsView />);
    await waitFor(() => expect(screen.getByText('backup-s3-001')).toBeInTheDocument());

    fireEvent.click(screen.getByRole('button', { name: 'Restore' }));
    expect(screen.getByText('Restore Backup Archive')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('S3 Endpoint'), {
      target: { value: 'http://minio:9000' },
    });
    fireEvent.change(screen.getByLabelText('Access Key'), {
      target: { value: 'minioadmin' },
    });
    fireEvent.change(screen.getByLabelText('Secret Key'), {
      target: { value: 'minioadmin' },
    });
    fireEvent.change(screen.getByPlaceholderText('RESTORE'), {
      target: { value: 'RESTORE' },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Confirm Restore' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(1);
    });

    expect(mock.history.post[0].url).toBe('/v1/backups/bkp-s3/restore');
    expect(mock.history.post[0].headers?.['Idempotency-Key']).toBeTruthy();
    expect(JSON.parse(mock.history.post[0].data)).toEqual({
      destination: {
        kind: 's3',
        endpoint: 'http://minio:9000',
        bucket: 'fusionpanel-backups',
        region: 'us-east-1',
        access_key: 'minioadmin',
        secret_key: 'minioadmin',
        prefix: 'sprint6',
      },
    });
  });
});
