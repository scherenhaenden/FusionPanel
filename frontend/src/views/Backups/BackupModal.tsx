import React, { useState } from 'react';
import { BackupScope } from './types';

interface BackupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onBackup: (scope: BackupScope) => void;
}

export const BackupModal: React.FC<BackupModalProps> = ({ isOpen, onClose, onBackup }) => {
  const [scope, setScope] = useState<BackupScope>('full');

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onBackup(scope);
  };

  return (
    <div className="modal-overlay">
      <div className="modal-content">
        <h2>Backup Now</h2>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Select Backup Target:</label>
            <div className="radio-group">
              <label>
                <input
                  type="radio"
                  name="scope"
                  value="files"
                  checked={scope === 'files'}
                  onChange={(e) => setScope(e.target.value as BackupScope)}
                />
                Files Only
              </label>
              <label>
                <input
                  type="radio"
                  name="scope"
                  value="databases"
                  checked={scope === 'databases'}
                  onChange={(e) => setScope(e.target.value as BackupScope)}
                />
                Databases Only
              </label>
              <label>
                <input
                  type="radio"
                  name="scope"
                  value="full"
                  checked={scope === 'full'}
                  onChange={(e) => setScope(e.target.value as BackupScope)}
                />
                Full Backup (Files & Databases)
              </label>
            </div>
          </div>
          <div className="modal-actions">
            <button type="button" onClick={onClose} className="btn-secondary">Cancel</button>
            <button type="submit" className="btn-primary">Start Backup</button>
          </div>
        </form>
      </div>
    </div>
  );
};
