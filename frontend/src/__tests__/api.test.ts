import { apiClient, api } from '../api/index';
import { SessionManager } from '../auth/session';
import { AxiosError } from 'axios';
import MockAdapter from 'axios-mock-adapter';
import { HealthResponse, CapabilitiesResponse, ErrorResponse, AuthResponse } from '../types';

describe('API Client', () => {
  let mockAdapter: MockAdapter;

  beforeEach(() => {
    SessionManager.clearToken();
    mockAdapter = new MockAdapter(apiClient, { delayResponse: 0 });

    mockAdapter.onGet('/v1/health').reply(() => {
      const response: HealthResponse = {
        status: 'ok',
        service: 'fusionpanel-api',
        version: 'mock-1.0.0',
      };
      return [200, response];
    });

    mockAdapter.onGet('/v1/capabilities').reply((config) => {
      const authHeader = config.headers?.Authorization;

      if (authHeader && authHeader === 'Bearer mock-secret-token') {
        const response: CapabilitiesResponse = {
          capabilities: ['control-plane.health.read', 'control-plane.capabilities.read'],
        };
        return [200, response];
      }

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
          const response: AuthResponse = { token: 'mock-secret-token' };
          return [200, response];
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

    it('returns capabilities when valid token is present', async () => {
      SessionManager.setToken('mock-secret-token');

      const response = await api.getCapabilities();
      expect(response).toEqual({
        capabilities: ['control-plane.health.read', 'control-plane.capabilities.read'],
      });
    });
  });

  describe('/v1/auth/login', () => {
    it('returns token for valid credentials', async () => {
      const response = await api.login({ username: 'admin', password: 'admin' });
      expect(response.token).toBe('mock-secret-token');
    });
  });
});
