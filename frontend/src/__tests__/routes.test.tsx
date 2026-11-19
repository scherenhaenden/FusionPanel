import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import App from '../App';
import { SessionManager } from '../auth/session';

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
  afterEach(() => {
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
});
