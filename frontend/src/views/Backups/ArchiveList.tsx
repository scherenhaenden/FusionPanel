import React from 'react';
import { DatabaseRecord, WebsiteRecord } from '../../api';
import { BackupRecord, BackupState } from './types';

interface ArchiveListProps {
  backups: BackupRecord[];
  sites: WebsiteRecord[];
  databases: DatabaseRecord[];
  onRestoreClick: (backup: BackupRecord) => void;
  loading?: boolean;
}

const STATE_BADGE_CLASSES: Record<BackupState, string> = {
  creating: 'bg-amber-950/60 text-amber-300 border-amber-800',
  ready: 'bg-emerald-950/60 text-emerald-300 border-emerald-800',
  restoring: 'bg-sky-950/60 text-sky-300 border-sky-800',
  failed: 'bg-red-950/60 text-red-300 border-red-800',
};

const formatTargetSummary = (
  backup: BackupRecord,
  sites: WebsiteRecord[],
  databases: DatabaseRecord[],
): string => {
  const { target } = backup;
  if (target.kind === 'site') {
    const site = sites.find((item) => item.id === target.site_id);
    return `Site: ${site ? site.domain : target.site_id}`;
  }
  if (target.kind === 'database') {
    const db = databases.find((item) => item.id === target.database_id);
    return `Database: ${db ? `${db.name} (${db.engine})` : target.database_id}`;
  }
  const site = sites.find((item) => item.id === target.site_id);
  const db = databases.find((item) => item.id === target.database_id);
  return `Bundle: ${site ? site.domain : target.site_id} + ${db ? db.name : target.database_id}`;
};

export const ArchiveList: React.FC<ArchiveListProps> = ({
  backups,
  sites,
  databases,
  onRestoreClick,
  loading,
}) => {
  if (loading) {
    return <div className="p-8 text-center text-slate-400">Loading backup archives...</div>;
  }

  if (backups.length === 0) {
    return (
      <div className="p-12 text-center bg-slate-900 border border-slate-800 rounded-lg">
        <p className="text-slate-400 mb-2">No backups found.</p>
        <p className="text-slate-500 text-sm">
          Create an on-demand backup of a site, database, or bundle to get started.
        </p>
      </div>
    );
  }

  return (
    <div className="bg-slate-900 border border-slate-800 rounded-lg overflow-hidden">
      <table className="w-full text-left border-collapse">
        <thead>
          <tr className="border-b border-slate-800 bg-slate-950/50 text-xs font-medium text-slate-400 uppercase tracking-wider">
            <th className="py-3 px-4">Archive</th>
            <th className="py-3 px-4">Target</th>
            <th className="py-3 px-4">Destination</th>
            <th className="py-3 px-4">State</th>
            <th className="py-3 px-4">Manifest SHA-256</th>
            <th className="py-3 px-4">Created</th>
            <th className="py-3 px-4 text-right">Actions</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-800/60 text-sm">
          {backups.map((backup) => (
            <tr key={backup.id} className="hover:bg-slate-800/30 transition-colors">
              <td className="py-3 px-4 font-mono text-xs text-slate-200">
                <div>{backup.archive_key}</div>
                <div className="text-[11px] text-slate-500">{backup.id}</div>
              </td>
              <td className="py-3 px-4 text-slate-200">
                {formatTargetSummary(backup, sites, databases)}
              </td>
              <td className="py-3 px-4">
                <div className="flex flex-col gap-0.5">
                  <span className="inline-flex w-fit items-center px-2 py-0.5 rounded text-xs font-medium bg-slate-800 text-slate-200 border border-slate-700 uppercase">
                    {backup.destination_kind}
                  </span>
                  <span className="text-xs text-slate-400 font-mono truncate max-w-xs" title={backup.destination_summary}>
                    {backup.destination_summary}
                  </span>
                </div>
              </td>
              <td className="py-3 px-4">
                <span
                  className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border ${STATE_BADGE_CLASSES[backup.state]}`}
                >
                  {backup.state}
                </span>
                {backup.last_error && (
                  <p className="text-xs text-red-400 mt-1 max-w-xs" title={backup.last_error}>
                    {backup.last_error}
                  </p>
                )}
              </td>
              <td className="py-3 px-4 font-mono text-xs text-slate-400">
                {backup.manifest_sha256 ? (
                  <span title={backup.manifest_sha256}>
                    {backup.manifest_sha256.slice(0, 12)}...
                  </span>
                ) : (
                  <span className="text-slate-600">—</span>
                )}
              </td>
              <td className="py-3 px-4 text-slate-400 text-xs">
                {new Date(backup.created_at).toLocaleString()}
              </td>
              <td className="py-3 px-4 text-right">
                <button
                  type="button"
                  disabled={backup.state !== 'ready'}
                  onClick={() => onRestoreClick(backup)}
                  className="text-xs font-medium px-3 py-1.5 rounded bg-slate-800 hover:bg-slate-700 disabled:opacity-40 disabled:cursor-not-allowed text-slate-200 border border-slate-700 transition-colors"
                >
                  Restore
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};
