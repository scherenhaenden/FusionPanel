import React, { useState } from 'react';
import { isValidFQDN } from './utils';

export interface CreateSiteFormData {
  domainName: string;
  documentRoot: string;
  isPhpEnabled: boolean;
  isProxyEnabled: boolean;
}

interface CreateSiteModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (data: CreateSiteFormData) => void;
}

export const CreateSiteModal: React.FC<CreateSiteModalProps> = ({ isOpen, onClose, onSubmit }) => {
  const [domainName, setDomainName] = useState('');
  const [documentRoot, setDocumentRoot] = useState('');
  const [isPhpEnabled, setIsPhpEnabled] = useState(false);
  const [isProxyEnabled, setIsProxyEnabled] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!isValidFQDN(domainName)) {
      setError('Invalid Domain Name (FQDN)');
      return;
    }
    if (!documentRoot) {
        setError('Document Root is required');
        return;
    }

    setError(null);
    onSubmit({
      domainName,
      documentRoot,
      isPhpEnabled,
      isProxyEnabled,
    });
    setDomainName('');
    setDocumentRoot('');
    setIsPhpEnabled(false);
    setIsProxyEnabled(false);
  };

  return (
    <div className="modal">
      <div className="modal-content">
        <h2>Create New Site</h2>
        {error && <p className="error" style={{ color: 'red' }}>{error}</p>}
        <form onSubmit={handleSubmit}>
          <div>
            <label htmlFor="domainName">Domain Name (FQDN):</label>
            <input
              type="text"
              id="domainName"
              value={domainName}
              onChange={(e) => setDomainName(e.target.value)}
              required
            />
          </div>
          <div>
            <label htmlFor="documentRoot">Document Root:</label>
            <input
              type="text"
              id="documentRoot"
              value={documentRoot}
              onChange={(e) => setDocumentRoot(e.target.value)}
              required
            />
          </div>
          <div>
            <label>
              <input
                type="checkbox"
                checked={isPhpEnabled}
                onChange={(e) => setIsPhpEnabled(e.target.checked)}
              />
              Enable PHP
            </label>
          </div>
          <div>
             <label>
              <input
                type="checkbox"
                checked={isProxyEnabled}
                onChange={(e) => setIsProxyEnabled(e.target.checked)}
              />
              Enable Proxy
            </label>
          </div>
          <button type="submit">Create</button>
          <button type="button" onClick={onClose}>Cancel</button>
        </form>
      </div>
    </div>
  );
};
