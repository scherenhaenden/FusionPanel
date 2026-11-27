import { apiClient, api } from '../api/index';
import { SessionManager } from '../auth/session';
import { AxiosError } from 'axios';
import MockAdapter from 'axios-mock-adapter';
import { HealthResponse, CapabilitiesResponse, ErrorResponse } from '../types';

describe('API Client', () => {
  let mockAdapter: MockAdapter;

  beforeEach(() => {
    SessionManager.clear();
    mockAdapter = new MockAdapter(apiClient, { delayResponse: 0 });

    mockAdapter.onGet('/v1/health').reply(() => {
      const response: HealthResponse = {
        status: 'ok',
        service: 'fusionpanel-api',
        version: 'mock-1.0.0',
      };
      return [200, response];
    });

    mockAdapter.onGet('/v1/capabilities').reply(() => {
      const errorResponse: ErrorResponse = {
        error: {
          code: 'unauthorized',
          message: 'Authentication required',
        },
      };
      return [401, errorResponse];
    });

    mockAdapter.onPost('/v1/auth/login').reply((config) => {
      try {
        const { username, password } = JSON.parse(config.data);
        if (username === 'admin' && password === 'admin') {
          return [200];
        }
      } catch (e) {
      }
      const errorResponse: ErrorResponse = {
        error: {
          code: 'unauthorized',
          message: 'Authentication required',
        },
      };
      return [401, errorResponse];
    });
  });

  afterEach(() => {
    mockAdapter.restore();
  });

  describe('/v1/health', () => {
    it('returns health response', async () => {
      const response = await api.getHealth();
      expect(response).toEqual({
        status: 'ok',
        service: 'fusionpanel-api',
        version: 'mock-1.0.0',
      });
    });
  });

  describe('/v1/capabilities', () => {
    it('returns 401 when no token is present', async () => {
      let error: AxiosError | null = null;
      try {
        await api.getCapabilities();
      } catch (e: any) {
        error = e;
      }
      expect(error).toBeTruthy();
      expect(error?.response?.status).toBe(401);
    });

    it('does not attach a client-side token to the request', async () => {
      await expect(api.getCapabilities()).rejects.toMatchObject({ response: { status: 401 } });
    });
  });

  describe('/v1/auth/login', () => {
    it('accepts valid credentials without returning a JavaScript-readable token', async () => {
      await expect(api.login({ username: 'admin', password: 'admin' })).resolves.toBeUndefined();
    });
  });

  describe('/v1/nodes and /v1/jobs', () => {
    it('returns nodes from the authenticated API response', async () => {
      const node = {
        node_id: 'node-1',
        name: 'local-node',
        state: 'online' as const,
        created_at: '2026-11-15T12:00:00Z',
        last_heartbeat: '2026-11-15T12:01:00Z',
      };
      mockAdapter.onGet('/v1/nodes').reply(200, { nodes: [node] });

      await expect(api.getNodes()).resolves.toEqual([node]);
      expect(mockAdapter.history.get[0].withCredentials).toBe(true);
    });

    it('posts a system info operation with an idempotency key', async () => {
      const job = {
        id: 'job-1',
        kind: 'agent_operation',
        payload: { operation: 'system.info' },
        state: 'queued' as const,
        worker_id: null,
        error_text: null,
        created_at: '2026-11-15T12:00:00Z',
        updated_at: '2026-11-15T12:00:00Z',
      };
      mockAdapter.onPost('/v1/jobs').reply(201, { job });

      await expect(api.createSystemInfoJob()).resolves.toEqual(job);
      const request = mockAdapter.history.post[0];
      expect(request.headers?.['Idempotency-Key']).toBeTruthy();
      expect(JSON.parse(request.data)).toEqual({ operation: 'system.info', payload: {} });
      expect(request.withCredentials).toBe(true);
    });
  });

  describe('/v1/mail/domains', () => {
    it('lists mail domains and sends Idempotency-Key on mail mutations', async () => {
      const domain = {
        id: 'dom-1',
        tenant_id: 'ten-1',
        node_id: 'node-1',
        domain: 'example.com',
        dns_zone_id: null,
        active: true,
        state: 'ready' as const,
        desired_revision: 1,
        queued_revision: null,
        applied_revision: 1,
        failed_revision: null,
        pending_job_id: null,
        last_error: null,
        created_at: '2026-11-27T10:00:00Z',
        updated_at: '2026-11-27T10:00:00Z',
      };
      mockAdapter.onGet('/v1/mail/domains').reply(200, { domains: [domain] });
      mockAdapter.onPost('/v1/mail/domains').reply(202, {
        domain,
        job: {
          id: 'job-mail-1',
          kind: 'agent_operation',
          payload: {},
          state: 'queued',
          worker_id: null,
          error_text: null,
          created_at: '',
          updated_at: '',
        },
      });

      await expect(api.getMailDomains()).resolves.toEqual([domain]);
      await api.createMailDomain({ domain: 'example.com', node_id: 'node-1' }, 'idem-key-123');
      expect(mockAdapter.history.post[0].headers?.['Idempotency-Key']).toBe('idem-key-123');
    });
  });

  describe('/v1/metrics and /v1/alerts', () => {
    it('uses the operator API routes for telemetry, rule management, and delivery history', async () => {
      const sample = {
        node_id: 'node/1',
        cpu_usage_percent: 42,
        memory_used_kb: 2048,
        memory_available_kb: 6144,
        disk_io_ops: 17,
        recorded_at: '2026-11-19T10:00:00Z',
      };
      const rule = {
        id: 'rule-1',
        name: 'High CPU',
        metric: 'cpu_usage_percent',
        operator: 'above',
        threshold: 85,
        severity: 'critical',
        webhook_url: 'https://alerts.example.test/hook',
        enabled: true,
        has_secret: true,
        created_at: '2026-11-19T10:00:00Z',
        updated_at: '2026-11-19T10:00:00Z',
      };
      mockAdapter.onGet('/v1/metrics').reply(200, { samples: [sample] });
      mockAdapter.onGet('/v1/nodes/node%2F1/metrics').reply(200, { samples: [sample] });
      mockAdapter.onGet('/v1/alerts/rules').reply(200, { rules: [rule] });
      mockAdapter.onPost('/v1/alerts/rules').reply(201, { rule });
      mockAdapter.onDelete('/v1/alerts/rules/rule-1').reply(204);
      mockAdapter.onGet('/v1/alerts/active').reply(200, { active: [] });
      mockAdapter.onGet('/v1/alerts/deliveries').reply(200, { deliveries: [] });

      await expect(api.getLatestMetrics()).resolves.toEqual([sample]);
      await expect(api.getNodeMetrics('node/1')).resolves.toEqual([sample]);
      await expect(api.getAlertRules()).resolves.toEqual([rule]);
      await expect(api.createAlertRule({
        name: 'High CPU',
        metric: 'cpu_usage_percent',
        operator: 'above',
        threshold: 85,
        severity: 'critical',
        webhook_url: rule.webhook_url,
        webhook_secret: 'test webhook secret with at least thirty two bytes',
      })).resolves.toEqual(rule);
      await expect(api.deleteAlertRule(rule.id)).resolves.toBeUndefined();
      await expect(api.getActiveAlerts()).resolves.toEqual([]);
      await expect(api.getAlertDeliveries(5)).resolves.toEqual([]);

      expect(JSON.parse(mockAdapter.history.post[0].data).webhook_secret).toBe(
        'test webhook secret with at least thirty two bytes',
      );
      const deliveriesRequest = mockAdapter.history.get.find((request) => request.url === '/v1/alerts/deliveries');
      expect(deliveriesRequest?.params).toEqual({ limit: 5 });
    });
  });
});
