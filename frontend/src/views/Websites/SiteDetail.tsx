import React from 'react';
import { Site } from './SiteList';

interface SiteDetailProps {
  site: Site | null;
  onClose: () => void;
}

export const SiteDetail: React.FC<SiteDetailProps> = ({ site, onClose }) => {
  if (!site) return null;

  return (
    <div className="site-detail">
      <button onClick={onClose} style={{ marginBottom: '1rem' }}>&larr; Back to List</button>
      <h2>Site Details: {site.domainName}</h2>

      <div className="detail-card">
        <div className="detail-row">
          <strong>Domain Name:</strong>
          <span>{site.domainName}</span>
        </div>
        <div className="detail-row">
          <strong>Status:</strong>
          <span>{site.isActive ? 'Active' : 'Inactive'}</span>
        </div>
        <div className="detail-row">
          <strong>SSL Enabled:</strong>
          <span>{site.isSslEnabled ? 'Yes' : 'No'}</span>
        </div>
        <div className="detail-row">
          <strong>PHP Version:</strong>
          <span>{site.phpVersion || 'N/A (Disabled)'}</span>
        </div>
        {/* Placeholder for more details like Document Root, Aliases, etc. */}
      </div>

      <style>{`
        .site-detail {
          padding: 1rem;
          border: 1px solid #ddd;
          border-radius: 8px;
          background: #f9f9f9;
        }
        .detail-row {
          display: flex;
          margin-bottom: 0.5rem;
        }
        .detail-row strong {
          width: 150px;
        }
      `}</style>
    </div>
  );
};
