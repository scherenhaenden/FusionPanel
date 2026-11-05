import React, { useState, useEffect } from 'react';
import { BackupArchive, BackupScope } from './types';
import { ArchiveList } from './ArchiveList';
import { BackupModal } from './BackupModal';
import { RestoreWizard } from './RestoreWizard';
import { api } from '../../api';
import './index.css';

export const BackupsView: React.FC = () => {
  const [archives, setArchives] = useState<BackupArchive[]>([]);
  const [isBackupModalOpen, setIsBackupModalOpen] = useState(false);
  const [selectedArchiveForRestore, setSelectedArchiveForRestore] = useState<BackupArchive | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [jobStatus, setJobStatus] = useState<string | null>(null);

  const loadBackups = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await api.getBackups();
      setArchives(data || []);
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to load backups');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadBackups();
  }, []);

  const handleBackup = async (scope: BackupScope) => {
    setError(null);
    setJobStatus('Starting backup job...');
    try {
      const newArchive = await api.createBackup({ scope });
      setJobStatus('Backup created successfully.');
      setArchives([newArchive, ...archives]);
      setIsBackupModalOpen(false);
      setTimeout(() => setJobStatus(null), 3000);
    } catch (e: any) {
      setJobStatus(null);
      setError(e?.response?.data?.error?.message || e.message || 'Failed to start backup');
    }
  };

  const handleRestore = async (archiveId: string) => {
    setError(null);
    setJobStatus('Starting restore job...');
    try {
      await api.restoreBackup(archiveId);
      console.log(`Restoring backup: ${archiveId}`);
      setSelectedArchiveForRestore(null);
      setJobStatus('Restore started successfully.');
      setTimeout(() => setJobStatus(null), 3000);
    } catch (e: any) {
      setJobStatus(null);
      setError(e?.response?.data?.error?.message || e.message || 'Failed to restore backup');
    }
  };

  return (
    <div className="backups-view">
      <div className="header-actions">
        <h2>Backup & Restore Management</h2>
        <button
          className="btn-primary"
          onClick={() => setIsBackupModalOpen(true)}
        >
          Backup Now
        </button>
      </div>

      {jobStatus && <div className="info-banner text-blue-600 mb-4">{jobStatus}</div>}
      {error && <div className="error-banner text-red-500 mb-4">{error} <button onClick={loadBackups} className="underline ml-2">Retry</button></div>}

      {isLoading ? (
        <div>Loading...</div>
      ) : (
        <ArchiveList
          archives={archives}
          onRestoreClick={(archive) => setSelectedArchiveForRestore(archive)}
        />
      )}

      <BackupModal
        isOpen={isBackupModalOpen}
        onClose={() => setIsBackupModalOpen(false)}
        onBackup={handleBackup}
      />

      <RestoreWizard
        isOpen={!!selectedArchiveForRestore}
        archive={selectedArchiveForRestore}
        onClose={() => setSelectedArchiveForRestore(null)}
        onRestore={handleRestore}
      />
    </div>
  );
};

export default BackupsView;
