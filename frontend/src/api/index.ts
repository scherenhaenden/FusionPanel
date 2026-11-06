import { apiClient } from './client';
import { HealthResponse, CapabilitiesResponse } from '../types';

export const api = {
  getHealth: async (): Promise<HealthResponse> => {
    const response = await apiClient.get<HealthResponse>('/v1/health');
    return response.data;
  },

  getCapabilities: async (): Promise<{ capabilities: string[]; unavailable?: string[] }> => {
    const response = await apiClient.get<{ capabilities: string[]; unavailable?: string[] }>('/v1/capabilities');
    return response.data;
  },

  login: async (payload: { username: string, password: string }): Promise<void> => {
    await apiClient.post('/v1/auth/login', payload);
  },

  logout: async (): Promise<void> => {
    await apiClient.post('/v1/auth/logout');
  },

  // Websites
  getSites: async (): Promise<any[]> => {
    const response = await apiClient.get<any[]>('/v1/websites');
    return response.data;
  },

  createSite: async (data: any): Promise<any> => {
    const response = await apiClient.post<any>('/v1/websites', data);
    return response.data;
  },

  // Databases
  getDatabases: async (): Promise<any[]> => {
    const response = await apiClient.get<any[]>('/v1/databases');
    return response.data;
  },

  createDatabase: async (data: any): Promise<any> => {
    const response = await apiClient.post<any>('/v1/databases', data);
    return response.data;
  },

  createDatabaseUser: async (dbName: string, data?: { username?: string }): Promise<any> => {
    const response = await apiClient.post<any>(`/v1/databases/${dbName}/users`, data || {});
    return response.data;
  },

  deleteDatabase: async (dbName: string): Promise<any> => {
    const response = await apiClient.delete<any>(`/v1/databases/${dbName}`);
    return response.data;
  },

  // Backups
  getBackups: async (): Promise<any[]> => {
    const response = await apiClient.get<any[]>('/v1/backups');
    return response.data;
  },

  createBackup: async (data: any): Promise<any> => {
    const response = await apiClient.post<any>('/v1/backups', data);
    return response.data;
  },

  restoreBackup: async (id: string): Promise<any> => {
    const response = await apiClient.post<any>(`/v1/backups/${id}/restore`);
    return response.data;
  },

  // Security
  getSecurityRules: async (): Promise<any[]> => {
    const response = await apiClient.get<any[]>('/v1/security/rules');
    return response.data;
  },

  addSecurityRule: async (data: any): Promise<any> => {
    const response = await apiClient.post<any>('/v1/security/rules', data);
    return response.data;
  }
};

export * from './client';
