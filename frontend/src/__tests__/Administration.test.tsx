import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import AdministrationView from '../views/Administration';
import SecurityRulesUI from '../views/Security';
import { apiClient } from '../api/client';

describe('AdministrationView & SecurityRulesUI', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
    mock.onGet('/v1/admin/users').reply(200, {
      users: [
        {
          id: 'user-1',
          username: 'admin',
          email: 'admin@example.com',
          status: 'active',
          roles: ['SuperAdmin'],
          created_at: '2026-11-17T16:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/admin/tokens').reply(200, {
      tokens: [
        {
          id: 'tok-1',
          tenant_id: 'tenant-1',
          user_id: 'user-1',
          name: 'ci-reader',
          scopes: ['sites:read'],
          created_at: '2026-11-17T16:00:00Z',
          revoked_at: null,
        },
      ],
    });
    mock.onGet('/v1/audit').reply(200, {
      audit_logs: [
        {
          id: 'aud-1',
          tenant_id: 'tenant-1',
          actor_id: 'user-1',
          action: 'user.create',
          resource_type: 'user',
          resource_id: 'user-1',
          outcome: 'success',
          detail: 'admin (SuperAdmin)',
          created_at: '2026-11-17T16:00:00Z',
        },
      ],
    });
    mock.onGet('/v1/security/rules').reply(200, { rules: [] });
    mock.onGet('/v1/auth/mfa').reply(200, { enabled: false });
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders Operators, API Tokens, and Audit Logs tabs with live data', async () => {
    render(<AdministrationView />);

    await waitFor(() => {
      expect(screen.getByText('admin@example.com')).toBeInTheDocument();
    });
    expect(screen.getByText('Operators')).toBeInTheDocument();
    expect(screen.getByText('API Tokens')).toBeInTheDocument();
    expect(screen.getByText('Audit Logs')).toBeInTheDocument();

    fireEvent.click(screen.getByText('API Tokens'));
    expect(screen.getByText('ci-reader')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Audit Logs'));
    expect(screen.getByText('user.create')).toBeInTheDocument();
    expect(screen.queryByText('Invite Operator')).not.toBeInTheDocument();
  });

  it('opens and closes the Invite Operator modal and creates API token', async () => {
    mock.onPost('/v1/admin/tokens').reply(201, {
      token: 'fp_tok_secret123',
      record: {
        id: 'tok-2',
        tenant_id: 'tenant-1',
        user_id: 'user-1',
        name: 'deploy-key',
        scopes: ['sites:read'],
        created_at: '2026-11-17T16:00:00Z',
        revoked_at: null,
      },
    });

    render(<AdministrationView />);

    await waitFor(() => {
      expect(screen.getByText('Invite Operator')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Invite Operator'));
    expect(screen.getByText('Email Address')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send Invite' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    fireEvent.click(screen.getByText('API Tokens'));
    fireEvent.change(screen.getByLabelText('Token Name'), { target: { value: 'deploy-key' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create API Token' }));

    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('fp_tok_secret123');
    });
  });

  it('enrolls and verifies TOTP MFA in SecurityRulesUI', async () => {
    mock.onPost('/v1/auth/mfa/enroll').reply(200, {
      secret: 'JBSWY3DPEHPK3PXP',
      otpauth_url: 'otpauth://totp/FusionPanel:user-1?secret=JBSWY3DPEHPK3PXP',
      enabled: false,
    });
    mock.onPost('/v1/auth/mfa/verify').reply(200, { enabled: true });

    render(<SecurityRulesUI />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Enable MFA' })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Enable MFA' }));
    await waitFor(() => {
      expect(screen.getByText('JBSWY3DPEHPK3PXP')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('TOTP Code'), { target: { value: '123456' } });
    fireEvent.click(screen.getByRole('button', { name: 'Verify & Enable' }));

    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('Two-factor authentication is now enabled.');
    });
  });
});
