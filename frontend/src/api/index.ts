import { apiClient } from './client';
import { HealthResponse, CapabilitiesResponse, JobLogEntry, JobRecord, NodeRecord } from '../types';

export type WebsiteKind = 'static' | 'reverse_proxy';
export type WebsiteState = 'provisioning' | 'active' | 'failed' | 'deleting';

export type CreateWebsiteRequest =
  | { domain: string; kind: 'static'; document_root: string }
  | { domain: string; kind: 'reverse_proxy'; upstream_url: string };

interface WebsiteRecordBase {
  id: string;
  domain: string;
  state: WebsiteState;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export type WebsiteRecord = WebsiteRecordBase &
  (
    | { kind: 'static'; document_root: string }
    | { kind: 'reverse_proxy'; upstream_url: string }
  );

export interface WebsiteOperationResponse {
  site: WebsiteRecord;
  job: JobRecord;
}

export type WebsiteTlsMode = 'custom_pem' | 'acme_http01';
export type WebsiteTlsState = 'disabled' | 'provisioning' | 'active' | 'failed' | 'deleting';

export interface WebsiteTlsRecord {
  site_id: string;
  tenant_id: string;
  mode: WebsiteTlsMode;
  state: WebsiteTlsState;
  acme_directory_url: string | null;
  contact_email: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export type ApplyWebsiteTlsRequest =
  | { mode: 'custom_pem'; certificate_pem: string; private_key_pem: string }
  | { mode: 'acme_http01'; acme_directory_url: string; contact_email?: string | null };

export interface WebsiteTlsOperationResponse {
  tls: WebsiteTlsRecord;
  job: JobRecord;
}

export const createIdempotencyKey = (): string => {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }

  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
};

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
  getSites: async (): Promise<WebsiteRecord[]> => {
    const response = await apiClient.get<{ websites: WebsiteRecord[] }>('/v1/websites');
    return response.data.websites;
  },

  getSite: async (id: string): Promise<WebsiteRecord> => {
    const response = await apiClient.get<{ website: WebsiteRecord }>(`/v1/websites/${encodeURIComponent(id)}`);
    return response.data.website;
  },

  createSite: async (data: CreateWebsiteRequest, idempotencyKey = createIdempotencyKey()): Promise<WebsiteOperationResponse> => {
    const response = await apiClient.post<WebsiteOperationResponse>('/v1/websites', data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  deleteSite: async (id: string, idempotencyKey = createIdempotencyKey()): Promise<WebsiteOperationResponse> => {
    const response = await apiClient.delete<WebsiteOperationResponse>(`/v1/websites/${encodeURIComponent(id)}`, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  getSiteTls: async (id: string): Promise<WebsiteTlsRecord | null> => {
    const response = await apiClient.get<{ tls: WebsiteTlsRecord | null }>(`/v1/websites/${encodeURIComponent(id)}/tls`);
    return response.data.tls;
  },

  applySiteTls: async (
    id: string,
    data: ApplyWebsiteTlsRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<WebsiteTlsOperationResponse> => {
    const response = await apiClient.post<WebsiteTlsOperationResponse>(`/v1/websites/${encodeURIComponent(id)}/tls`, data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  deleteSiteTls: async (id: string, idempotencyKey = createIdempotencyKey()): Promise<WebsiteTlsOperationResponse> => {
    const response = await apiClient.delete<WebsiteTlsOperationResponse>(`/v1/websites/${encodeURIComponent(id)}/tls`, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
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
