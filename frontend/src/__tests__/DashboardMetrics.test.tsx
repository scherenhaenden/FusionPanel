import React from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import '@testing-library/jest-dom';
import { api } from '../api';
import {
  AlertDeliveryRecord,
  AlertRuleRecord,
  AlertStateRecord,
  HealthResponse,
  JobRecord,
  NodeMetricSample,
  NodeRecord,
} from '../types';
import { Dashboard } from '../views/Dashboard';

jest.mock('../api', () => ({
  api: {
    getHealth: jest.fn(),
    getCapabilities: jest.fn(),
    getNodes: jest.fn(),
    getJobs: jest.fn(),
    getLatestMetrics: jest.fn(),
    getNodeMetrics: jest.fn(),
    getActiveAlerts: jest.fn(),
    getAlertRules: jest.fn(),
    getAlertDeliveries: jest.fn(),
    createAlertRule: jest.fn(),
    deleteAlertRule: jest.fn(),
  },
}));

const apiMock = jest.mocked(api);
const now = new Date().toISOString();

const node: NodeRecord = {
  node_id: 'node-1',
  name: 'edge-1',
  state: 'online',
  created_at: now,
  last_heartbeat: now,
};

const sample: NodeMetricSample = {
  node_id: node.node_id,
  cpu_usage_percent: 92,
  memory_used_kb: 4096,
  memory_available_kb: 4096,
  disk_io_ops: 42,
  recorded_at: now,
};

const rule: AlertRuleRecord = {
  id: 'rule-1',
  name: 'High CPU',
  metric: 'cpu_usage_percent',
  operator: 'above',
  threshold: 85,
  severity: 'critical',
  webhook_url: 'https://alerts.example.test/hook',
  enabled: true,
  has_secret: true,
  created_at: now,
  updated_at: now,
};

const activeAlert: AlertStateRecord = {
  rule_id: rule.id,
  node_id: node.node_id,
  active: true,
  last_value: 92,
  started_at: now,
  resolved_at: null,
  updated_at: now,
};

const delivery: AlertDeliveryRecord = {
  id: 'delivery-1',
  rule_id: rule.id,
  node_id: node.node_id,
  event: 'triggered',
  status: 'delivered',
  attempt_count: 1,
  http_status: 204,
  last_error: null,
  created_at: now,
  updated_at: now,
};

const health: HealthResponse = { status: 'ok', service: 'fusionpanel-api', version: '0.11.0-dev.1' };
const job: JobRecord = {
  id: 'job-1',
  kind: 'agent_operation',
  payload: {},
  state: 'running',
  worker_id: node.node_id,
  error_text: null,
  created_at: now,
  updated_at: now,
};

const setDashboardData = (options?: {
  nodes?: NodeRecord[];
  samples?: NodeMetricSample[];
  rules?: AlertRuleRecord[];
  active?: AlertStateRecord[];
  deliveries?: AlertDeliveryRecord[];
}) => {
  apiMock.getHealth.mockResolvedValue(health);
  apiMock.getCapabilities.mockResolvedValue({ capabilities: ['nodes', 'observability'] });
  apiMock.getNodes.mockResolvedValue(options?.nodes ?? [node]);
  apiMock.getJobs.mockResolvedValue([job]);
  apiMock.getLatestMetrics.mockResolvedValue(options?.samples ?? [sample]);
  apiMock.getNodeMetrics.mockResolvedValue(options?.samples ?? [sample]);
  apiMock.getActiveAlerts.mockResolvedValue(options?.active ?? []);
  apiMock.getAlertRules.mockResolvedValue(options?.rules ?? []);
  apiMock.getAlertDeliveries.mockResolvedValue(options?.deliveries ?? []);
  apiMock.deleteAlertRule.mockResolvedValue(undefined);
};

describe('Dashboard telemetry and alerts', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    setDashboardData();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('shows current telemetry, CPU history, and node/job health', async () => {
    setDashboardData({ rules: [rule], active: [activeAlert], deliveries: [delivery] });

    render(<Dashboard />);

    expect(await screen.findByText('92.0%')).toBeInTheDocument();
    expect(screen.getByText('Fresh')).toBeInTheDocument();
    expect(screen.getByText('50.0% (4 MiB / 8 MiB)')).toBeInTheDocument();
    expect(screen.getByText('42')).toBeInTheDocument();
    expect(screen.getByRole('img', { name: 'CPU history for edge-1' })).toBeInTheDocument();
    expect(screen.getByText('1 active alert')).toBeInTheDocument();
    expect(screen.getByText('Jobs running')).toBeInTheDocument();
    expect(screen.getAllByText('High CPU').length).toBeGreaterThan(0);
  });

  it('labels stale telemetry and nodes waiting for their first sample', async () => {
    const staleSample = { ...sample, recorded_at: new Date(Date.now() - 120_000).toISOString() };
    const secondNode = { ...node, node_id: 'node-2', name: 'edge-2', state: 'offline' as const };
    setDashboardData({ nodes: [node, secondNode], samples: [staleSample] });

    render(<Dashboard />);

    expect(await screen.findByText('Stale')).toBeInTheDocument();
    const secondNodeRow = screen.getByRole('row', { name: /edge-2/ });
    expect(within(secondNodeRow).getByText('Waiting for first sample')).toBeInTheDocument();
    expect(within(secondNodeRow).getByText('offline')).toBeInTheDocument();
  });

  it('creates a rule and clears the signing secret from the form', async () => {
    const signingSecret = 'unit-test-webhook-secret-with-32-bytes';
    apiMock.createAlertRule.mockResolvedValue(rule);
    apiMock.getAlertRules.mockResolvedValueOnce([]).mockResolvedValue([rule]);

    render(<Dashboard />);

    fireEvent.change(await screen.findByLabelText('Name'), { target: { value: 'High CPU' } });
    fireEvent.change(screen.getByLabelText('Webhook URL'), { target: { value: rule.webhook_url } });
    fireEvent.change(screen.getByLabelText(/Signing secret/), { target: { value: signingSecret } });
    fireEvent.click(screen.getByRole('button', { name: 'Create alert rule' }));

    await waitFor(() => expect(apiMock.createAlertRule).toHaveBeenCalledWith({
      name: 'High CPU',
      metric: 'cpu_usage_percent',
      operator: 'above',
      threshold: 85,
      severity: 'warning',
      webhook_url: rule.webhook_url,
      webhook_secret: signingSecret,
      enabled: true,
    }));
    expect(await screen.findByRole('status')).toHaveTextContent('stored encrypted');
    expect(screen.getByLabelText(/Signing secret/)).toHaveValue('');
    expect(screen.queryByText(signingSecret)).not.toBeInTheDocument();
  });

  it('deletes an alert rule from the operator list', async () => {
    setDashboardData({ rules: [rule], active: [activeAlert], deliveries: [delivery] });
    render(<Dashboard />);

    const deleteButton = await screen.findByRole('button', { name: 'Delete' });
    fireEvent.click(deleteButton);

    await waitFor(() => expect(apiMock.deleteAlertRule).toHaveBeenCalledWith(rule.id));
    expect(screen.queryByText('High CPU')).not.toBeInTheDocument();
    expect(screen.getByText('Alert rule deleted.')).toBeInTheDocument();
  });

  it('shows a clear authorization error and keeps telemetry unavailable', async () => {
    apiMock.getLatestMetrics.mockRejectedValue({
      response: { status: 403, data: { error: { message: 'Operator role required' } } },
    });

    render(<Dashboard />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Operator role required');
    expect(screen.getByText('No nodes registered yet.')).toBeInTheDocument();
  });
});
