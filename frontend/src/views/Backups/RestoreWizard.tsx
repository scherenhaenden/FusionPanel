import React, { useState } from 'react';
import { BackupArchive } from './types';

interface RestoreWizardProps {
  isOpen: boolean;
  archive: BackupArchive | null;
  onClose: () => void;
  onRestore: (archiveId: string) => void;
}

export const RestoreWizard: React.FC<RestoreWizardProps> = ({ isOpen, archive, onClose, onRestore }) => {
  const [step, setStep] = useState<1 | 2>(1);
  const [confirmText, setConfirmText] = useState('');

  if (!isOpen || !archive) return null;

  const handleNext = () => setStep(2);
  const handleBack = () => setStep(1);

  const handleRestore = () => {
    if (confirmText === 'RESTORE') {
      onRestore(archive.id);
      setStep(1);
      setConfirmText('');
    }
  };

  const handleClose = () => {
    setStep(1);
    setConfirmText('');
    onClose();
  };

  return (
    <div className="modal-overlay">
      <div className="modal-content">
        <h2>Restore Backup - Step {step} of 2</h2>

        {step === 1 && (
          <div className="wizard-step">
            <p>You are about to restore the following backup:</p>
            <ul>
              <li><strong>ID:</strong> {archive.id}</li>
              <li><strong>Date:</strong> {new Date(archive.timestamp).toLocaleString()}</li>
              <li><strong>Scope:</strong> {archive.scope}</li>
            </ul>

            <div className="warning-box">
              <strong>Warning:</strong> Restoring this backup will overwrite the current target directories and databases included in the scope. Any changes made since this backup will be permanently lost.
            </div>

            <div className="modal-actions">
              <button onClick={handleClose} className="btn-secondary">Cancel</button>
              <button onClick={handleNext} className="btn-primary">Next</button>
            </div>
          </div>
        )}

        {step === 2 && (
          <div className="wizard-step">
            <p>Please confirm your action.</p>
            <p>Type <strong>RESTORE</strong> below to proceed with the restoration.</p>

            <div className="form-group">
              <input
                type="text"
                value={confirmText}
                onChange={(e) => setConfirmText(e.target.value)}
                placeholder="Type RESTORE"
                className="confirm-input"
              />
            </div>

            <div className="modal-actions">
              <button onClick={handleBack} className="btn-secondary">Back</button>
              <button
                onClick={handleRestore}
                className="btn-danger"
                disabled={confirmText !== 'RESTORE'}
              >
                Confirm Restore
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
