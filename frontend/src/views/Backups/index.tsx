import React, { useCallback, useEffect, useState } from 'react';
import { api, DatabaseRecord, WebsiteRecord } from '../../api';
import { ArchiveList } from './ArchiveList';
import { BackupModal } from './BackupModal';
import { RestoreWizard } from './RestoreWizard';
import { BackupRecord, CreateBackupRequest, RestoreBackupRequest } from './types';

export const BackupsView: React.FC = () => {
  const [backups, setBackups] = useState<BackupRecord[]>([]);
  const [sites, setSites] = useState<WebsiteRecord[]>([]);
  const [databases, setDatabases] = useState<DatabaseRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isBackupModalOpen, setIsBackupModalOpen] = useState(false);
  const [selectedBackupForRestore, setSelectedBackupForRestore] = useState<BackupRecord | null>(null);

  const loadData = useCallback(async () => {
    try {
      const [backupList, siteList, dbList] = await Promise.all([
        api.getBackups(),
        api.getSites().catch(() => [] as WebsiteRecord[]),
        api.getDatabases().catch(() => [] as DatabaseRecord[]),
      ]);
      setBackups(Array.isArray(backupList) ? backupList : []);
      setSites(Array.isArray(siteList) ? siteList : []);
      setDatabases(Array.isArray(dbList) ? dbList : []);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load backups.');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const hasPendingOperation = backups.some(
    (backup) => backup.state === 'creating' || backup.state === 'restoring',
  );

  useEffect(() => {
    if (!hasPendingOperation) return;
    const timer = window.setInterval(() => {
      void api
        .getBackups()
        .then((next) => setBackups(Array.isArray(next) ? next : []))
        .catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(timer);
  }, [hasPendingOperation]);

  const handleCreateBackup = async (request: CreateBackupRequest) => {
    const response = await api.createBackup(request);
    setBackups((prev) => [response.backup, ...prev.filter((item) => item.id !== response.backup.id)]);
    await loadData();
  };

  const handleConfirmRestore = async (backupId: string, request: RestoreBackupRequest) => {
    const response = await api.restoreBackup(backupId, request);
    setBackups((prev) =>
      prev.map((item) => (item.id === response.backup.id ? response.backup : item)),
    );
    await loadData();
  };

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-xl font-semibold text-white">Backups & Snapshots</h2>
          <p className="text-sm text-slate-400">
            Create consistent site, database, or bundle snapshots to local or S3 storage and restore on demand.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setIsBackupModalOpen(true)}
          className="px-4 py-2 bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium rounded-md transition-colors"
        >
          Backup Now
        </button>
      </div>

      {error && (
        <div className="p-3 bg-red-950/60 border border-red-800 rounded text-sm text-red-300">
          {error}
        </div>
      )}

      <ArchiveList
        backups={backups}
        sites={sites}
        databases={databases}
        onRestoreClick={(backup) => setSelectedBackupForRestore(backup)}
        loading={loading}
      />

      <BackupModal
        isOpen={isBackupModalOpen}
        sites={sites}
        databases={databases}
        onClose={() => setIsBackupModalOpen(false)}
        onCreate={handleCreateBackup}
      />

      <RestoreWizard
        backup={selectedBackupForRestore}
        sites={sites}
        databases={databases}
        onClose={() => setSelectedBackupForRestore(null)}
        onConfirmRestore={handleConfirmRestore}
      />
    </div>
  );
};

export default BackupsView;
