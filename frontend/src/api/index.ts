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

export type FileEntryKind = 'file' | 'dir';

export interface FileEntry {
  name: string;
  path: string;
  kind: FileEntryKind;
  size: number;
}

export interface FileReadResponse {
  path: string;
  content: string;
}

export interface FileWriteResponse {
  path: string;
  size: number;
}

export interface FileRenameResponse {
  from: string;
  to: string;
}

export type CronJobState = 'provisioning' | 'ready' | 'failed' | 'deleting';

export interface CronJobRecord {
  id: string;
  tenant_id: string;
  name: string;
  schedule: string;
  command: string;
  enabled: boolean;
  state: CronJobState;
  last_run_at: string | null;
  last_run_status: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateCronJobRequest {
  name: string;
  schedule: string;
  command: string;
  enabled?: boolean;
}

export interface CronOperationResponse {
  cron_job: CronJobRecord;
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

export type DatabaseEngine = 'postgresql' | 'mariadb';
export type DatabaseState = 'creating' | 'ready' | 'failed' | 'deleting';

export interface DatabaseRecord {
  id: string;
  tenant_id?: string;
  name: string;
  engine: DatabaseEngine;
  state: DatabaseState;
  last_error?: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface DatabaseCredentials {
  host: string;
  port: number;
  database: string;
  username: string;
  password: string;
}

export interface DatabaseOperationResponse {
  database: DatabaseRecord;
  job: JobRecord;
}

export interface DatabaseUserOperationResponse {
  user?: {
    id: string;
    database_id: string;
    username: string;
    state: string;
  };
  credentials: DatabaseCredentials;
  job?: JobRecord;
}

export type BackupState = 'creating' | 'ready' | 'restoring' | 'failed';
export type BackupTargetKind = 'site' | 'database' | 'bundle';
export type BackupDestinationKind = 'local' | 's3';

export type BackupTargetRequest =
  | { kind: 'site'; site_id: string }
  | { kind: 'database'; database_id: string }
  | { kind: 'bundle'; site_id: string; database_id: string };

export type BackupDestination =
  | { kind: 'local'; base_dir: string }
  | {
      kind: 's3';
      endpoint: string;
      bucket: string;
      region: string;
      access_key: string;
      secret_key: string;
      prefix?: string;
    };

export interface BackupRecord {
  id: string;
  tenant_id?: string;
  archive_key: string;
  target: BackupTargetRequest;
  destination_kind: BackupDestinationKind;
  destination_summary: string;
  state: BackupState;
  manifest_sha256: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateBackupRequest {
  target: BackupTargetRequest;
  destination: BackupDestination;
}

export interface RestoreBackupRequest {
  destination?: BackupDestination;
}

export interface BackupOperationResponse {
  backup: BackupRecord;
  job: JobRecord;
}

export type DnsZoneState = 'provisioning' | 'ready' | 'failed' | 'deleting';
export type DnsRecordType = 'A' | 'AAAA' | 'CNAME' | 'MX' | 'TXT';

export interface DnsZoneRecord {
  id: string;
  tenant_id?: string;
  domain: string;
  serial: number;
  default_ttl: number;
  state: DnsZoneState;
  last_error: string | null;
  last_probe_result: string | null;
  created_at: string;
  updated_at: string;
}

export interface DnsRecordEntry {
  id: string;
  zone_id: string;
  tenant_id?: string;
  name: string;
  record_type: DnsRecordType;
  value: string;
  ttl: number;
  priority: number | null;
  created_at: string;
  updated_at: string;
}

export interface DnsZoneWithRecordsResponse {
  zone: DnsZoneRecord;
  records: DnsRecordEntry[];
}

export interface DnsOperationResponse {
  zone: DnsZoneRecord;
  records?: DnsRecordEntry[];
  job: JobRecord;
}

export interface CreateDnsZoneRequest {
  domain: string;
  default_ttl?: number;
}

export interface UpsertDnsRecordRequest {
  name: string;
  record_type: DnsRecordType;
  value: string;
  ttl?: number;
  priority?: number | null;
}

export interface UpsertDkimRequest {
  selector: string;
  value: string;
  ttl?: number;
}

export interface ProbeDnsRequest {
  record_name: string;
  record_type: DnsRecordType;
  expected_substring?: string;
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
  getDatabases: async (): Promise<DatabaseRecord[]> => {
    const response = await apiClient.get<{ databases?: DatabaseRecord[] } | DatabaseRecord[]>('/v1/databases');
    return Array.isArray(response.data) ? response.data : (response.data.databases || []);
  },

  getDatabase: async (id: string): Promise<DatabaseRecord> => {
    const response = await apiClient.get<{ database: DatabaseRecord }>(`/v1/databases/${encodeURIComponent(id)}`);
    return response.data.database;
  },

  createDatabase: async (
    data: { name: string; engine: DatabaseEngine },
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DatabaseOperationResponse> => {
    const response = await apiClient.post<DatabaseOperationResponse>('/v1/databases', data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  createDatabaseUser: async (
    databaseId: string,
    data?: { username?: string },
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DatabaseCredentials> => {
    const response = await apiClient.post<DatabaseUserOperationResponse | DatabaseCredentials>(
      `/v1/databases/${encodeURIComponent(databaseId)}/users`,
      data || {},
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return 'credentials' in response.data ? response.data.credentials : response.data;
  },

  deleteDatabase: async (
    databaseId: string,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DatabaseOperationResponse> => {
    const response = await apiClient.delete<DatabaseOperationResponse>(
      `/v1/databases/${encodeURIComponent(databaseId)}`,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  // Backups
  getBackups: async (): Promise<BackupRecord[]> => {
    const response = await apiClient.get<{ backups: BackupRecord[] } | BackupRecord[]>('/v1/backups');
    return Array.isArray(response.data) ? response.data : response.data.backups;
  },

  getBackup: async (backupId: string): Promise<BackupRecord> => {
    const response = await apiClient.get<{ backup: BackupRecord } | BackupRecord>(
      `/v1/backups/${encodeURIComponent(backupId)}`,
    );
    return 'backup' in response.data ? response.data.backup : response.data;
  },

  createBackup: async (
    data: CreateBackupRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<BackupOperationResponse> => {
    const response = await apiClient.post<BackupOperationResponse>('/v1/backups', data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  restoreBackup: async (
    backupId: string,
    data: RestoreBackupRequest = {},
    idempotencyKey = createIdempotencyKey(),
  ): Promise<BackupOperationResponse> => {
    const response = await apiClient.post<BackupOperationResponse>(
      `/v1/backups/${encodeURIComponent(backupId)}/restore`,
      data,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  // DNS
  getDnsZones: async (): Promise<DnsZoneRecord[]> => {
    const response = await apiClient.get<{ zones: DnsZoneRecord[] } | DnsZoneRecord[]>(
      '/v1/dns/zones',
    );
    return Array.isArray(response.data) ? response.data : response.data.zones;
  },

  getDnsZone: async (zoneId: string): Promise<DnsZoneWithRecordsResponse> => {
    const response = await apiClient.get<DnsZoneWithRecordsResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}`,
    );
    return response.data;
  },

  createDnsZone: async (
    data: CreateDnsZoneRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.post<DnsOperationResponse>('/v1/dns/zones', data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  deleteDnsZone: async (
    zoneId: string,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.delete<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}`,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  createDnsRecord: async (
    zoneId: string,
    data: UpsertDnsRecordRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.post<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}/records`,
      data,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  updateDnsRecord: async (
    zoneId: string,
    recordId: string,
    data: UpsertDnsRecordRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.put<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}/records/${encodeURIComponent(recordId)}`,
      data,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  deleteDnsRecord: async (
    zoneId: string,
    recordId: string,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.delete<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}/records/${encodeURIComponent(recordId)}`,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  upsertDkimRecord: async (
    zoneId: string,
    data: UpsertDkimRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.post<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}/dkim`,
      data,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  probeDnsZone: async (
    zoneId: string,
    data: ProbeDnsRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<DnsOperationResponse> => {
    const response = await apiClient.post<DnsOperationResponse>(
      `/v1/dns/zones/${encodeURIComponent(zoneId)}/probe`,
      data,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  // Files
  listFiles: async (siteId: string, path = ''): Promise<FileEntry[]> => {
    const response = await apiClient.get<{ entries: FileEntry[] }>(
      `/v1/files/${encodeURIComponent(siteId)}/list`,
      { params: { path } },
    );
    return response.data.entries;
  },

  readFile: async (siteId: string, path: string): Promise<FileReadResponse> => {
    const response = await apiClient.get<FileReadResponse>(
      `/v1/files/${encodeURIComponent(siteId)}/read`,
      { params: { path } },
    );
    return response.data;
  },

  writeFile: async (
    siteId: string,
    path: string,
    content: string,
  ): Promise<FileWriteResponse> => {
    const response = await apiClient.put<FileWriteResponse>(
      `/v1/files/${encodeURIComponent(siteId)}/write`,
      { path, content },
    );
    return response.data;
  },

  renameFile: async (
    siteId: string,
    from: string,
    to: string,
  ): Promise<FileRenameResponse> => {
    const response = await apiClient.post<FileRenameResponse>(
      `/v1/files/${encodeURIComponent(siteId)}/rename`,
      { from, to },
    );
    return response.data;
  },

  deleteFile: async (siteId: string, path: string): Promise<void> => {
    await apiClient.delete(`/v1/files/${encodeURIComponent(siteId)}/delete`, {
      params: { path },
    });
  },

  // Cron Jobs
  getCronJobs: async (): Promise<CronJobRecord[]> => {
    const response = await apiClient.get<{ cron_jobs: CronJobRecord[] } | CronJobRecord[]>(
      '/v1/cron',
    );
    return Array.isArray(response.data) ? response.data : response.data.cron_jobs;
  },

  createCronJob: async (
    data: CreateCronJobRequest,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<CronOperationResponse> => {
    const response = await apiClient.post<CronOperationResponse>('/v1/cron', data, {
      headers: { 'Idempotency-Key': idempotencyKey },
    });
    return response.data;
  },

  deleteCronJob: async (
    cronId: string,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<CronOperationResponse> => {
    const response = await apiClient.delete<CronOperationResponse>(
      `/v1/cron/${encodeURIComponent(cronId)}`,
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
    return response.data;
  },

  runCronJobNow: async (
    cronId: string,
    idempotencyKey = createIdempotencyKey(),
  ): Promise<CronOperationResponse> => {
    const response = await apiClient.post<CronOperationResponse>(
      `/v1/cron/${encodeURIComponent(cronId)}/run`,
      {},
      { headers: { 'Idempotency-Key': idempotencyKey } },
    );
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
