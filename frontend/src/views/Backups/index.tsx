import React, { useState } from 'react';
import { BackupArchive, BackupScope } from './types';
import { ArchiveList } from './ArchiveList';
import { BackupModal } from './BackupModal';
import { RestoreWizard } from './RestoreWizard';
import './index.css';

export const BackupsView: React.FC = () => {
  const [archives, setArchives] = useState<BackupArchive[]>([
    {
      id: 'bkp-1',
      timestamp: new Date(Date.now() - 86400000).toISOString(),
      size: 104857600,
      checksumStatus: 'valid',
      scope: 'full'
    },
    {
      id: 'bkp-2',
      timestamp: new Date(Date.now() - 172800000).toISOString(),
      size: 52428800,
      checksumStatus: 'pending',
      scope: 'databases'
    }
  ]);

  const [isBackupModalOpen, setIsBackupModalOpen] = useState(false);
  const [selectedArchiveForRestore, setSelectedArchiveForRestore] = useState<BackupArchive | null>(null);

  const handleBackup = (scope: BackupScope) => {
    // In a real app, this would make an API call to start the backup.
    const newArchive: BackupArchive = {
      id: `bkp-${Date.now()}`,
      timestamp: new Date().toISOString(),
      size: 0,
      checksumStatus: 'pending',
      scope
    };
    setArchives([newArchive, ...archives]);
    setIsBackupModalOpen(false);
  };

  const handleRestore = (archiveId: string) => {
    // In a real app, this would make an API call to restore the backup.
    console.log(`Restoring backup: ${archiveId}`);
    setSelectedArchiveForRestore(null);
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

      <ArchiveList
        archives={archives}
        onRestoreClick={(archive) => setSelectedArchiveForRestore(archive)}
      />

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
