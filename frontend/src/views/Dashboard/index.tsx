import React, { FormEvent, useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../../api';
import {
  AlertDeliveryRecord,
  AlertMetric,
  AlertRuleRecord,
  AlertSeverity,
  AlertStateRecord,
  AlertOperator,
  CapabilitiesResponse,
  HealthResponse,
  JobRecord,
  NodeMetricSample,
  NodeRecord,
} from '../../types';

const REFRESH_INTERVAL_MS = 30_000;
const METRIC_STALE_AFTER_MS = 90_000;

const errorMessage = (error: unknown): string => {
  const value = error as { response?: { data?: { error?: { message?: string } } }; message?: string };
  if (value.response?.data?.error?.message) return value.response.data.error.message;
  if (value.response && 'status' in value.response) {
    const status = (value.response as { status?: number }).status;
    if (status === 401 || status === 403) return 'You are not authorized to view live telemetry and alerts.';
  }
  return value.message || 'Failed to load dashboard data.';
};

const formatDate = (value: string): string => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 'Unknown' : date.toLocaleString();
};

const formatMemory = (kilobytes: number): string => {
  const megabytes = kilobytes / 1024;
  return megabytes >= 1024
    ? `${(megabytes / 1024).toFixed(1)} GiB`
    : `${megabytes.toFixed(0)} MiB`;
};

const memoryPercent = (sample: NodeMetricSample): number => {
  const total = sample.memory_used_kb + sample.memory_available_kb;
  return total > 0 ? (sample.memory_used_kb / total) * 100 : 0;
};

const isStale = (sample: NodeMetricSample): boolean => {
  const timestamp = new Date(sample.recorded_at).getTime();
  return Number.isNaN(timestamp) || Date.now() - timestamp > METRIC_STALE_AFTER_MS;
};

const METRIC_LABELS: Record<AlertMetric, string> = {
  cpu_usage_percent: 'CPU usage (%)',
  memory_used_percent: 'Memory used (%)',
  disk_io_ops: 'Disk I/O operations',
};

interface RuleFormState {
  name: string;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: string;
  severity: AlertSeverity;
  webhook_url: string;
  webhook_secret: string;
}

const initialRuleForm: RuleFormState = {
  name: '',
  metric: 'cpu_usage_percent',
  operator: 'above',
  threshold: '85',
  severity: 'warning',
  webhook_url: '',
  webhook_secret: '',
};

export const Dashboard: React.FC = () => {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [capabilities, setCapabilities] = useState<CapabilitiesResponse | null>(null);
  const [nodes, setNodes] = useState<NodeRecord[]>([]);
  const [jobs, setJobs] = useState<JobRecord[]>([]);
  const [samples, setSamples] = useState<NodeMetricSample[]>([]);
  const [history, setHistory] = useState<NodeMetricSample[]>([]);
  const [activeAlerts, setActiveAlerts] = useState<AlertStateRecord[]>([]);
  const [rules, setRules] = useState<AlertRuleRecord[]>([]);
  const [deliveries, setDeliveries] = useState<AlertDeliveryRecord[]>([]);
  const [selectedNodeId, setSelectedNodeId] = useState('');
  const selectedNodeRef = useRef('');
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [isHistoryLoading, setIsHistoryLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const [ruleError, setRuleError] = useState<string | null>(null);
  const [ruleSuccess, setRuleSuccess] = useState<string | null>(null);
  const [isSavingRule, setIsSavingRule] = useState(false);
  const [deletingRuleId, setDeletingRuleId] = useState<string | null>(null);
  const [ruleForm, setRuleForm] = useState<RuleFormState>(initialRuleForm);

  const loadData = useCallback(async () => {
    setIsRefreshing(true);
    try {
      const [healthData, capabilitiesData, nodeData, jobData, latest, active, ruleData, deliveryData] =
        await Promise.all([
          api.getHealth(),
          api.getCapabilities(),
          api.getNodes(),
          api.getJobs(),
          api.getLatestMetrics(),
          api.getActiveAlerts(),
          api.getAlertRules(),
          api.getAlertDeliveries(10),
        ]);

      setHealth(healthData);
      setCapabilities(capabilitiesData);
      setNodes(nodeData);
      setJobs(jobData);
      setSamples(latest);
      setActiveAlerts(active);
      setRules(ruleData);
      setDeliveries(deliveryData);
      setError(null);

      const historyNodeId = latest.some((sample) => sample.node_id === selectedNodeRef.current)
        ? selectedNodeRef.current
        : latest[0]?.node_id || '';
      selectedNodeRef.current = historyNodeId;
      setSelectedNodeId(historyNodeId);
      if (historyNodeId) {
        try {
          setHistory(await api.getNodeMetrics(historyNodeId));
          setHistoryError(null);
        } catch (historyLoadError) {
          setHistoryError(errorMessage(historyLoadError));
        }
      } else {
        setHistory([]);
        setHistoryError(null);
      }
    } catch (loadError) {
      setError(errorMessage(loadError));
    } finally {
      setIsLoading(false);
      setIsRefreshing(false);
    }
  }, []);

  useEffect(() => {
    void loadData();
    const interval = window.setInterval(() => void loadData(), REFRESH_INTERVAL_MS);
    return () => window.clearInterval(interval);
  }, [loadData]);

  const selectNode = async (nodeId: string) => {
    selectedNodeRef.current = nodeId;
    setSelectedNodeId(nodeId);
    setIsHistoryLoading(true);
    setHistoryError(null);
    try {
      setHistory(await api.getNodeMetrics(nodeId));
    } catch (historyLoadError) {
      setHistory([]);
      setHistoryError(errorMessage(historyLoadError));
    } finally {
      setIsHistoryLoading(false);
    }
  };

  const submitRule = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setIsSavingRule(true);
    setRuleError(null);
    setRuleSuccess(null);
    try {
      const rule = await api.createAlertRule({
        ...ruleForm,
        threshold: Number(ruleForm.threshold),
        enabled: true,
      });
      setRules((current) => [...current, rule]);
      setRuleForm(initialRuleForm);
      setRuleSuccess('Alert rule created. The webhook secret is stored encrypted and will not be shown again.');
      await loadData();
    } catch (saveError) {
      setRuleError(errorMessage(saveError));
    } finally {
      setIsSavingRule(false);
    }
  };

  const removeRule = async (ruleId: string) => {
    setDeletingRuleId(ruleId);
    setRuleError(null);
    setRuleSuccess(null);
    try {
      await api.deleteAlertRule(ruleId);
      setRules((current) => current.filter((rule) => rule.id !== ruleId));
      setActiveAlerts((current) => current.filter((alert) => alert.rule_id !== ruleId));
      setDeliveries((current) => current.filter((delivery) => delivery.rule_id !== ruleId));
      setRuleSuccess('Alert rule deleted.');
    } catch (deleteError) {
      setRuleError(errorMessage(deleteError));
    } finally {
      setDeletingRuleId(null);
    }
  };

  const sampleByNode = new Map(samples.map((sample) => [sample.node_id, sample]));
  const ruleById = new Map(rules.map((rule) => [rule.id, rule]));
  const selectedSample = sampleByNode.get(selectedNodeId);
  const selectedNode = nodes.find((node) => node.node_id === selectedNodeId);
  const chartCoordinates = history
    .map((sample, index) => {
      const x = history.length <= 1 ? 50 : (index / (history.length - 1)) * 100;
      const y = 76 - (Math.max(0, Math.min(100, sample.cpu_usage_percent)) / 100) * 64;
      return { x, y };
    });
  const chartPoints = chartCoordinates.map(({ x, y }) => `${x},${y}`).join(' ');
  const failedJobs = jobs.filter((job) => job.state === 'failed').length;
  const runningJobs = jobs.filter((job) => job.state === 'running').length;
  const queuedJobs = jobs.filter((job) => job.state === 'queued').length;
  const onlineNodes = nodes.filter((node) => node.state === 'online').length;

  return (
    <main className="space-y-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Dashboard</h1>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
            Live node telemetry, job health, and alert delivery.
          </p>
        </div>
        <button
          type="button"
          onClick={() => void loadData()}
          disabled={isRefreshing}
          className="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isRefreshing ? 'Refreshing…' : 'Refresh dashboard'}
        </button>
      </header>

      {error && (
        <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          <p>{error}</p>
          <button type="button" onClick={() => void loadData()} className="mt-2 underline">
            Retry
          </button>
        </div>
      )}

      <section aria-labelledby="telemetry-heading" className="space-y-4">
        <div>
          <h2 id="telemetry-heading" className="text-lg font-semibold text-gray-900 dark:text-white">
            Node telemetry
          </h2>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
            Agent samples refresh every 30 seconds. History is limited to the most recent 120 samples per node.
          </p>
        </div>

        {isLoading && nodes.length === 0 ? (
          <p role="status" className="rounded border border-gray-200 bg-white p-5 text-gray-600 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300">
            Loading telemetry…
          </p>
        ) : nodes.length === 0 ? (
          <div className="rounded border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
            <p className="font-medium text-gray-900 dark:text-white">No nodes registered yet.</p>
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
              A running FusionPanel agent will appear here after it registers with the API.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto rounded-lg border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
            <table className="min-w-full divide-y divide-gray-200 text-left dark:divide-gray-700">
              <caption className="sr-only">Current telemetry for registered FusionPanel nodes</caption>
              <thead className="bg-gray-50 text-xs uppercase tracking-wide text-gray-500 dark:bg-gray-900 dark:text-gray-400">
                <tr>
                  <th scope="col" className="px-4 py-3">Node</th>
                  <th scope="col" className="px-4 py-3">Node status</th>
                  <th scope="col" className="px-4 py-3">Telemetry</th>
                  <th scope="col" className="px-4 py-3">CPU</th>
                  <th scope="col" className="px-4 py-3">Memory</th>
                  <th scope="col" className="px-4 py-3">Disk I/O</th>
                  <th scope="col" className="px-4 py-3">Sample time</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {nodes.map((node) => {
                  const sample = sampleByNode.get(node.node_id);
                  const stale = sample ? isStale(sample) : false;
                  return (
                    <tr key={node.node_id}>
                      <th scope="row" className="px-4 py-4 font-medium text-gray-900 dark:text-white">
                        <span>{node.name}</span>
                        <span className="mt-1 block font-mono text-xs font-normal text-gray-500 dark:text-gray-400">{node.node_id}</span>
                      </th>
                      <td className="px-4 py-4 text-sm capitalize text-gray-700 dark:text-gray-200">{node.state}</td>
                      <td className="px-4 py-4 text-sm">
                        {sample ? (
                          <span className={stale ? 'text-amber-700 dark:text-amber-300' : 'text-green-700 dark:text-green-300'}>
                            {stale ? 'Stale' : 'Fresh'}
                          </span>
                        ) : (
                          <span className="text-gray-500 dark:text-gray-400">Waiting for first sample</span>
                        )}
                      </td>
                      <td className="px-4 py-4 text-sm text-gray-700 dark:text-gray-200">{sample ? `${sample.cpu_usage_percent.toFixed(1)}%` : '—'}</td>
                      <td className="px-4 py-4 text-sm text-gray-700 dark:text-gray-200">
                        {sample ? `${memoryPercent(sample).toFixed(1)}% (${formatMemory(sample.memory_used_kb)} / ${formatMemory(sample.memory_used_kb + sample.memory_available_kb)})` : '—'}
                      </td>
                      <td className="px-4 py-4 text-sm text-gray-700 dark:text-gray-200">{sample ? sample.disk_io_ops.toLocaleString() : '—'}</td>
                      <td className="px-4 py-4 text-sm text-gray-600 dark:text-gray-300">{sample ? formatDate(sample.recorded_at) : '—'}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}

        {selectedSample && (
          <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <h3 className="font-medium text-gray-900 dark:text-white">Recent CPU history</h3>
                <p className="text-sm text-gray-600 dark:text-gray-300">
                  {selectedNode?.name || selectedNodeId} · {history.length} of up to 120 samples
                </p>
              </div>
              {samples.length > 1 && (
                <label className="text-sm text-gray-700 dark:text-gray-200">
                  Node history
                  <select
                    aria-label="Node history"
                    value={selectedNodeId}
                    onChange={(event) => void selectNode(event.target.value)}
                    className="ml-2 rounded border border-gray-300 bg-white px-2 py-1 dark:border-gray-600 dark:bg-gray-900"
                  >
                    {samples.map((sample) => (
                      <option key={sample.node_id} value={sample.node_id}>
                        {nodes.find((node) => node.node_id === sample.node_id)?.name || sample.node_id}
                      </option>
                    ))}
                  </select>
                </label>
              )}
            </div>
            {historyError ? (
              <p role="alert" className="mt-3 text-sm text-red-700 dark:text-red-300">{historyError}</p>
            ) : isHistoryLoading ? (
              <p role="status" className="mt-3 text-sm text-gray-600 dark:text-gray-300">Loading node history…</p>
            ) : history.length === 0 ? (
              <p className="mt-3 text-sm text-gray-600 dark:text-gray-300">No historical samples are available for this node.</p>
            ) : (
              <>
                <svg role="img" aria-label={`CPU history for ${selectedNode?.name || selectedNodeId}`} viewBox="0 0 100 80" className="mt-4 h-28 w-full overflow-visible" preserveAspectRatio="none">
                  <line x1="0" y1="12" x2="100" y2="12" stroke="currentColor" strokeOpacity="0.15" />
                  <line x1="0" y1="44" x2="100" y2="44" stroke="currentColor" strokeOpacity="0.15" />
                  <line x1="0" y1="76" x2="100" y2="76" stroke="currentColor" strokeOpacity="0.15" />
                  <polyline points={chartPoints} fill="none" stroke="#2563eb" strokeWidth="2" vectorEffect="non-scaling-stroke" />
                  {chartCoordinates.map(({ x, y }, index) => <circle key={`${history[index].recorded_at}:${index}`} cx={x} cy={y} r="1.5" fill="#2563eb" />)}
                </svg>
                <div className="flex justify-between text-xs text-gray-500 dark:text-gray-400">
                  <span>{history[0] ? formatDate(history[0].recorded_at) : ''}</span>
                  <span>{history[history.length - 1] ? formatDate(history[history.length - 1].recorded_at) : ''}</span>
                </div>
              </>
            )}
          </div>
        )}
      </section>

      <section aria-labelledby="health-heading" className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <h2 id="health-heading" className="font-semibold text-gray-900 dark:text-white">Node and job health</h2>
          <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
            <div><dt className="text-gray-500 dark:text-gray-400">Nodes online</dt><dd className="mt-1 text-lg font-semibold text-gray-900 dark:text-white">{onlineNodes} / {nodes.length}</dd></div>
            <div><dt className="text-gray-500 dark:text-gray-400">Jobs running</dt><dd className="mt-1 text-lg font-semibold text-gray-900 dark:text-white">{runningJobs}</dd></div>
            <div><dt className="text-gray-500 dark:text-gray-400">Jobs queued</dt><dd className="mt-1 text-lg font-semibold text-gray-900 dark:text-white">{queuedJobs}</dd></div>
            <div><dt className="text-gray-500 dark:text-gray-400">Jobs failed</dt><dd className="mt-1 text-lg font-semibold text-gray-900 dark:text-white">{failedJobs}</dd></div>
          </dl>
        </div>
        <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <h2 className="font-semibold text-gray-900 dark:text-white">API status</h2>
          {health ? (
            <dl className="mt-4 grid grid-cols-2 gap-4 text-sm">
              <div><dt className="text-gray-500 dark:text-gray-400">Status</dt><dd className="mt-1 font-medium text-gray-900 dark:text-white">{health.status}</dd></div>
              <div><dt className="text-gray-500 dark:text-gray-400">Version</dt><dd className="mt-1 font-medium text-gray-900 dark:text-white">{health.version}</dd></div>
              <div className="col-span-2"><dt className="text-gray-500 dark:text-gray-400">Capabilities</dt><dd className="mt-1 text-gray-700 dark:text-gray-200">{capabilities?.capabilities.join(', ') || 'None reported'}</dd></div>
            </dl>
          ) : (
            <p className="mt-4 text-sm text-gray-600 dark:text-gray-300">No API health data available.</p>
          )}
        </div>
      </section>

      <section aria-labelledby="alerts-heading" className="space-y-4">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 id="alerts-heading" className="text-lg font-semibold text-gray-900 dark:text-white">Alerts and signed webhooks</h2>
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">Rules evaluate live CPU, memory, and disk-I/O samples.</p>
          </div>
          <p className="text-sm text-gray-700 dark:text-gray-200">{activeAlerts.length} active alert{activeAlerts.length === 1 ? '' : 's'}</p>
        </div>

        {activeAlerts.length > 0 && (
          <div className="overflow-x-auto rounded-lg border border-amber-300 bg-amber-50 dark:border-amber-800 dark:bg-amber-950">
            <table className="min-w-full divide-y divide-amber-200 text-left dark:divide-amber-900">
              <caption className="px-4 py-3 text-left font-medium text-amber-900 dark:text-amber-100">Active alerts</caption>
              <thead className="text-xs uppercase tracking-wide text-amber-800 dark:text-amber-200"><tr><th className="px-4 py-2">Rule</th><th className="px-4 py-2">Node</th><th className="px-4 py-2">Current value</th><th className="px-4 py-2">Since</th></tr></thead>
              <tbody className="divide-y divide-amber-200 text-sm text-amber-950 dark:divide-amber-900 dark:text-amber-100">
                {activeAlerts.map((alert) => {
                  const rule = ruleById.get(alert.rule_id);
                  return <tr key={`${alert.rule_id}:${alert.node_id}`}><td className="px-4 py-3">{rule?.name || alert.rule_id}</td><td className="px-4 py-3">{nodes.find((node) => node.node_id === alert.node_id)?.name || alert.node_id}</td><td className="px-4 py-3">{rule ? `${METRIC_LABELS[rule.metric]} ${rule.operator} ${alert.last_value.toFixed(1)}` : alert.last_value.toFixed(1)}</td><td className="px-4 py-3">{alert.started_at ? formatDate(alert.started_at) : formatDate(alert.updated_at)}</td></tr>;
                })}
              </tbody>
            </table>
          </div>
        )}

        <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,0.8fr)]">
          <div className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
            <h3 className="font-medium text-gray-900 dark:text-white">Alert rules</h3>
            {rules.length === 0 ? (
              <p className="mt-3 text-sm text-gray-600 dark:text-gray-300">No alert rules configured.</p>
            ) : (
              <ul className="mt-3 divide-y divide-gray-200 dark:divide-gray-700">
                {rules.map((rule) => (
                  <li key={rule.id} className="flex flex-wrap items-center justify-between gap-3 py-3">
                    <div>
                      <p className="font-medium text-gray-900 dark:text-white">{rule.name}</p>
                      <p className="text-sm text-gray-600 dark:text-gray-300">
                        {METRIC_LABELS[rule.metric]} {rule.operator} {rule.threshold} · {rule.severity} · {rule.enabled ? 'enabled' : 'disabled'}
                      </p>
                      <p className="break-all text-xs text-gray-500 dark:text-gray-400">{rule.webhook_url} · {rule.has_secret ? 'secret stored' : 'no secret'}</p>
                    </div>
                    <button type="button" onClick={() => void removeRule(rule.id)} disabled={deletingRuleId === rule.id} className="rounded border border-red-300 px-3 py-1.5 text-sm text-red-700 hover:bg-red-50 disabled:opacity-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-950">
                      {deletingRuleId === rule.id ? 'Deleting…' : 'Delete'}
                    </button>
                  </li>
                ))}
              </ul>
            )}

            <h3 className="mt-6 font-medium text-gray-900 dark:text-white">Recent webhook deliveries</h3>
            {deliveries.length === 0 ? (
              <p className="mt-3 text-sm text-gray-600 dark:text-gray-300">No webhook deliveries recorded yet.</p>
            ) : (
              <div className="mt-3 overflow-x-auto">
                <table className="min-w-full text-left text-sm">
                  <thead className="text-xs uppercase text-gray-500 dark:text-gray-400"><tr><th className="py-2 pr-3">Event</th><th className="py-2 pr-3">Status</th><th className="py-2 pr-3">Attempts</th><th className="py-2">Updated</th></tr></thead>
                  <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                    {deliveries.map((delivery) => (
                      <tr key={delivery.id}><td className="py-2 pr-3 text-gray-800 dark:text-gray-200">{ruleById.get(delivery.rule_id)?.name || delivery.rule_id} · {delivery.event}</td><td className="py-2 pr-3 capitalize text-gray-800 dark:text-gray-200">{delivery.status}{delivery.last_error ? ` (${delivery.last_error})` : ''}</td><td className="py-2 pr-3 text-gray-800 dark:text-gray-200">{delivery.attempt_count}</td><td className="py-2 text-gray-600 dark:text-gray-300">{formatDate(delivery.updated_at)}</td></tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          <form onSubmit={(event) => void submitRule(event)} className="space-y-3 rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
            <h3 className="font-medium text-gray-900 dark:text-white">Create alert rule</h3>
            <label className="block text-sm text-gray-700 dark:text-gray-200">
              Name
              <input required maxLength={128} value={ruleForm.name} onChange={(event) => setRuleForm({ ...ruleForm, name: event.target.value })} className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 dark:border-gray-600 dark:bg-gray-900" />
            </label>
            <div className="grid grid-cols-2 gap-3">
              <label className="block text-sm text-gray-700 dark:text-gray-200">Metric
                <select value={ruleForm.metric} onChange={(event) => setRuleForm({ ...ruleForm, metric: event.target.value as AlertMetric })} className="mt-1 w-full rounded border border-gray-300 bg-white px-2 py-2 dark:border-gray-600 dark:bg-gray-900">
                  {Object.entries(METRIC_LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                </select>
              </label>
              <label className="block text-sm text-gray-700 dark:text-gray-200">Condition
                <select value={ruleForm.operator} onChange={(event) => setRuleForm({ ...ruleForm, operator: event.target.value as AlertOperator })} className="mt-1 w-full rounded border border-gray-300 bg-white px-2 py-2 dark:border-gray-600 dark:bg-gray-900">
                  <option value="above">Above</option><option value="below">Below</option>
                </select>
              </label>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <label className="block text-sm text-gray-700 dark:text-gray-200">Threshold
                <input type="number" required min="0" max={ruleForm.metric === 'disk_io_ops' ? '1000000000000000' : '100'} step={ruleForm.metric === 'disk_io_ops' ? '1' : '0.1'} value={ruleForm.threshold} onChange={(event) => setRuleForm({ ...ruleForm, threshold: event.target.value })} className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 dark:border-gray-600 dark:bg-gray-900" />
              </label>
              <label className="block text-sm text-gray-700 dark:text-gray-200">Severity
                <select value={ruleForm.severity} onChange={(event) => setRuleForm({ ...ruleForm, severity: event.target.value as AlertSeverity })} className="mt-1 w-full rounded border border-gray-300 bg-white px-2 py-2 dark:border-gray-600 dark:bg-gray-900">
                  <option value="info">Info</option><option value="warning">Warning</option><option value="critical">Critical</option>
                </select>
              </label>
            </div>
            <label className="block text-sm text-gray-700 dark:text-gray-200">Webhook URL
              <input type="url" required maxLength={2048} value={ruleForm.webhook_url} onChange={(event) => setRuleForm({ ...ruleForm, webhook_url: event.target.value })} placeholder="https://alerts.example.com/hooks/fusionpanel" className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 dark:border-gray-600 dark:bg-gray-900" />
            </label>
            <label className="block text-sm text-gray-700 dark:text-gray-200">Signing secret
              <input type="password" required minLength={32} maxLength={1024} autoComplete="new-password" value={ruleForm.webhook_secret} onChange={(event) => setRuleForm({ ...ruleForm, webhook_secret: event.target.value })} className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 dark:border-gray-600 dark:bg-gray-900" />
              <span className="mt-1 block text-xs text-gray-500 dark:text-gray-400">At least 32 characters. It is not returned by the API.</span>
            </label>
            {ruleError && <p role="alert" className="text-sm text-red-700 dark:text-red-300">{ruleError}</p>}
            {ruleSuccess && <p role="status" className="text-sm text-green-700 dark:text-green-300">{ruleSuccess}</p>}
            <button type="submit" disabled={isSavingRule} className="w-full rounded bg-blue-600 px-4 py-2 font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50">
              {isSavingRule ? 'Creating…' : 'Create alert rule'}
            </button>
          </form>
        </div>
      </section>
    </main>
  );
};

export default Dashboard;
