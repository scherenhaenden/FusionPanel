import React, { useState } from 'react';
import { Operator, AuditLog } from '../../types';
import { UserPlus, Shield, Activity, Users } from 'lucide-react';

const mockOperators: Operator[] = [
  { id: '1', email: 'admin@example.com', role: 'Super Admin', status: 'Active' },
  { id: '2', email: 'editor@example.com', role: 'Editor', status: 'Active' },
  { id: '3', email: 'viewer@example.com', role: 'Viewer', status: 'Pending' },
];

const mockAuditLogs: AuditLog[] = [
  { id: '1', timestamp: '2023-10-27T10:00:00Z', action: 'Login', operator: 'admin@example.com', details: 'Successful login' },
  { id: '2', timestamp: '2023-10-27T10:15:00Z', action: 'Update', operator: 'admin@example.com', details: 'Updated system settings' },
  { id: '3', timestamp: '2023-10-27T10:30:00Z', action: 'Invite', operator: 'admin@example.com', details: 'Invited new user viewer@example.com' },
];

export default function AdministrationView() {
  const [activeTab, setActiveTab] = useState<'operators' | 'audit'>('operators');
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState('Viewer');
  const [showInviteModal, setShowInviteModal] = useState(false);

  const handleInvite = (e: React.FormEvent) => {
    e.preventDefault();
    // Simulate invite
    console.log(`Inviting ${inviteEmail} as ${inviteRole}`);
    setShowInviteModal(false);
    setInviteEmail('');
  };

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
          <Shield className="w-6 h-6" /> Administration
        </h1>
        {activeTab === 'operators' && (
          <button
            onClick={() => setShowInviteModal(true)}
            className="flex items-center gap-2 bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg transition-colors"
          >
            <UserPlus className="w-4 h-4" /> Invite Operator
          </button>
        )}
      </div>

      <div className="border-b border-gray-200 dark:border-gray-700">
        <nav className="flex space-x-4">
          <button
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

      {activeTab === 'operators' ? (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
          <table className="w-full text-sm text-left text-gray-500 dark:text-gray-400">
            <thead className="text-xs text-gray-700 uppercase bg-gray-50 dark:bg-gray-700 dark:text-gray-300">
              <tr>
                <th className="px-6 py-3">Email</th>
                <th className="px-6 py-3">Role</th>
                <th className="px-6 py-3">Status</th>
                <th className="px-6 py-3">Actions</th>
              </tr>
            </thead>
            <tbody>
              {mockOperators.map((operator) => (
                <tr key={operator.id} className="bg-white border-b dark:bg-gray-800 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
                  <td className="px-6 py-4 font-medium text-gray-900 whitespace-nowrap dark:text-white">
                    {operator.email}
                  </td>
                  <td className="px-6 py-4">{operator.role}</td>
                  <td className="px-6 py-4">
                    <span className={`px-2 py-1 rounded-full text-xs ${
                      operator.status === 'Active'
                        ? 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400'
                        : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400'
                    }`}>
                      {operator.status}
                    </span>
                  </td>
                  <td className="px-6 py-4">
                    <button className="text-blue-600 dark:text-blue-400 hover:underline">Edit</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-sm border border-gray-200 dark:border-gray-700 overflow-hidden">
          <table className="w-full text-sm text-left text-gray-500 dark:text-gray-400">
            <thead className="text-xs text-gray-700 uppercase bg-gray-50 dark:bg-gray-700 dark:text-gray-300">
              <tr>
                <th className="px-6 py-3">Timestamp</th>
                <th className="px-6 py-3">Action</th>
                <th className="px-6 py-3">Operator</th>
                <th className="px-6 py-3">Details</th>
              </tr>
            </thead>
            <tbody>
              {mockAuditLogs.map((log) => (
                <tr key={log.id} className="bg-white border-b dark:bg-gray-800 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-700/50">
                  <td className="px-6 py-4 whitespace-nowrap">
                    {new Date(log.timestamp).toLocaleString()}
                  </td>
                  <td className="px-6 py-4">
                    <span className="px-2 py-1 rounded bg-gray-100 dark:bg-gray-700 text-gray-800 dark:text-gray-200 text-xs font-mono">
                      {log.action}
                    </span>
                  </td>
                  <td className="px-6 py-4">{log.operator}</td>
                  <td className="px-6 py-4">{log.details}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {showInviteModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
          <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full p-6 border border-gray-200 dark:border-gray-700">
            <h2 className="text-xl font-bold mb-4 text-gray-900 dark:text-white">Invite Operator</h2>
            <form onSubmit={handleInvite}>
              <div className="mb-4">
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Email Address
                </label>
                <input
                  type="email"
                  required
                  value={inviteEmail}
                  onChange={(e) => setInviteEmail(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent outline-none"
                  placeholder="operator@example.com"
                />
              </div>
              <div className="mb-6">
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Role
                </label>
                <select
                  value={inviteRole}
                  onChange={(e) => setInviteRole(e.target.value)}
                  className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500 focus:border-transparent outline-none"
                >
                  <option value="Super Admin">Super Admin</option>
                  <option value="Admin">Admin</option>
                  <option value="Editor">Editor</option>
                  <option value="Viewer">Viewer</option>
                </select>
              </div>
              <div className="flex justify-end gap-3">
                <button
                  type="button"
                  onClick={() => setShowInviteModal(false)}
                  className="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors"
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
