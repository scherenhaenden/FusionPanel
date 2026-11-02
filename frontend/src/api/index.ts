import { apiClient } from './client';
import { HealthResponse, CapabilitiesResponse, AuthResponse } from '../types';

export const api = {
  getHealth: async (): Promise<HealthResponse> => {
    const response = await apiClient.get<HealthResponse>('/v1/health');
    return response.data;
  },

  getCapabilities: async (): Promise<CapabilitiesResponse> => {
    const response = await apiClient.get<CapabilitiesResponse>('/v1/capabilities');
    return response.data;
  },

  login: async (token: string): Promise<AuthResponse> => {
    // In actual implementation this might be a POST to /v1/auth
    // but the Rust backend currently just expects the admin token in the header.
    // For the sake of matching the prompt which asks to mirror /api/v1/auth
    // (the prompt specifically mentioned /health, /api/v1/capabilities, /api/v1/auth,
    // although backend uses /v1/health and /v1/capabilities).
    // Let's create an auth endpoint that validates the token or returns a mock.
    const response = await apiClient.post<AuthResponse>('/v1/auth', { token });
    return response.data;
  }
};

export * from './client';
export * from './mock';
