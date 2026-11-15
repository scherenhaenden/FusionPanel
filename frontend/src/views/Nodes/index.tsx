import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../../api';
import { NodeRecord } from '../../types';

const messageFromError = (error: unknown): string => {
  const value = error as { response?: { data?: { error?: { message?: string } } }; message?: string };
  return value.response?.data?.error?.message || value.message || 'Could not load nodes.';
};

const formatDate = (value: string): string => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
};

export const NodesView: React.FC = () => {
  const [nodes, setNodes] = useState<NodeRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadNodes = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      setNodes(await api.getNodes());
    } catch (loadError) {
      setError(messageFromError(loadError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadNodes();
  }, [loadNodes]);

  return (
    <main className="space-y-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Nodes</h1>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
            Registered agents and their most recent heartbeat.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void loadNodes()}
          disabled={isLoading}
          className="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isLoading ? 'Refreshing…' : 'Refresh nodes'}
        </button>
      </header>

      {error && (
        <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          <p>{error}</p>
          <button type="button" onClick={() => void loadNodes()} className="mt-2 underline">
            Retry
          </button>
        </div>
      )}

      <section aria-labelledby="nodes-list-heading" className="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <h2 id="nodes-list-heading" className="sr-only">Registered nodes</h2>
        {isLoading && nodes.length === 0 ? (
          <p role="status" className="p-6 text-gray-600 dark:text-gray-300">Loading nodes…</p>
        ) : error && nodes.length === 0 ? null : nodes.length === 0 ? (
          <div className="p-6">
            <p className="font-medium text-gray-900 dark:text-white">No nodes registered yet.</p>
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
              A running FusionPanel agent will appear here after it registers with the API.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200 text-left dark:divide-gray-700">
              <thead className="bg-gray-50 text-xs uppercase tracking-wide text-gray-500 dark:bg-gray-900 dark:text-gray-400">
                <tr>
                  <th scope="col" className="px-5 py-3">Node</th>
                  <th scope="col" className="px-5 py-3">Status</th>
                  <th scope="col" className="px-5 py-3">Last heartbeat</th>
                  <th scope="col" className="px-5 py-3">Registered</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {nodes.map((node) => (
                  <tr key={node.node_id}>
                    <td className="px-5 py-4">
                      <div className="font-medium text-gray-900 dark:text-white">{node.name}</div>
                      <div className="mt-1 font-mono text-xs text-gray-500 dark:text-gray-400">{node.node_id}</div>
                    </td>
                    <td className="px-5 py-4">
                      <span className={`inline-flex rounded-full px-2.5 py-1 text-xs font-medium ${node.state === 'online' ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200' : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200'}`}>
                        {node.state}
                      </span>
                    </td>
                    <td className="px-5 py-4 text-sm text-gray-700 dark:text-gray-200">{formatDate(node.last_heartbeat)}</td>
                    <td className="px-5 py-4 text-sm text-gray-700 dark:text-gray-200">{formatDate(node.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </main>
  );
};

export default NodesView;
