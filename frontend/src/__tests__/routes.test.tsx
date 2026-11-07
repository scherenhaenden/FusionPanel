import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import App from '../App';
import { SessionManager } from '../auth/session';

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
