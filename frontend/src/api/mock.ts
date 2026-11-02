import MockAdapter from 'axios-mock-adapter';
import { AxiosInstance } from 'axios';
import { HealthResponse, CapabilitiesResponse, ErrorResponse, AuthResponse } from '../types';

export const setupMockAdapter = (client: AxiosInstance, delay = 0): MockAdapter => {
  const mock = new MockAdapter(client, { delayResponse: delay });

  // Mock /v1/health
  mock.onGet('/v1/health').reply(() => {
    const response: HealthResponse = {
      status: 'ok',
      service: 'fusionpanel-api',
      version: 'mock-1.0.0',
    };
    return [200, response];
  });

  // Mock /v1/capabilities
  mock.onGet('/v1/capabilities').reply((config) => {
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

  // Mock /v1/auth (assuming a basic mock auth implementation for frontend offline mode)
  mock.onPost('/v1/auth').reply((config) => {
    try {
      const { token } = JSON.parse(config.data);
      if (token === 'mock-secret-token') {
        const response: AuthResponse = { token: 'mock-secret-token' };
        return [200, response];
      }
    } catch (e) {
      // JSON parse error
    }
    const errorResponse: ErrorResponse = {
      error: {
        code: 'unauthorized',
        message: 'Authentication required',
      },
    };
    return [401, errorResponse];
  });

  return mock;
};
