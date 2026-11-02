import { apiClient, api } from '../api/index';
import { setupMockAdapter } from '../api/mock';
import { SessionManager } from '../auth/session';
import { AxiosError } from 'axios';

describe('API Client', () => {
  let mockAdapter: ReturnType<typeof setupMockAdapter>;

  beforeEach(() => {
    // Clear the token before each test
    SessionManager.clearToken();
    // Setup mock adapter
    mockAdapter = setupMockAdapter(apiClient);
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
      // Assert TypeScript types conceptually by testing properties
      expect(typeof response.status).toBe('string');
      expect(typeof response.service).toBe('string');
      expect(typeof response.version).toBe('string');
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
      expect(error?.response?.data).toEqual({
        error: {
          code: 'unauthorized',
          message: 'Authentication required',
        },
      });
    });

    it('returns capabilities when valid token is present', async () => {
      SessionManager.setToken('mock-secret-token');

      const response = await api.getCapabilities();
      expect(response).toEqual({
        capabilities: ['control-plane.health.read', 'control-plane.capabilities.read'],
      });
      expect(Array.isArray(response.capabilities)).toBe(true);
      expect(typeof response.capabilities[0]).toBe('string');
    });

    it('returns 401 when invalid token is present', async () => {
      SessionManager.setToken('wrong-token');

      let error: AxiosError | null = null;
      try {
        await api.getCapabilities();
      } catch (e: any) {
        error = e;
      }
      expect(error).toBeTruthy();
      expect(error?.response?.status).toBe(401);
    });
  });

  describe('Auth Interceptor and 401 Handling', () => {
    it('clears token on 401 response', async () => {
      // Set an invalid token
      SessionManager.setToken('wrong-token');

      try {
        await api.getCapabilities();
      } catch (e) {
        // Ignored
      }

      // Token should be cleared by the 401 interceptor
      expect(SessionManager.getToken()).toBeNull();
    });

    it('dispatches unauthorized event on 401 response', async () => {
      const eventListener = jest.fn();
      window.addEventListener('unauthorized', eventListener);

      try {
        await api.getCapabilities(); // Will 401 because no token
      } catch (e) {
        // Ignored
      }

      expect(eventListener).toHaveBeenCalled();
      window.removeEventListener('unauthorized', eventListener);
    });
  });

  describe('/v1/auth', () => {
    it('returns token for valid credentials', async () => {
      const response = await api.login('mock-secret-token');
      expect(response.token).toBe('mock-secret-token');
    });
  });
});
