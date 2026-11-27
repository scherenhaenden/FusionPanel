import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import MailView from '../views/Mail';
import { apiClient } from '../api/client';

describe('MailView (Unit Evidence)', () => {
  let mock: MockAdapter;

  const baseDomain = {
    id: 'dom-1',
    tenant_id: 'ten-1',
    node_id: 'node-1',
    domain: 'example.com',
    dns_zone_id: 'zone-1',
    active: true,
    state: 'ready' as const,
    desired_revision: 2,
    queued_revision: null,
    applied_revision: 2,
    failed_revision: null,
    pending_job_id: null,
    last_error: null,
    created_at: '2026-11-27T10:00:00Z',
    updated_at: '2026-11-27T10:05:00Z',
  };

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
    mock.onGet('/v1/capabilities').reply(200, {
      capabilities: ['control-plane.health.read', 'control-plane.capabilities.read'],
      unavailable: [],
    });
    mock.onGet('/v1/nodes').reply(200, {
      nodes: [
        {
          node_id: 'node-1',
          name: 'mail-node-1',
          state: 'online',
          created_at: '2026-11-27T10:00:00Z',
          last_heartbeat: '2026-11-27T10:05:00Z',
        },
      ],
    });
    mock.onGet('/v1/dns/zones').reply(200, {
      zones: [
        {
          id: 'zone-1',
          domain: 'example.com',
          serial: 3,
          default_ttl: 300,
          state: 'ready',
          last_error: null,
          last_probe_result: null,
          created_at: '2026-11-27T10:00:00Z',
          updated_at: '2026-11-27T10:05:00Z',
        },
      ],
    });
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders denied state when operator lacks MailManage permission (403)', async () => {
    mock.onGet('/v1/mail/domains').reply(403, {
      error: { code: 'forbidden', message: 'Missing MailManage permission' },
    });

    render(<MailView />);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Access Denied');
    });
    expect(screen.getByText('Missing MailManage permission')).toBeInTheDocument();
  });

  it('renders denied state when mail capability is unavailable', async () => {
    mock.onGet('/v1/capabilities').reply(200, {
      capabilities: [],
      unavailable: ['mail'],
    });
    mock.onGet('/v1/mail/domains').reply(200, { domains: [] });

    render(<MailView />);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Access Denied');
    });
    expect(
      screen.getByText('Mail management capability is unavailable for this workspace.'),
    ).toBeInTheDocument();
  });

  it('renders empty state and validates required domain form fields', async () => {
    mock.onGet('/v1/mail/domains').reply(200, { domains: [] });

    render(<MailView />);

    expect(screen.getByText('Loading mail domains...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText('No mail domains configured yet.')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Create Mail Domain' }));
    fireEvent.click(screen.getByRole('button', { name: 'Save Domain' }));

    expect(
      screen.getByText('Enter a valid canonical domain name (e.g. example.com).'),
    ).toBeInTheDocument();
    expect(mock.history.post.length).toBe(0);
  });

  it('handles mailbox creation, clears credentials immediately, alias creation, DKIM publication, and confirmed removal', async () => {
    mock.onGet('/v1/mail/domains').reply(200, { domains: [baseDomain] });
    mock.onGet('/v1/mail/domains/dom-1').reply(200, {
      domain: baseDomain,
      mailboxes: [],
      aliases: [],
      dkim: {
        domain_id: 'dom-1',
        tenant_id: 'ten-1',
        selector: 'default',
        enabled: true,
        dns_zone_id: 'zone-1',
        txt_record: 'v=DKIM1; k=rsa; p=MIIBIjAN',
        public_key_pem: null,
        state: 'ready',
        queued_revision: null,
        applied_revision: 2,
        failed_revision: null,
        pending_job_id: null,
        last_error: null,
        created_at: '2026-11-27T10:00:00Z',
        updated_at: '2026-11-27T10:05:00Z',
      },
    });
    mock.onGet('/v1/dns/zones/zone-1').reply(200, {
      zone: {
        id: 'zone-1',
        domain: 'example.com',
        serial: 3,
        default_ttl: 300,
        state: 'ready',
        last_error: null,
        last_probe_result: null,
        created_at: '2026-11-27T10:00:00Z',
        updated_at: '2026-11-27T10:05:00Z',
      },
      records: [
        {
          id: 'rec-dkim',
          zone_id: 'zone-1',
          name: 'default._domainkey',
          record_type: 'TXT',
          value: 'v=DKIM1; k=rsa; p=MIIBIjAN',
          ttl: 300,
          priority: null,
          created_at: '2026-11-27T10:00:00Z',
          updated_at: '2026-11-27T10:05:00Z',
        },
      ],
    });

    const createdMailbox = {
      id: 'mb-1',
      domain_id: 'dom-1',
      tenant_id: 'ten-1',
      address: 'alice@example.com',
      local_part: 'alice',
      quota_bytes: 1024 * 1024 * 1024,
      active: true,
      created_at: '2026-11-27T10:06:00Z',
      updated_at: '2026-11-27T10:06:00Z',
    };

    mock.onPost('/v1/mail/domains/dom-1/mailboxes').reply(202, {
      mailbox: createdMailbox,
      domain: {
        ...baseDomain,
        state: 'provisioning',
        desired_revision: 3,
        queued_revision: 3,
        pending_job_id: 'job-mb-1',
      },
      job: {
        id: 'job-mb-1',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    mock.onPost('/v1/mail/domains/dom-1/aliases').reply(202, {
      alias: {
        id: 'al-1',
        domain_id: 'dom-1',
        tenant_id: 'ten-1',
        source: 'support@example.com',
        destinations: ['alice@example.com'],
        active: true,
        created_at: '2026-11-27T10:07:00Z',
        updated_at: '2026-11-27T10:07:00Z',
      },
      domain: baseDomain,
      job: {
        id: 'job-al-1',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    mock.onDelete('/v1/mail/domains/dom-1').reply(202, {
      domain: { ...baseDomain, state: 'deleting', pending_job_id: 'job-del-1' },
      job: {
        id: 'job-del-1',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    render(<MailView />);

    await waitFor(() => {
      expect(screen.getByText('v=DKIM1; k=rsa; p=MIIBIjAN')).toBeInTheDocument();
    });
    expect(screen.getByText('Published in DNS zone')).toBeInTheDocument();

    // Validate short password
    fireEvent.change(screen.getByLabelText('Mailbox Local Part'), { target: { value: 'alice' } });
    fireEvent.change(screen.getByLabelText('Mailbox Password'), { target: { value: 'short' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Mailbox' }));
    expect(
      screen.getByText('Password must be at least 8 characters without control characters.'),
    ).toBeInTheDocument();
    expect(mock.history.post.length).toBe(0);

    // Valid mailbox submission
    const secretPw = 'Sup3rS3cretMail!';
    fireEvent.change(screen.getByLabelText('Mailbox Password'), { target: { value: secretPw } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Mailbox' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(1);
    });
    expect(mock.history.post[0].url).toBe('/v1/mail/domains/dom-1/mailboxes');
    expect(mock.history.post[0].headers?.['Idempotency-Key']).toBeTruthy();

    // Password input is cleared immediately after submission succeeds and secret is not in DOM
    await waitFor(() => {
      expect((screen.getByLabelText('Mailbox Password') as HTMLInputElement).value).toBe('');
    });
    expect(document.body.textContent).not.toContain(secretPw);

    // Alias submission
    fireEvent.change(screen.getByLabelText('Alias Local Part'), { target: { value: 'support' } });
    fireEvent.change(screen.getByLabelText('Destinations (comma-separated)'), {
      target: { value: 'alice@example.com' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add Alias' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(2);
    });
    expect(mock.history.post[1].url).toBe('/v1/mail/domains/dom-1/aliases');

    // Confirm removal before deleting domain
    fireEvent.click(screen.getByRole('button', { name: 'Delete Domain' }));
    expect(screen.getByRole('dialog')).toHaveTextContent('Confirm Removal');
    expect(mock.history.delete.length).toBe(0);

    fireEvent.click(screen.getByRole('button', { name: 'Confirm Delete' }));
    await waitFor(() => {
      expect(mock.history.delete.length).toBe(1);
    });
    expect(mock.history.delete[0].url).toBe('/v1/mail/domains/dom-1');
  });

  it('displays failed agent apply revisions/errors and preserves Idempotency-Key on retry', async () => {
    const failedDomain = {
      ...baseDomain,
      state: 'failed' as const,
      desired_revision: 3,
      queued_revision: null,
      applied_revision: 2,
      failed_revision: 3,
      pending_job_id: null,
      last_error: 'Mail domain configuration could not be applied',
    };

    mock.onGet('/v1/mail/domains').reply(200, { domains: [failedDomain] });
    mock.onGet('/v1/mail/domains/dom-1').reply(200, {
      domain: failedDomain,
      mailboxes: [],
      aliases: [],
      dkim: null,
    });
    mock.onGet('/v1/dns/zones/zone-1').reply(200, {
      zone: {
        id: 'zone-1',
        domain: 'example.com',
        serial: 3,
        default_ttl: 300,
        state: 'ready',
        last_error: null,
        last_probe_result: null,
        created_at: '2026-11-27T10:00:00Z',
        updated_at: '2026-11-27T10:05:00Z',
      },
      records: [],
    });

    // First DKIM submit fails with 500, second succeeds with 202
    mock
      .onPost('/v1/mail/domains/dom-1/dkim')
      .replyOnce(500, { error: { code: 'internal_error', message: 'Transient dispatch error' } })
      .onPost('/v1/mail/domains/dom-1/dkim')
      .replyOnce(202, {
        dkim: {
          domain_id: 'dom-1',
          tenant_id: 'ten-1',
          selector: 's1',
          enabled: true,
          dns_zone_id: 'zone-1',
          txt_record: null,
          public_key_pem: null,
          state: 'provisioning',
          queued_revision: 4,
          applied_revision: null,
          failed_revision: null,
          pending_job_id: 'job-dkim-4',
          last_error: null,
          created_at: '2026-11-27T10:10:00Z',
          updated_at: '2026-11-27T10:10:00Z',
        },
        domain: {
          ...failedDomain,
          state: 'provisioning',
          desired_revision: 4,
          queued_revision: 4,
          pending_job_id: 'job-dkim-4',
        },
        job: {
          id: 'job-dkim-4',
          kind: 'agent_operation',
          state: 'queued',
          payload: {},
          worker_id: null,
          error_text: null,
          created_at: '',
          updated_at: '',
        },
      });

    render(<MailView />);

    await waitFor(() => {
      expect(
        screen.getByText('Mail domain configuration could not be applied'),
      ).toBeInTheDocument();
    });
    expect(screen.getByText('Failed Rev: 3')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('DKIM Selector'), { target: { value: 's1' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save DKIM' }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Transient dispatch error');
    });

    // Retry the failed submission
    fireEvent.click(screen.getByRole('button', { name: 'Save DKIM' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(2);
    });

    const firstKey = mock.history.post[0].headers?.['Idempotency-Key'];
    const secondKey = mock.history.post[1].headers?.['Idempotency-Key'];
    expect(firstKey).toBeTruthy();
    expect(secondKey).toBe(firstKey);
  });
});
