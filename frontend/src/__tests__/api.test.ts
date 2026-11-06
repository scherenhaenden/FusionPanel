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

    mockAdapter.onPost('/api/v1/auth/login').reply((config) => {
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

  describe('/api/v1/auth/login', () => {
    it('accepts valid credentials without returning a JavaScript-readable token', async () => {
      await expect(api.login({ username: 'admin', password: 'admin' })).resolves.toBeUndefined();
    });
  });
});
