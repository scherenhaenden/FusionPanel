export type BackupScope = 'files' | 'databases' | 'full';

export type ChecksumStatus = 'valid' | 'invalid' | 'pending';

export interface BackupArchive {
  id: string;
  timestamp: string; // ISO format or localized string
  size: number; // Size in bytes
  checksumStatus: ChecksumStatus;
  scope: BackupScope;
}
