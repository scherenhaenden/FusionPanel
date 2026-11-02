import React from 'react';
import { BackupArchive } from './types';

interface ArchiveListProps {
  archives: BackupArchive[];
  onRestoreClick: (archive: BackupArchive) => void;
}

export const ArchiveList: React.FC<ArchiveListProps> = ({ archives, onRestoreClick }) => {
  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="archive-list">
      <h3>Available Backups</h3>
      {archives.length === 0 ? (
        <p>No backups available.</p>
      ) : (
        <table className="table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Timestamp</th>
              <th>Size</th>
              <th>Scope</th>
              <th>Checksum Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {archives.map((archive) => (
              <tr key={archive.id}>
                <td>{archive.id}</td>
                <td>{new Date(archive.timestamp).toLocaleString()}</td>
                <td>{formatSize(archive.size)}</td>
                <td style={{ textTransform: 'capitalize' }}>{archive.scope}</td>
                <td>
                  <span className={`status-badge status-${archive.checksumStatus}`}>
                    {archive.checksumStatus.toUpperCase()}
                  </span>
                </td>
                <td>
                  <button
                    className="btn-secondary"
                    onClick={() => onRestoreClick(archive)}
                  >
                    Restore
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
};
