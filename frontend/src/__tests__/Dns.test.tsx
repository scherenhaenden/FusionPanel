import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import DnsView from '../views/Dns';
import { apiClient } from '../api/client';

describe('DnsView (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders zones and records, syncs DKIM, and triggers propagation probe', async () => {
    const zone = {
      id: 'zone-1',
      domain: 'example.com',
      serial: 1,
      default_ttl: 300,
      state: 'ready',
      last_error: null,
      last_probe_result: null,
      created_at: '2026-11-17T14:00:00Z',
      updated_at: '2026-11-17T14:00:00Z',
    };

    mock.onGet('/v1/dns/zones').reply(200, { zones: [zone] });
    mock.onGet('/v1/dns/zones/zone-1').reply(200, {
      zone,
      records: [
        {
          id: 'rec-1',
          zone_id: 'zone-1',
          name: 'www',
          record_type: 'A',
          value: '203.0.113.10',
          ttl: 300,
          priority: null,
          created_at: '2026-11-17T14:00:00Z',
          updated_at: '2026-11-17T14:00:00Z',
        },
      ],
    });
    mock.onPost('/v1/dns/zones/zone-1/dkim').reply(202, {
      zone: { ...zone, serial: 2 },
      records: [
        {
          id: 'rec-2',
          zone_id: 'zone-1',
          name: 'default._domainkey',
          record_type: 'TXT',
          value: 'v=DKIM1; k=rsa; p=MIIBIjAN',
          ttl: 300,
          priority: null,
          created_at: '2026-11-17T14:01:00Z',
          updated_at: '2026-11-17T14:01:00Z',
        },
      ],
      job: {
        id: 'job-dkim',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });
    mock.onPost('/v1/dns/zones/zone-1/probe').reply(202, {
      zone: { ...zone, serial: 2 },
      job: {
        id: 'job-probe',
        kind: 'agent_operation',
        state: 'queued',
        payload: {},
        worker_id: null,
        error_text: null,
        created_at: '',
        updated_at: '',
      },
    });

    render(<DnsView />);

    await waitFor(() => {
      expect(screen.getByText('203.0.113.10')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('DKIM TXT Record'), {
      target: { value: 'v=DKIM1; k=rsa; p=MIIBIjAN' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Sync DKIM' }));

    await waitFor(() => {
      expect(screen.getByText('default._domainkey')).toBeInTheDocument();
    });

    expect(mock.history.post[0].url).toBe('/v1/dns/zones/zone-1/dkim');
    expect(mock.history.post[0].headers?.['Idempotency-Key']).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Probe Propagation' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(2);
    });
    expect(mock.history.post[1].url).toBe('/v1/dns/zones/zone-1/probe');
    expect(mock.history.post[1].headers?.['Idempotency-Key']).toBeTruthy();
  });
});
