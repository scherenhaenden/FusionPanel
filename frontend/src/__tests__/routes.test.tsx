import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import App from '../App';
import { SessionManager } from '../auth/session';
import { apiClient } from '../api/client';

jest.mock('../api', () => ({
  api: {
    getHealth: jest.fn().mockResolvedValue({ status: 'ok', service: 'fusionpanel-api', version: 'test' }),
    getCapabilities: jest.fn().mockResolvedValue({ capabilities: [] }),
    getMe: jest.fn().mockResolvedValue({ user_id: 'test-user', tenant_id: 'test-tenant', roles: ['operator'] }),
    getNodes: jest.fn().mockResolvedValue([]),
    getJobs: jest.fn().mockResolvedValue([]),
    getLatestMetrics: jest.fn().mockResolvedValue([]),
    getNodeMetrics: jest.fn().mockResolvedValue([]),
    getActiveAlerts: jest.fn().mockResolvedValue([]),
    getAlertRules: jest.fn().mockResolvedValue([]),
    getAlertDeliveries: jest.fn().mockResolvedValue([]),
    createAlertRule: jest.fn(),
    deleteAlertRule: jest.fn(),
  },
}));

describe('App Routes (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
    mock.onGet('/v1/auth/me').reply(200, { user_id: 'u1', tenant_id: 't1', roles: ['admin'] });
    mock.onGet('/v1/capabilities').reply(200, { capabilities: [], unavailable: [] });
    mock.onGet('/v1/mail/domains').reply(200, { domains: [] });
    mock.onGet('/v1/nodes').reply(200, { nodes: [] });
    mock.onGet('/v1/dns/zones').reply(200, { zones: [] });
  });

  afterEach(() => {
    mock.restore();
    SessionManager.clear();
    // Clear out window history so each test starts from '/'
    window.history.pushState({}, 'Test', '/');
  });

  it('redirects to login when unauthenticated', async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('Sign in to your account')).toBeInTheDocument();
    });
  });

  it('renders dashboard when authenticated', async () => {
    SessionManager.markAuthenticated();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeInTheDocument();
    });
  });

  it('renders mail route and sidebar link when authenticated', async () => {
    SessionManager.markAuthenticated();
    window.history.pushState({}, 'Mail', '/mail');
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole('link', { name: 'Mail' })).toHaveAttribute('href', '/mail');
      expect(screen.getByText('No mail domains configured yet.')).toBeInTheDocument();
    });
  });
});
