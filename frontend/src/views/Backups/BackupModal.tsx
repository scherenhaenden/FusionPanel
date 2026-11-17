import React, { useEffect, useState } from 'react';
import { DatabaseRecord, WebsiteRecord } from '../../api';
import {
  BackupDestinationKind,
  BackupTargetKind,
  BackupTargetRequest,
  CreateBackupRequest,
} from './types';

interface BackupModalProps {
  isOpen: boolean;
  sites: WebsiteRecord[];
  databases: DatabaseRecord[];
  onClose: () => void;
  onCreate: (request: CreateBackupRequest) => Promise<void>;
}

const DEFAULT_LOCAL_BASE_DIR = '/var/backups/fusionpanel';

export const BackupModal: React.FC<BackupModalProps> = ({
  isOpen,
  sites,
  databases,
  onClose,
  onCreate,
}) => {
  const [targetKind, setTargetKind] = useState<BackupTargetKind>('site');
  const [siteId, setSiteId] = useState('');
  const [databaseId, setDatabaseId] = useState('');
  const [destinationKind, setDestinationKind] = useState<BackupDestinationKind>('local');
  const [baseDir, setBaseDir] = useState(DEFAULT_LOCAL_BASE_DIR);
  const [endpoint, setEndpoint] = useState('');
  const [bucket, setBucket] = useState('');
  const [region, setRegion] = useState('us-east-1');
  const [accessKey, setAccessKey] = useState('');
  const [secretKey, setSecretKey] = useState('');
  const [prefix, setPrefix] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    if (!siteId && sites.length > 0) {
      setSiteId(sites[0].id);
    }
    if (!databaseId && databases.length > 0) {
      setDatabaseId(databases[0].id);
    }
  }, [isOpen, sites, databases, siteId, databaseId]);

  if (!isOpen) return null;

  const needsSite = targetKind === 'site' || targetKind === 'bundle';
  const needsDatabase = targetKind === 'database' || targetKind === 'bundle';

  const handleClose = () => {
    setSecretKey('');
    setError(null);
    onClose();
  };

  const buildTarget = (): BackupTargetRequest | null => {
    if (targetKind === 'site') {
      return siteId ? { kind: 'site', site_id: siteId } : null;
    }
    if (targetKind === 'database') {
      return databaseId ? { kind: 'database', database_id: databaseId } : null;
    }
    if (siteId && databaseId) {
      return { kind: 'bundle', site_id: siteId, database_id: databaseId };
    }
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const target = buildTarget();
    if (!target) {
      setError('Select the required site and/or database target.');
      return;
    }

    if (destinationKind === 'local' && !baseDir.trim()) {
      setError('Local destination base directory is required.');
      return;
    }

    if (
      destinationKind === 's3' &&
      (!endpoint.trim() || !bucket.trim() || !region.trim() || !accessKey.trim() || !secretKey.trim())
    ) {
      setError('All required S3 fields (endpoint, bucket, region, access key, secret key) must be filled.');
      return;
    }

    const request: CreateBackupRequest = {
      target,
      destination:
        destinationKind === 'local'
          ? { kind: 'local', base_dir: baseDir.trim() }
          : {
              kind: 's3',
              endpoint: endpoint.trim(),
              bucket: bucket.trim(),
              region: region.trim(),
              access_key: accessKey.trim(),
              secret_key: secretKey,
              ...(prefix.trim() ? { prefix: prefix.trim() } : {}),
            },
    };

    setIsSubmitting(true);
    try {
      await onCreate(request);
      setSecretKey('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create backup.');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="backup-modal-title"
    >
      <div className="bg-slate-900 border border-slate-800 rounded-lg max-w-lg w-full p-6 shadow-xl max-h-[90vh] overflow-y-auto">
        <div className="flex justify-between items-center mb-4">
          <h3 id="backup-modal-title" className="text-lg font-medium text-white">
            Create Backup
          </h3>
          <button
            type="button"
            onClick={handleClose}
            className="text-slate-400 hover:text-slate-200"
            aria-label="Close modal"
          >
            ✕
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="backup-target-kind" className="block text-sm font-medium text-slate-300 mb-1">
              Target Scope
            </label>
            <select
              id="backup-target-kind"
              value={targetKind}
              onChange={(e) => setTargetKind(e.target.value as BackupTargetKind)}
              className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
            >
              <option value="site">Site (static document root)</option>
              <option value="database">Database (PostgreSQL / MariaDB dump)</option>
              <option value="bundle">Bundle (Site + Database)</option>
            </select>
          </div>

          {needsSite && (
            <div>
              <label htmlFor="backup-site-select" className="block text-sm font-medium text-slate-300 mb-1">
                Website
              </label>
              <select
                id="backup-site-select"
                value={siteId}
                onChange={(e) => setSiteId(e.target.value)}
                className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
              >
                <option value="">Select a site...</option>
                {sites.map((site) => (
                  <option key={site.id} value={site.id}>
                    {site.domain} ({site.kind})
                  </option>
                ))}
              </select>
            </div>
          )}

          {needsDatabase && (
            <div>
              <label htmlFor="backup-database-select" className="block text-sm font-medium text-slate-300 mb-1">
                Database
              </label>
              <select
                id="backup-database-select"
                value={databaseId}
                onChange={(e) => setDatabaseId(e.target.value)}
                className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
              >
                <option value="">Select a database...</option>
                {databases.map((db) => (
                  <option key={db.id} value={db.id}>
                    {db.name} ({db.engine})
                  </option>
                ))}
              </select>
            </div>
          )}

          <div>
            <label htmlFor="backup-destination-kind" className="block text-sm font-medium text-slate-300 mb-1">
              Destination
            </label>
            <select
              id="backup-destination-kind"
              value={destinationKind}
              onChange={(e) => setDestinationKind(e.target.value as BackupDestinationKind)}
              className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
            >
              <option value="local">Local Storage</option>
              <option value="s3">S3-Compatible Object Storage</option>
            </select>
          </div>

          {destinationKind === 'local' ? (
            <div>
              <label htmlFor="backup-local-base-dir" className="block text-sm font-medium text-slate-300 mb-1">
                Base Directory
              </label>
              <input
                id="backup-local-base-dir"
                type="text"
                value={baseDir}
                onChange={(e) => setBaseDir(e.target.value)}
                placeholder="/var/backups/fusionpanel"
                className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
              />
            </div>
          ) : (
            <div className="space-y-3 border border-slate-800 rounded p-3 bg-slate-950/50">
              <div>
                <label htmlFor="backup-s3-endpoint" className="block text-xs font-medium text-slate-300 mb-1">
                  S3 Endpoint
                </label>
                <input
                  id="backup-s3-endpoint"
                  type="text"
                  value={endpoint}
                  onChange={(e) => setEndpoint(e.target.value)}
                  placeholder="http://minio:9000"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label htmlFor="backup-s3-bucket" className="block text-xs font-medium text-slate-300 mb-1">
                    Bucket
                  </label>
                  <input
                    id="backup-s3-bucket"
                    type="text"
                    value={bucket}
                    onChange={(e) => setBucket(e.target.value)}
                    placeholder="fusionpanel-backups"
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
                <div>
                  <label htmlFor="backup-s3-region" className="block text-xs font-medium text-slate-300 mb-1">
                    Region
                  </label>
                  <input
                    id="backup-s3-region"
                    type="text"
                    value={region}
                    onChange={(e) => setRegion(e.target.value)}
                    placeholder="us-east-1"
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label htmlFor="backup-s3-access-key" className="block text-xs font-medium text-slate-300 mb-1">
                    Access Key
                  </label>
                  <input
                    id="backup-s3-access-key"
                    type="text"
                    value={accessKey}
                    onChange={(e) => setAccessKey(e.target.value)}
                    placeholder="minioadmin"
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
                <div>
                  <label htmlFor="backup-s3-secret-key" className="block text-xs font-medium text-slate-300 mb-1">
                    Secret Key
                  </label>
                  <input
                    id="backup-s3-secret-key"
                    type="password"
                    value={secretKey}
                    onChange={(e) => setSecretKey(e.target.value)}
                    placeholder="••••••••"
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
              </div>
              <div>
                <label htmlFor="backup-s3-prefix" className="block text-xs font-medium text-slate-300 mb-1">
                  Prefix (optional)
                </label>
                <input
                  id="backup-s3-prefix"
                  type="text"
                  value={prefix}
                  onChange={(e) => setPrefix(e.target.value)}
                  placeholder="prod/daily"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
            </div>
          )}

          {error && (
            <div className="p-3 bg-red-950/60 border border-red-800 rounded text-xs text-red-300">
              {error}
            </div>
          )}

          <div className="flex justify-end gap-3 pt-4 border-t border-slate-800">
            <button
              type="button"
              onClick={handleClose}
              className="px-4 py-2 text-sm text-slate-400 hover:text-slate-200"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="px-4 py-2 bg-sky-600 hover:bg-sky-500 disabled:opacity-50 text-white text-sm font-medium rounded"
            >
              {isSubmitting ? 'Starting Backup...' : 'Start Backup'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
