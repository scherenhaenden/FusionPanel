import { apiClient } from './client';
import { HealthResponse, CapabilitiesResponse, JobLogEntry, JobRecord, NodeRecord } from '../types';

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

  getMe: async (): Promise<{ user_id: string; tenant_id: string; roles: string[] }> => {
    const response = await apiClient.get<{ user_id: string; tenant_id: string; roles: string[] }>('/v1/auth/me');
    return response.data;
  },

  logout: async (): Promise<void> => {
    await apiClient.post('/v1/auth/logout');
  },

  // Nodes and jobs
  getNodes: async (): Promise<NodeRecord[]> => {
    const response = await apiClient.get<{ nodes: NodeRecord[] }>('/v1/nodes');
    return response.data.nodes;
  },

  getJobs: async (): Promise<JobRecord[]> => {
    const response = await apiClient.get<{ jobs: JobRecord[] }>('/v1/jobs');
    return response.data.jobs;
  },

  getJob: async (id: string): Promise<JobRecord> => {
    const response = await apiClient.get<{ job: JobRecord }>(`/v1/jobs/${encodeURIComponent(id)}`);
    return response.data.job;
  },

  getJobLogs: async (id: string): Promise<JobLogEntry[]> => {
    const response = await apiClient.get<{ logs: JobLogEntry[] }>(`/v1/jobs/${encodeURIComponent(id)}/logs`);
    return response.data.logs;
  },

  createSystemInfoJob: async (): Promise<JobRecord> => {
    const idempotencyKey = typeof globalThis.crypto?.randomUUID === 'function'
      ? globalThis.crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const response = await apiClient.post<{ job: JobRecord }>(
      '/v1/jobs',
      { operation: 'system.info', payload: {} },
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data.job;
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
