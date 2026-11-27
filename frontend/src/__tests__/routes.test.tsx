import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import App from '../App';
import { SessionManager } from '../auth/session';
import { apiClient } from '../api/client';

describe('App Routes (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient, { delayResponse: 0 });
    mock.onGet('/v1/auth/me').reply(200, { user_id: 'u1', tenant_id: 't1', roles: ['Admin'] });
    mock.onGet('/v1/containers').reply(200, { containers: [] });
    mock.onGet('/v1/containers/catalog').reply(200, { apps: [] });
    mock.onAny().reply(200, {});
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

  it('renders containers view at /containers when authenticated', async () => {
    SessionManager.markAuthenticated();
    window.history.pushState({}, 'Containers', '/containers');
    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Containers' })).toBeInTheDocument();
    });
    expect(screen.getByRole('link', { name: 'Containers' })).toHaveAttribute('href', '/containers');
  });
});
