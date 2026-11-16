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
