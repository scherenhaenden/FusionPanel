export interface HealthResponse {
  status: string;
  service: string;
  version: string;
}

export interface CapabilitiesResponse {
  capabilities: string[];
}

export interface ErrorDetail {
  code: string;
  message: string;
}

export interface ErrorResponse {
  error: ErrorDetail;
}

export interface AuthResponse {
  token: string;
}

export interface Operator {
  id: string;
  email: string;
  role: string;
  status: string;
}

export interface AuditLog {
  id: string;
  timestamp: string;
  action: string;
  operator: string;
  details: string;
}

export type NodeState = 'online' | 'offline';

export interface NodeRecord {
  node_id: string;
  name: string;
  state: NodeState;
  created_at: string;
  last_heartbeat: string;
}

export interface NodeMetricSample {
  node_id: string;
  cpu_usage_percent: number;
  memory_used_kb: number;
  memory_available_kb: number;
  disk_io_ops: number;
  recorded_at: string;
}

export type AlertMetric = 'cpu_usage_percent' | 'memory_used_percent' | 'disk_io_ops';
export type AlertOperator = 'above' | 'below';
export type AlertSeverity = 'info' | 'warning' | 'critical';

export interface AlertRuleRecord {
  id: string;
  name: string;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  severity: AlertSeverity;
  webhook_url: string;
  enabled: boolean;
  has_secret: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateAlertRuleRequest {
  name: string;
  metric: AlertMetric;
  operator: AlertOperator;
  threshold: number;
  severity: AlertSeverity;
  webhook_url: string;
  webhook_secret: string;
  enabled?: boolean;
}

export interface AlertStateRecord {
  rule_id: string;
  node_id: string;
  active: boolean;
  last_value: number;
  started_at: string | null;
  resolved_at: string | null;
  updated_at: string;
}

export interface AlertDeliveryRecord {
  id: string;
  rule_id: string;
  node_id: string;
  event: 'triggered' | 'resolved' | string;
  status: 'pending' | 'delivered' | 'failed' | string;
  attempt_count: number;
  http_status: number | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export type JobState = 'queued' | 'running' | 'succeeded' | 'failed' | 'cancelled';

export interface JobRecord {
  id: string;
  kind: string;
  payload: Record<string, unknown>;
  state: JobState;
  worker_id: string | null;
  error_text: string | null;
  created_at: string;
  updated_at: string;
}

export interface JobLogEntry {
  sequence: number;
  stream: 'stdout' | 'stderr' | 'system' | string;
  text: string;
  created_at: string;
}

export type MailResourceState = 'provisioning' | 'ready' | 'failed' | 'deleting';

export interface MailDomainRecord {
  id: string;
  tenant_id: string;
  node_id: string;
  domain: string;
  dns_zone_id: string | null;
  active: boolean;
  state: MailResourceState;
  desired_revision: number;
  queued_revision: number | null;
  applied_revision: number | null;
  failed_revision: number | null;
  pending_job_id: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface MailboxRecord {
  id: string;
  domain_id: string;
  tenant_id: string;
  address: string;
  local_part: string;
  quota_bytes: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface MailAliasRecord {
  id: string;
  domain_id: string;
  tenant_id: string;
  source: string;
  destinations: string[];
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface MailDkimRecord {
  domain_id: string;
  tenant_id: string;
  selector: string;
  enabled: boolean;
  dns_zone_id: string | null;
  txt_record: string | null;
  public_key_pem: string | null;
  state: MailResourceState;
  queued_revision: number | null;
  applied_revision: number | null;
  failed_revision: number | null;
  pending_job_id: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface MailDomainDetailsResponse {
  domain: MailDomainRecord;
  mailboxes: MailboxRecord[];
  aliases: MailAliasRecord[];
  dkim: MailDkimRecord | null;
}

export interface CreateMailDomainRequest {
  domain: string;
  node_id: string;
  dns_zone_id?: string | null;
  active?: boolean;
}

export interface UpdateMailDomainRequest {
  node_id?: string;
  dns_zone_id?: string | null;
  active?: boolean;
}

export interface CreateMailboxRequest {
  address?: string;
  local_part?: string;
  password: string;
  quota_bytes?: number;
  active?: boolean;
}

export interface UpdateMailboxRequest {
  password?: string;
  quota_bytes?: number;
  active?: boolean;
}

export interface CreateMailAliasRequest {
  source?: string;
  local_part?: string;
  destinations: string[];
  active?: boolean;
}

export interface UpdateMailAliasRequest {
  destinations?: string[];
  active?: boolean;
}

export interface UpsertMailDkimRequest {
  selector: string;
  enabled?: boolean;
  dns_zone_id?: string | null;
  rotate?: boolean;
}

export interface MailDomainOperationResponse {
  domain: MailDomainRecord;
  job: JobRecord;
}

export interface MailboxOperationResponse {
  mailbox?: MailboxRecord | null;
  domain: MailDomainRecord;
  job: JobRecord;
}

export interface MailAliasOperationResponse {
  alias?: MailAliasRecord | null;
  domain: MailDomainRecord;
  job: JobRecord;
}

export interface MailDkimOperationResponse {
  dkim: MailDkimRecord | null;
  domain: MailDomainRecord;
  job: JobRecord;
}
