import React from 'react';

export interface Site {
  id: string;
  domainName: string;
  isActive: boolean;
  isSslEnabled: boolean;
  phpVersion?: string; // e.g., '8.1', undefined if PHP is disabled
}

interface SiteListProps {
  sites: Site[];
  onSelectSite: (site: Site) => void;
}

export const SiteList: React.FC<SiteListProps> = ({ sites, onSelectSite }) => {
  return (
    <div className="site-list">
      <h2>Websites</h2>
      <table>
        <thead>
          <tr>
            <th>Domain</th>
            <th>Status</th>
            <th>SSL</th>
            <th>PHP Version</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {sites.map((site) => (
            <tr key={site.id}>
              <td>{site.domainName}</td>
              <td>
                <span className={`badge ${site.isActive ? 'badge-active' : 'badge-inactive'}`}>
                  {site.isActive ? 'Active' : 'Inactive'}
                </span>
              </td>
              <td>
                <span className={`badge ${site.isSslEnabled ? 'badge-success' : 'badge-warning'}`}>
                  {site.isSslEnabled ? 'Enabled' : 'Disabled'}
                </span>
              </td>
              <td>
                {site.phpVersion ? (
                  <span className="badge badge-info">{site.phpVersion}</span>
                ) : (
                  <span className="badge badge-secondary">N/A</span>
                )}
              </td>
              <td>
                <button onClick={() => onSelectSite(site)}>View Details</button>
              </td>
            </tr>
          ))}
          {sites.length === 0 && (
            <tr>
              <td colSpan={5} style={{ textAlign: 'center' }}>No websites found.</td>
            </tr>
          )}
        </tbody>
      </table>
      <style>{`
        .badge {
          padding: 0.25em 0.5em;
          border-radius: 4px;
          color: white;
          font-size: 0.85em;
        }
        .badge-active { background-color: #28a745; }
        .badge-inactive { background-color: #dc3545; }
        .badge-success { background-color: #28a745; }
        .badge-warning { background-color: #ffc107; color: #212529; }
        .badge-info { background-color: #17a2b8; }
        .badge-secondary { background-color: #6c757d; }
      `}</style>
    </div>
  );
};
