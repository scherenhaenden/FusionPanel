import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  Box,
  Lock,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Square,
  Terminal,
  Trash2,
} from 'lucide-react';
import {
  api,
  clampContainerLogLimit,
  ContainerCatalogApp,
  ContainerLogEntry,
  ContainerPortMapping,
  ContainerRecord,
  ContainerState,
  DEFAULT_CONTAINER_LOG_LINES,
  MAX_CONTAINER_LOG_LINES,
} from '../../api';

const READ_ONLY_ROLES = new Set(['viewer', 'readonly', 'read_only', 'auditor']);
const STALE_THRESHOLD_MS = 5 * 60 * 1000;
const POLL_INTERVAL_MS = 2500;
const LOG_LIMIT_OPTIONS = [50, 100, 200, 500];

const TRANSITIONAL_STATES = new Set<ContainerState>([
  'creating',
  'starting',
  'stopping',
  'restarting',
  'deleting',
]);

const STATE_STYLES: Record<ContainerState, string> = {
  creating: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  starting: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  running: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
  stopping: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
  stopped: 'bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-200',
  restarting: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
  deleting: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
  unknown: 'bg-amber-100 text-amber-800 dark:bg-amber-900 dark:text-amber-200',
};

function extractErrorMessage(err: unknown, fallback: string): string {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const resp = (
      err as { response?: { status?: number; data?: { error?: unknown; message?: unknown } } }
    ).response;
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
    if (resp?.status === 403) {
      return 'Permission denied: your role does not allow this container action.';
    }
  }
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

function isForbiddenError(err: unknown): boolean {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const status = (err as { response?: { status?: number } }).response?.status;
    return status === 403;
  }
  return false;
}

function isContainerStale(container: ContainerRecord): boolean {
  if (container.stale) return true;
  if (!container.status_updated_at) return false;
  const updatedMs = Date.parse(container.status_updated_at);
  if (Number.isNaN(updatedMs)) return false;
  return Date.now() - updatedMs > STALE_THRESHOLD_MS;
}

function formatPort(port: string | ContainerPortMapping): string {
  if (typeof port === 'string') return port;
  const proto = port.protocol ? `/${port.protocol}` : '';
  return `${port.host_port}:${port.container_port}${proto}`;
}

function formatTimestamp(value?: string | null): string {
  if (!value) return '—';
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

export default function ContainersView() {
  const [containers, setContainers] = useState<ContainerRecord[]>([]);
  const [catalog, setCatalog] = useState<ContainerCatalogApp[]>([]);
  const [loading, setLoading] = useState(true);
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [deployBusy, setDeployBusy] = useState(false);

  const [error, setError] = useState<string | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [staleListBanner, setStaleListBanner] = useState(false);

  const [isReadOnly, setIsReadOnly] = useState(false);
  const [userRoles, setUserRoles] = useState<string[]>([]);

  const [showDeployModal, setShowDeployModal] = useState(false);
  const [selectedAppId, setSelectedAppId] = useState('');
  const [containerName, setContainerName] = useState('');
  const [hostPort, setHostPort] = useState('');
  const [formError, setFormError] = useState<string | null>(null);

  const [confirmRemoveTarget, setConfirmRemoveTarget] = useState<ContainerRecord | null>(null);

  const [logsContainer, setLogsContainer] = useState<ContainerRecord | null>(null);
  const [logs, setLogs] = useState<ContainerLogEntry[]>([]);
  const [logsTruncated, setLogsTruncated] = useState(false);
  const [logLimit, setLogLimit] = useState<number>(DEFAULT_CONTAINER_LOG_LINES);
  const [logsLoading, setLogsLoading] = useState(false);
  const [logsError, setLogsError] = useState<string | null>(null);

  const loadRole = useCallback(async () => {
    try {
      const me = await api.getMe();
      const roles = Array.isArray(me?.roles) ? me.roles : [];
      setUserRoles(roles);
      if (roles.length > 0 && roles.every((r) => READ_ONLY_ROLES.has(r.toLowerCase()))) {
        setIsReadOnly(true);
      }
    } catch {
      // Non-fatal: backend still enforces RBAC and returns 403 on unauthorized mutations
    }
  }, []);

  const loadContainers = useCallback(async (isInitial = false) => {
    if (isInitial) setLoading(true);
    try {
      const list = await api.getContainers();
      setContainers(list);
      setError(null);
      setStaleListBanner(false);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to load containers'));
      setContainers((prev) => {
        if (prev.length > 0) {
          setStaleListBanner(true);
        }
        return prev;
      });
    } finally {
      if (isInitial) setLoading(false);
    }
  }, []);

  const loadCatalog = useCallback(async () => {
    setCatalogLoading(true);
    setCatalogError(null);
    try {
      const apps = await api.getContainerCatalog();
      setCatalog(apps);
      if (apps.length > 0) {
        setSelectedAppId((prev) => prev || apps[0].id);
      }
    } catch (err) {
      setCatalogError(extractErrorMessage(err, 'Failed to load app catalog'));
    } finally {
      setCatalogLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadRole();
    void loadContainers(true);
    void loadCatalog();
  }, [loadRole, loadContainers, loadCatalog]);

  const hasTransitional = containers.some((c) => TRANSITIONAL_STATES.has(c.state));

  useEffect(() => {
    if (!hasTransitional) return undefined;
    const timer = window.setTimeout(() => {
      void loadContainers(false);
    }, POLL_INTERVAL_MS);
    return () => window.clearTimeout(timer);
  }, [hasTransitional, loadContainers]);

  const selectedCatalogApp = useMemo(
    () => catalog.find((app) => app.id === selectedAppId) || null,
    [catalog, selectedAppId],
  );

  useEffect(() => {
    if (selectedCatalogApp && selectedCatalogApp.default_port != null) {
      setHostPort(String(selectedCatalogApp.default_port));
    } else {
      setHostPort('');
    }
  }, [selectedCatalogApp]);

  const loadLogs = useCallback(async (containerId: string, limit: number) => {
    const bounded = clampContainerLogLimit(limit);
    setLogsLoading(true);
    setLogsError(null);
    try {
      const res = await api.getContainerLogs(containerId, bounded);
      const entries = res.logs || [];
      const wasClipped = Boolean(res.truncated) || entries.length > bounded;
      setLogs(entries.slice(-bounded));
      setLogsTruncated(wasClipped);
    } catch (err) {
      setLogs([]);
      setLogsTruncated(false);
      setLogsError(extractErrorMessage(err, 'Failed to load container logs'));
    } finally {
      setLogsLoading(false);
    }
  }, []);

  const openLogs = (container: ContainerRecord) => {
    setLogsContainer(container);
    setLogs([]);
    setLogsTruncated(false);
    setLogsError(null);
    void loadLogs(container.id, logLimit);
  };

  const handleLogLimitChange = (newLimit: number) => {
    const bounded = clampContainerLogLimit(newLimit);
    setLogLimit(bounded);
    if (logsContainer) {
      void loadLogs(logsContainer.id, bounded);
    }
  };

  const handleDeploy = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isReadOnly) return;

    const trimmedName = containerName.trim();
    if (!selectedCatalogApp) {
      setFormError('Please select a pinned app from the catalog.');
      return;
    }
    if (!trimmedName || !/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(trimmedName)) {
      setFormError('Container name must start with a letter or digit and use only letters, digits, ., _, or -.');
      return;
    }

    let parsedPort: number | undefined;
    if (hostPort.trim() !== '') {
      const portNum = Number(hostPort.trim());
      if (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535) {
        setFormError('Host port must be an integer between 1 and 65535.');
        return;
      }
      parsedPort = portNum;
    }

    setDeployBusy(true);
    setFormError(null);
    setError(null);
    try {
      await api.createContainer({
        name: trimmedName,
        app_id: selectedCatalogApp.id,
        image: selectedCatalogApp.image,
        ...(parsedPort !== undefined ? { port: parsedPort } : {}),
      });
      setContainerName('');
      setShowDeployModal(false);
      await loadContainers(false);
    } catch (err) {
      if (isForbiddenError(err)) setIsReadOnly(true);
      setFormError(extractErrorMessage(err, 'Failed to deploy container'));
    } finally {
      setDeployBusy(false);
    }
  };

  const handleLifecycleAction = async (
    container: ContainerRecord,
    action: 'start' | 'stop' | 'restart',
  ) => {
    if (isReadOnly) return;
    setBusyId(container.id);
    setError(null);
    try {
      if (action === 'start') await api.startContainer(container.id);
      if (action === 'stop') await api.stopContainer(container.id);
      if (action === 'restart') await api.restartContainer(container.id);
      await loadContainers(false);
    } catch (err) {
      if (isForbiddenError(err)) setIsReadOnly(true);
      setError(extractErrorMessage(err, `Failed to ${action} container "${container.name}"`));
    } finally {
      setBusyId(null);
    }
  };

  const handleConfirmRemove = async () => {
    if (!confirmRemoveTarget || isReadOnly) return;
    const target = confirmRemoveTarget;
    setBusyId(target.id);
    setError(null);
    try {
      await api.removeContainer(target.id);
      setConfirmRemoveTarget(null);
      if (logsContainer?.id === target.id) {
        setLogsContainer(null);
        setLogs([]);
      }
      await loadContainers(false);
    } catch (err) {
      if (isForbiddenError(err)) setIsReadOnly(true);
      setError(extractErrorMessage(err, `Failed to remove container "${target.name}"`));
      setConfirmRemoveTarget(null);
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
            <Box className="w-6 h-6 text-blue-600 dark:text-blue-400" />
            Containers
          </h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Deploy pinned catalog apps, monitor container status, inspect bounded logs, and manage lifecycle actions.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => void loadContainers(false)}
            className="inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
          >
            <RefreshCw className="w-4 h-4" />
            Refresh
          </button>
          <button
            type="button"
            disabled={isReadOnly}
            onClick={() => {
              setFormError(null);
              setShowDeployModal(true);
            }}
            className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Plus className="w-4 h-4" />
            Deploy App
          </button>
        </div>
      </div>

      {isReadOnly && (
        <div
          role="status"
          className="flex items-center gap-2 rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-200"
        >
          <Lock className="w-4 h-4 shrink-0" />
          <span>
            Read-only access{userRoles.length > 0 ? ` (${userRoles.join(', ')})` : ''}: container deployment and lifecycle actions are disabled.
          </span>
        </div>
      )}

      {staleListBanner && (
        <div
          role="status"
          className="flex items-center gap-2 rounded-lg border border-amber-300 bg-amber-50 p-3 text-sm text-amber-800 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-200"
        >
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>Showing previously loaded container data; status may be stale until refresh succeeds.</span>
        </div>
      )}

      {error && (
        <div
          role="alert"
          className="rounded-lg border border-red-300 bg-red-50 p-3 text-sm text-red-700 dark:border-red-700 dark:bg-red-900/30 dark:text-red-200"
        >
          {error}
        </div>
      )}

      <div className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
        {loading ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">
            Loading containers...
          </div>
        ) : containers.length === 0 && !error ? (
          <div className="p-8 text-center space-y-2">
            <p className="text-sm font-medium text-gray-900 dark:text-white">No containers deployed yet.</p>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {isReadOnly
                ? 'Contact an administrator to deploy an application from the pinned catalog.'
                : 'Select a pinned image from the app catalog to deploy your first container.'}
            </p>
          </div>
        ) : containers.length === 0 ? (
          <div className="p-8 text-center">
            <button
              type="button"
              onClick={() => void loadContainers(true)}
              className="inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
            >
              <RefreshCw className="w-4 h-4" />
              Retry
            </button>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-gray-600 dark:text-gray-300">
              <thead className="bg-gray-50 dark:bg-gray-700/50 text-xs uppercase text-gray-500 dark:text-gray-400">
                <tr>
                  <th className="px-4 py-3">Container</th>
                  <th className="px-4 py-3">Pinned Image</th>
                  <th className="px-4 py-3">Status</th>
                  <th className="px-4 py-3">Ports</th>
                  <th className="px-4 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {containers.map((container) => {
                  const stale = isContainerStale(container);
                  const isBusy = busyId === container.id;
                  const isRunning = container.state === 'running';
                  const isTransitional = TRANSITIONAL_STATES.has(container.state);
                  const badgeStyle = STATE_STYLES[container.state] || STATE_STYLES.unknown;

                  return (
                    <tr key={container.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/30">
                      <td className="px-4 py-3">
                        <div className="font-medium text-gray-900 dark:text-white">{container.name}</div>
                        <div className="text-xs text-gray-400 font-mono">{container.id}</div>
                        {container.last_error && (
                          <div className="mt-1 text-xs text-red-600 dark:text-red-400">
                            {container.last_error}
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3 font-mono text-xs text-gray-700 dark:text-gray-300">
                        {container.image}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className={`inline-flex rounded-full px-2.5 py-0.5 text-xs font-medium ${badgeStyle}`}>
                            {container.state}
                          </span>
                          {stale && (
                            <span
                              className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-900/60 dark:text-amber-200"
                              title={`Last updated: ${formatTimestamp(container.status_updated_at || container.updated_at)}`}
                            >
                              <AlertTriangle className="w-3 h-3" />
                              Stale
                            </span>
                          )}
                        </div>
                        {container.status_text && (
                          <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                            {container.status_text}
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3 text-xs font-mono">
                        {container.ports && container.ports.length > 0
                          ? container.ports.map(formatPort).join(', ')
                          : '—'}
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center justify-end gap-1.5 flex-wrap">
                          {!isRunning && (
                            <button
                              type="button"
                              disabled={isReadOnly || isBusy || isTransitional}
                              onClick={() => void handleLifecycleAction(container, 'start')}
                              className="inline-flex items-center gap-1 rounded border border-green-300 px-2 py-1 text-xs font-medium text-green-700 hover:bg-green-50 disabled:opacity-50 disabled:cursor-not-allowed dark:border-green-700 dark:text-green-300 dark:hover:bg-green-900/30"
                              aria-label={`Start ${container.name}`}
                            >
                              <Play className="w-3 h-3" />
                              Start
                            </button>
                          )}
                          {isRunning && (
                            <button
                              type="button"
                              disabled={isReadOnly || isBusy || isTransitional}
                              onClick={() => void handleLifecycleAction(container, 'stop')}
                              className="inline-flex items-center gap-1 rounded border border-amber-300 px-2 py-1 text-xs font-medium text-amber-700 hover:bg-amber-50 disabled:opacity-50 disabled:cursor-not-allowed dark:border-amber-700 dark:text-amber-300 dark:hover:bg-amber-900/30"
                              aria-label={`Stop ${container.name}`}
                            >
                              <Square className="w-3 h-3" />
                              Stop
                            </button>
                          )}
                          <button
                            type="button"
                            disabled={isReadOnly || isBusy || isTransitional}
                            onClick={() => void handleLifecycleAction(container, 'restart')}
                            className="inline-flex items-center gap-1 rounded border border-gray-300 px-2 py-1 text-xs font-medium text-gray-700 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
                            aria-label={`Restart ${container.name}`}
                          >
                            <RotateCw className="w-3 h-3" />
                            Restart
                          </button>
                          <button
                            type="button"
                            onClick={() => openLogs(container)}
                            className="inline-flex items-center gap-1 rounded border border-blue-300 px-2 py-1 text-xs font-medium text-blue-700 hover:bg-blue-50 dark:border-blue-700 dark:text-blue-300 dark:hover:bg-blue-900/30"
                            aria-label={`Logs ${container.name}`}
                          >
                            <Terminal className="w-3 h-3" />
                            Logs
                          </button>
                          <button
                            type="button"
                            disabled={isReadOnly || isBusy}
                            onClick={() => setConfirmRemoveTarget(container)}
                            className="inline-flex items-center gap-1 rounded border border-red-300 px-2 py-1 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-50 disabled:cursor-not-allowed dark:border-red-700 dark:text-red-300 dark:hover:bg-red-900/30"
                            aria-label={`Remove ${container.name}`}
                          >
                            <Trash2 className="w-3 h-3" />
                            Remove
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Bounded Logs Viewer */}
      {logsContainer && (
        <section
          aria-label="Container Logs"
          className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-3"
        >
          <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
            <div>
              <h2 className="text-base font-semibold text-gray-900 dark:text-white">
                Logs: {logsContainer.name}
              </h2>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Bounded tail view (max {MAX_CONTAINER_LOG_LINES} lines).
              </p>
            </div>
            <div className="flex items-center gap-2">
              <label htmlFor="container-log-limit" className="text-xs text-gray-600 dark:text-gray-300">
                Line limit
              </label>
              <select
                id="container-log-limit"
                value={logLimit}
                onChange={(e) => handleLogLimitChange(Number(e.target.value))}
                className="rounded border border-gray-300 bg-white px-2 py-1 text-xs text-gray-800 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
              >
                {LOG_LIMIT_OPTIONS.map((opt) => (
                  <option key={opt} value={opt}>
                    {opt} lines
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={() => void loadLogs(logsContainer.id, logLimit)}
                className="inline-flex items-center gap-1 rounded border border-gray-300 px-2.5 py-1 text-xs font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
              >
                <RefreshCw className="w-3 h-3" />
                Reload Logs
              </button>
              <button
                type="button"
                onClick={() => {
                  setLogsContainer(null);
                  setLogs([]);
                  setLogsError(null);
                }}
                className="rounded border border-gray-300 px-2.5 py-1 text-xs font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
              >
                Close
              </button>
            </div>
          </div>

          {logsTruncated && (
            <div
              role="status"
              className="rounded border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-200"
            >
              Log output truncated to the latest {logLimit} lines (limit {MAX_CONTAINER_LOG_LINES}).
            </div>
          )}

          {logsError && (
            <div
              role="alert"
              className="rounded border border-red-300 bg-red-50 px-3 py-2 text-xs text-red-700 dark:border-red-700 dark:bg-red-900/30 dark:text-red-200"
            >
              {logsError}
            </div>
          )}

          {logsLoading ? (
            <div className="py-4 text-xs text-gray-500 dark:text-gray-400">Loading logs...</div>
          ) : logs.length === 0 ? (
            <div className="py-4 text-xs text-gray-500 dark:text-gray-400">No log output available.</div>
          ) : (
            <pre className="max-h-72 overflow-y-auto rounded bg-gray-900 p-3 text-xs text-gray-100 font-mono whitespace-pre-wrap">
              {logs
                .map((entry) => {
                  const prefix = [entry.timestamp, entry.stream].filter(Boolean).join(' ');
                  return prefix ? `[${prefix}] ${entry.text}` : entry.text;
                })
                .join('\n')}
            </pre>
          )}
        </section>
      )}

      {/* Deploy App Modal (Pinned Catalog) */}
      {showDeployModal && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="deploy-container-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
        >
          <div className="w-full max-w-lg rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800 space-y-4">
            <h2 id="deploy-container-title" className="text-lg font-semibold text-gray-900 dark:text-white">
              Deploy App from Pinned Catalog
            </h2>

            {catalogError && (
              <div
                role="alert"
                className="rounded border border-red-300 bg-red-50 p-2.5 text-xs text-red-700 dark:border-red-700 dark:bg-red-900/30 dark:text-red-200"
              >
                {catalogError}
              </div>
            )}

            {formError && (
              <div
                role="alert"
                className="rounded border border-red-300 bg-red-50 p-2.5 text-xs text-red-700 dark:border-red-700 dark:bg-red-900/30 dark:text-red-200"
              >
                {formError}
              </div>
            )}

            <form onSubmit={(e) => void handleDeploy(e)} className="space-y-4">
              <div>
                <label
                  htmlFor="catalog-app-select"
                  className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
                >
                  Catalog App (Pinned Image)
                </label>
                {catalogLoading ? (
                  <p className="text-xs text-gray-500">Loading catalog...</p>
                ) : catalog.length === 0 ? (
                  <p className="text-xs text-gray-500">No pinned catalog apps available.</p>
                ) : (
                  <select
                    id="catalog-app-select"
                    value={selectedAppId}
                    onChange={(e) => setSelectedAppId(e.target.value)}
                    className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                  >
                    {catalog.map((app) => (
                      <option key={app.id} value={app.id}>
                        {app.name} ({app.image})
                      </option>
                    ))}
                  </select>
                )}
                {selectedCatalogApp?.description && (
                  <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                    {selectedCatalogApp.description}
                  </p>
                )}
              </div>

              <div>
                <label
                  htmlFor="container-name-input"
                  className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
                >
                  Container Name
                </label>
                <input
                  id="container-name-input"
                  type="text"
                  value={containerName}
                  onChange={(e) => setContainerName(e.target.value)}
                  placeholder="e.g. web-cache"
                  className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                />
              </div>

              <div>
                <label
                  htmlFor="container-port-input"
                  className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
                >
                  Host Port (optional)
                </label>
                <input
                  id="container-port-input"
                  type="number"
                  min={1}
                  max={65535}
                  value={hostPort}
                  onChange={(e) => setHostPort(e.target.value)}
                  placeholder="8080"
                  className="w-full rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-700 dark:text-white"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setShowDeployModal(false)}
                  className="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={deployBusy || catalogLoading || catalog.length === 0 || isReadOnly}
                  className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {deployBusy ? 'Deploying...' : 'Deploy Container'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Confirmed Delete Modal */}
      {confirmRemoveTarget && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-remove-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
        >
          <div className="w-full max-w-md rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800 space-y-4">
            <h2 id="confirm-remove-title" className="text-lg font-semibold text-gray-900 dark:text-white">
              Confirm Container Removal
            </h2>
            <p className="text-sm text-gray-600 dark:text-gray-300">
              Are you sure you want to remove container{' '}
              <span className="font-semibold text-gray-900 dark:text-white">
                {confirmRemoveTarget.name}
              </span>
              ? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setConfirmRemoveTarget(null)}
                className="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={busyId === confirmRemoveTarget.id || isReadOnly}
                onClick={() => void handleConfirmRemove()}
                className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-50"
              >
                {busyId === confirmRemoveTarget.id ? 'Removing...' : 'Confirm Remove'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
