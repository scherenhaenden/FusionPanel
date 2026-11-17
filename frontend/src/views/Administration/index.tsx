import React, { useCallback, useEffect, useState } from 'react';
import { UserPlus, Shield, Activity, Users, KeyRound, Trash2 } from 'lucide-react';
import { AdminUserRecord, ApiTokenRecord, AuditLogRecord, api } from '../../api';

function extractErrorMessage(err: unknown, fallback: string): string {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const resp = (err as { response?: { data?: { error?: unknown; message?: unknown } } }).response;
    const data = resp?.data;
    if (typeof data?.message === 'string' && data.message) return data.message;
    if (typeof data?.error === 'string' && data.error) return data.error;
    if (
      typeof data?.error === 'object' &&
      data.error !== null &&
      'message' in data.error &&
      typeof (data.error as { message?: unknown }).message === 'string'
    ) {
      return (data.error as { message: string }).message;
    }
  }
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

export default function AdministrationView() {
  const [activeTab, setActiveTab] = useState<'operators' | 'tokens' | 'audit'>('operators');
  const [users, setUsers] = useState<AdminUserRecord[]>([]);
  const [tokens, setTokens] = useState<ApiTokenRecord[]>([]);
  const [auditLogs, setAuditLogs] = useState<AuditLogRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [showInviteModal, setShowInviteModal] = useState(false);
  const [inviteUsername, setInviteUsername] = useState('');
  const [inviteEmail, setInviteEmail] = useState('');
  const [invitePassword, setInvitePassword] = useState('');
  const [inviteRole, setInviteRole] = useState('viewer');

  const [tokenName, setTokenName] = useState('');
  const [tokenScopes, setTokenScopes] = useState('sites:read');
  const [createdTokenSecret, setCreatedTokenSecret] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [userList, tokenList, auditList] = await Promise.all([
        api.getAdminUsers(),
        api.getApiTokens(),
        api.getAuditLogs(),
      ]);
      setUsers(userList);
      setTokens(tokenList);
      setAuditLogs(auditList);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to load administration data'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleInvite = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await api.createAdminUser({
        username: inviteUsername.trim(),
        email: inviteEmail.trim(),
        password: invitePassword,
        role: inviteRole,
      });
      setShowInviteModal(false);
      setInviteUsername('');
      setInviteEmail('');
      setInvitePassword('');
      setInviteRole('viewer');
      await loadData();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to create operator'));
    } finally {
      setBusy(false);
    }
  };

  const handleDeleteUser = async (userId: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.deleteAdminUser(userId);
      await loadData();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to delete operator'));
    } finally {
      setBusy(false);
    }
  };

  const handleCreateToken = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!tokenName.trim()) return;
    setBusy(true);
    setError(null);
    setCreatedTokenSecret(null);
    try {
      const scopes = tokenScopes
        .split(',')
        .map((s) => s.trim())
        .filter(Boolean);
      const res = await api.createApiToken(tokenName.trim(), scopes.length ? scopes : ['sites:read']);
      setCreatedTokenSecret(res.token);
      setTokenName('');
      await loadData();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to create API token'));
    } finally {
      setBusy(false);
    }
  };

  const handleRevokeToken = async (tokenId: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.revokeApiToken(tokenId);
      await loadData();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to revoke API token'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
          <Shield className="w-6 h-6" /> Administration
        </h1>
        {activeTab === 'operators' && (
          <button
            type="button"
            onClick={() => setShowInviteModal(true)}
            className="flex items-center gap-2 bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg transition-colors"
          >
            <UserPlus className="w-4 h-4" /> Invite Operator
          </button>
        )}
      </div>

      {error && (
        <div
          role="alert"
          className="rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-900/30 dark:text-red-200"
        >
          {error}
        </div>
      )}

      <div className="border-b border-gray-200 dark:border-gray-700">
        <nav className="flex space-x-4">
          <button
            type="button"
            onClick={() => setActiveTab('operators')}
            className={`py-2 px-4 border-b-2 font-medium text-sm transition-colors ${
              activeTab === 'operators'
                ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <Users className="w-4 h-4" /> Operators
            </div>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('tokens')}
            className={`py-2 px-4 border-b-2 font-medium text-sm transition-colors ${
              activeTab === 'tokens'
                ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <KeyRound className="w-4 h-4" /> API Tokens
            </div>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('audit')}
            className={`py-2 px-4 border-b-2 font-medium text-sm transition-colors ${
              activeTab === 'audit'
                ? 'border-blue-500 text-blue-600 dark:text-blue-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <Activity className="w-4 h-4" /> Audit Logs
            </div>
          </button>
        </nav>
      </div>

      {loading ? (
        <p className="text-sm text-gray-500 dark:text-gray-400">Loading administration data...</p>
      ) : activeTab === 'operators' ? (
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
          <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead className="bg-gray-50 dark:bg-gray-700">
              <tr>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Username</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Email</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Roles</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Status</th>
                <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Actions</th>
              </tr>
            </thead>
            <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {users.map((op) => (
                <tr key={op.id}>
                  <td className="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900 dark:text-white">{op.username}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-600 dark:text-gray-300">{op.email}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500 dark:text-gray-300">{op.roles.join(', ')}</td>
                  <td className="px-6 py-4 whitespace-nowrap">
                    <span className="px-2 inline-flex text-xs leading-5 font-semibold rounded-full bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200">
                      {op.status}
                    </span>
                  </td>
                  <td className="px-6 py-4 whitespace-nowrap text-right">
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={`Delete ${op.username}`}
                      onClick={() => void handleDeleteUser(op.id)}
                      className="inline-flex items-center gap-1 rounded border border-red-300 px-2.5 py-1 text-xs text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-400"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : activeTab === 'tokens' ? (
        <div className="space-y-4">
          <form
            onSubmit={handleCreateToken}
            className="bg-white dark:bg-gray-800 shadow rounded-lg p-4 border border-gray-200 dark:border-gray-700 grid grid-cols-1 md:grid-cols-3 gap-4 items-end"
          >
            <div>
              <label htmlFor="token-name" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Token Name
              </label>
              <input
                id="token-name"
                type="text"
                required
                value={tokenName}
                onChange={(e) => setTokenName(e.target.value)}
                placeholder="ci-readonly"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label htmlFor="token-scopes" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                Scopes (comma-separated)
              </label>
              <input
                id="token-scopes"
                type="text"
                required
                value={tokenScopes}
                onChange={(e) => setTokenScopes(e.target.value)}
                placeholder="sites:read"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <button
              type="submit"
              disabled={busy}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              Create API Token
            </button>
          </form>

          {createdTokenSecret && (
            <div
              role="status"
              className="rounded-lg border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-900 dark:border-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-100"
            >
              Copy your new API token now (shown once):{' '}
              <code className="font-mono font-semibold">{createdTokenSecret}</code>
            </div>
          )}

          <div className="bg-white dark:bg-gray-800 shadow rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
            <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
              <thead className="bg-gray-50 dark:bg-gray-700">
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Name</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Scopes</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Status</th>
                  <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Actions</th>
                </tr>
              </thead>
              <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
                {tokens.map((tok) => (
                  <tr key={tok.id}>
                    <td className="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900 dark:text-white">{tok.name}</td>
                    <td className="px-6 py-4 whitespace-nowrap font-mono text-xs text-gray-600 dark:text-gray-300">{tok.scopes.join(', ')}</td>
                    <td className="px-6 py-4 whitespace-nowrap text-xs">
                      {tok.revoked_at ? (
                        <span className="text-red-600 dark:text-red-400">revoked</span>
                      ) : (
                        <span className="text-emerald-600 dark:text-emerald-400">active</span>
                      )}
                    </td>
                    <td className="px-6 py-4 whitespace-nowrap text-right">
                      {!tok.revoked_at && (
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => void handleRevokeToken(tok.id)}
                          className="rounded border border-red-300 px-2.5 py-1 text-xs text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-400"
                        >
                          Revoke
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ) : (
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
          <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
            <thead className="bg-gray-50 dark:bg-gray-700">
              <tr>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Timestamp</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Actor</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Action</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Resource</th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 dark:text-gray-300 uppercase tracking-wider">Details</th>
              </tr>
            </thead>
            <tbody className="bg-white dark:bg-gray-800 divide-y divide-gray-200 dark:divide-gray-700">
              {auditLogs.map((log) => (
                <tr key={log.id}>
                  <td className="px-6 py-4 whitespace-nowrap text-xs text-gray-500 dark:text-gray-400">{log.created_at}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-900 dark:text-white">{log.actor_id}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-sm font-medium text-blue-600 dark:text-blue-400">{log.action}</td>
                  <td className="px-6 py-4 whitespace-nowrap text-xs font-mono text-gray-500 dark:text-gray-300">{log.resource_type}</td>
                  <td className="px-6 py-4 text-sm text-gray-500 dark:text-gray-300">{log.detail || log.outcome}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {showInviteModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center p-4 z-50">
          <div className="bg-white dark:bg-gray-800 rounded-lg max-w-md w-full p-6 space-y-4">
            <h2 className="text-xl font-bold text-gray-900 dark:text-white">Invite Operator</h2>
            <form onSubmit={handleInvite} className="space-y-4">
              <div>
                <label htmlFor="invite-username" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Username
                </label>
                <input
                  id="invite-username"
                  type="text"
                  required
                  value={inviteUsername}
                  onChange={(e) => setInviteUsername(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm dark:bg-gray-700 dark:text-white"
                  placeholder="operator1"
                />
              </div>
              <div>
                <label htmlFor="invite-email" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Email Address
                </label>
                <input
                  id="invite-email"
                  type="email"
                  required
                  value={inviteEmail}
                  onChange={(e) => setInviteEmail(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm dark:bg-gray-700 dark:text-white"
                  placeholder="operator@example.com"
                />
              </div>
              <div>
                <label htmlFor="invite-password" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Password
                </label>
                <input
                  id="invite-password"
                  type="password"
                  required
                  minLength={8}
                  value={invitePassword}
                  onChange={(e) => setInvitePassword(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm dark:bg-gray-700 dark:text-white"
                  placeholder="Minimum 8 characters"
                />
              </div>
              <div>
                <label htmlFor="invite-role" className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Role
                </label>
                <select
                  id="invite-role"
                  value={inviteRole}
                  onChange={(e) => setInviteRole(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm dark:bg-gray-700 dark:text-white"
                >
                  <option value="viewer">Viewer</option>
                  <option value="site_operator">Site Operator</option>
                  <option value="tenant_admin">Tenant Admin</option>
                  <option value="super_admin">Super Admin</option>
                </select>
              </div>
              <div className="flex justify-end space-x-3 pt-4">
                <button
                  type="button"
                  onClick={() => setShowInviteModal(false)}
                  className="px-4 py-2 text-sm font-medium text-gray-700 bg-gray-100 hover:bg-gray-200 dark:bg-gray-700 dark:text-gray-300 dark:hover:bg-gray-600 rounded-md"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={busy}
                  className="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md disabled:opacity-50"
                >
                  Send Invite
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
