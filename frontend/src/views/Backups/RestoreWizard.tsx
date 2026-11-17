import React, { useEffect, useState } from 'react';
import { DatabaseRecord, WebsiteRecord } from '../../api';
import { BackupRecord, RestoreBackupRequest } from './types';

interface RestoreWizardProps {
  backup: BackupRecord | null;
  sites: WebsiteRecord[];
  databases: DatabaseRecord[];
  onClose: () => void;
  onConfirmRestore: (backupId: string, request: RestoreBackupRequest) => Promise<void>;
}

const formatTarget = (
  backup: BackupRecord,
  sites: WebsiteRecord[],
  databases: DatabaseRecord[],
): string => {
  const { target } = backup;
  if (target.kind === 'site') {
    const site = sites.find((s) => s.id === target.site_id);
    return `site ${site ? site.domain : target.site_id}`;
  }
  if (target.kind === 'database') {
    const db = databases.find((d) => d.id === target.database_id);
    return `database ${db ? db.name : target.database_id}`;
  }
  const site = sites.find((s) => s.id === target.site_id);
  const db = databases.find((d) => d.id === target.database_id);
  return `bundle (${site ? site.domain : target.site_id} + ${db ? db.name : target.database_id})`;
};

export const RestoreWizard: React.FC<RestoreWizardProps> = ({
  backup,
  sites,
  databases,
  onClose,
  onConfirmRestore,
}) => {
  const [confirmText, setConfirmText] = useState('');
  const [endpoint, setEndpoint] = useState('');
  const [bucket, setBucket] = useState('');
  const [region, setRegion] = useState('us-east-1');
  const [accessKey, setAccessKey] = useState('');
  const [secretKey, setSecretKey] = useState('');
  const [prefix, setPrefix] = useState('');
  const [isRestoring, setIsRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setConfirmText('');
    setSecretKey('');
    setError(null);
    if (backup?.destination_kind === 's3') {
      const stripped = backup.destination_summary.replace(/^s3:\/\//, '');
      const [parsedBucket, ...rest] = stripped.split('/');
      if (parsedBucket) setBucket(parsedBucket);
      if (rest.length > 0) setPrefix(rest.join('/'));
    }
  }, [backup]);

  if (!backup) return null;

  const isS3 = backup.destination_kind === 's3';
  const s3Valid =
    !isS3 ||
    Boolean(
      endpoint.trim() && bucket.trim() && region.trim() && accessKey.trim() && secretKey.trim(),
    );

  const handleClose = () => {
    setSecretKey('');
    setConfirmText('');
    setError(null);
    onClose();
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (confirmText !== 'RESTORE' || !s3Valid) return;

    setError(null);
    setIsRestoring(true);
    try {
      const payload: RestoreBackupRequest = isS3
        ? {
            destination: {
              kind: 's3',
              endpoint: endpoint.trim(),
              bucket: bucket.trim(),
              region: region.trim(),
              access_key: accessKey.trim(),
              secret_key: secretKey,
              ...(prefix.trim() ? { prefix: prefix.trim() } : {}),
            },
          }
        : {};
      await onConfirmRestore(backup.id, payload);
      setSecretKey('');
      setConfirmText('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start restore.');
    } finally {
      setIsRestoring(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="restore-wizard-title"
    >
      <div className="bg-slate-900 border border-slate-800 rounded-lg max-w-lg w-full p-6 shadow-xl max-h-[90vh] overflow-y-auto">
        <div className="flex justify-between items-center mb-4">
          <h3 id="restore-wizard-title" className="text-lg font-medium text-amber-400">
            Restore Backup Archive
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
          <div className="p-3 bg-amber-950/30 border border-amber-900/50 rounded text-sm text-amber-200/90 space-y-1">
            <p>
              <strong>Warning:</strong> Restoring{' '}
              <span className="font-mono text-amber-100">{backup.archive_key}</span> will overwrite
              the live {formatTarget(backup, sites, databases)} after SHA-256 verification.
            </p>
            <p className="text-xs text-slate-300 font-mono">
              Destination: {backup.destination_summary}
            </p>
            {backup.manifest_sha256 && (
              <p className="text-xs text-slate-300 font-mono truncate">
                Manifest SHA-256: {backup.manifest_sha256}
              </p>
            )}
          </div>

          {isS3 && (
            <div className="space-y-3 border border-slate-800 rounded p-3 bg-slate-950/50">
              <p className="text-xs text-slate-300">
                Enter S3 credentials to download and verify this archive:
              </p>
              <div>
                <label htmlFor="restore-s3-endpoint" className="block text-xs font-medium text-slate-300 mb-1">
                  S3 Endpoint
                </label>
                <input
                  id="restore-s3-endpoint"
                  type="text"
                  value={endpoint}
                  onChange={(e) => setEndpoint(e.target.value)}
                  placeholder="http://minio:9000"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label htmlFor="restore-s3-bucket" className="block text-xs font-medium text-slate-300 mb-1">
                    Bucket
                  </label>
                  <input
                    id="restore-s3-bucket"
                    type="text"
                    value={bucket}
                    onChange={(e) => setBucket(e.target.value)}
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
                <div>
                  <label htmlFor="restore-s3-region" className="block text-xs font-medium text-slate-300 mb-1">
                    Region
                  </label>
                  <input
                    id="restore-s3-region"
                    type="text"
                    value={region}
                    onChange={(e) => setRegion(e.target.value)}
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label htmlFor="restore-s3-access-key" className="block text-xs font-medium text-slate-300 mb-1">
                    Access Key
                  </label>
                  <input
                    id="restore-s3-access-key"
                    type="text"
                    value={accessKey}
                    onChange={(e) => setAccessKey(e.target.value)}
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
                <div>
                  <label htmlFor="restore-s3-secret-key" className="block text-xs font-medium text-slate-300 mb-1">
                    Secret Key
                  </label>
                  <input
                    id="restore-s3-secret-key"
                    type="password"
                    value={secretKey}
                    onChange={(e) => setSecretKey(e.target.value)}
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                </div>
              </div>
              <div>
                <label htmlFor="restore-s3-prefix" className="block text-xs font-medium text-slate-300 mb-1">
                  Prefix (optional)
                </label>
                <input
                  id="restore-s3-prefix"
                  type="text"
                  value={prefix}
                  onChange={(e) => setPrefix(e.target.value)}
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
            </div>
          )}

          <div>
            <label htmlFor="restore-confirm-input" className="block text-sm font-medium text-slate-300 mb-2">
              Type <span className="font-mono text-white bg-slate-800 px-1.5 py-0.5 rounded">RESTORE</span> to confirm
            </label>
            <input
              id="restore-confirm-input"
              type="text"
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder="RESTORE"
              className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm focus:outline-none focus:border-amber-500 font-mono"
            />
          </div>

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
              disabled={confirmText !== 'RESTORE' || !s3Valid || isRestoring}
              className="px-4 py-2 bg-amber-600 hover:bg-amber-500 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium rounded"
            >
              {isRestoring ? 'Restoring...' : 'Confirm Restore'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
