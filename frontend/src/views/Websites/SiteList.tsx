import React, { useState } from 'react';
import { WebsiteRecord } from '../../api';

interface SiteListProps {
  sites: WebsiteRecord[];
  pendingSiteIds: string[];
  onSelectSite: (site: WebsiteRecord) => void;
  onDeleteSite: (site: WebsiteRecord) => void;
}

const siteStateLabel: Record<WebsiteRecord['state'], string> = {
  provisioning: 'Provisioning',
  active: 'Active',
  failed: 'Failed',
  deleting: 'Deleting',
};

export const SiteList: React.FC<SiteListProps> = ({ sites, pendingSiteIds, onSelectSite, onDeleteSite }) => {
  const [confirmingSiteId, setConfirmingSiteId] = useState<string | null>(null);

  if (sites.length === 0) {
    return <p className="py-8 text-center text-gray-600 dark:text-gray-300">No websites found.</p>;
  }

  return (
    <div className="overflow-x-auto rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800">
      <table className="min-w-full divide-y divide-gray-200 dark:divide-gray-700">
        <thead className="bg-gray-50 dark:bg-gray-900">
          <tr>
            <th scope="col" className="px-4 py-3 text-left text-sm font-semibold">Domain</th>
            <th scope="col" className="px-4 py-3 text-left text-sm font-semibold">Type</th>
            <th scope="col" className="px-4 py-3 text-left text-sm font-semibold">Configuration</th>
            <th scope="col" className="px-4 py-3 text-left text-sm font-semibold">State</th>
            <th scope="col" className="px-4 py-3 text-left text-sm font-semibold">Actions</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
          {sites.map((site) => {
            const isPending = pendingSiteIds.includes(site.id) || site.state === 'provisioning' || site.state === 'deleting';
            const isConfirmingDelete = confirmingSiteId === site.id;
            const configuration = site.kind === 'static'
              ? `Document root: ${site.document_root}`
              : `Upstream: ${site.upstream_url}`;

            return (
              <tr key={site.id}>
                <th scope="row" className="px-4 py-4 text-left font-medium text-gray-900 dark:text-white">{site.domain}</th>
                <td className="px-4 py-4 text-sm text-gray-700 dark:text-gray-200">
                  {site.kind === 'static' ? 'Static' : 'Reverse proxy'}
                </td>
                <td className="max-w-sm break-all px-4 py-4 text-sm text-gray-700 dark:text-gray-200">{configuration}</td>
                <td className="px-4 py-4 text-sm">
                  <span
                    role="status"
                    aria-label={`${site.domain} state: ${siteStateLabel[site.state]}`}
                    className={`rounded-full px-2.5 py-1 text-xs font-medium ${
                      site.state === 'active'
                        ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
                        : site.state === 'failed'
                          ? 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200'
                          : 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200'
                    }`}
                  >
                    {siteStateLabel[site.state]}
                  </span>
                  {site.last_error && (
                    <p className="mt-2 max-w-sm text-sm text-red-700 dark:text-red-300">{site.last_error}</p>
                  )}
                </td>
                <td className="px-4 py-4">
                  {isConfirmingDelete ? (
                    <div role="group" aria-label={`Confirm deletion of ${site.domain}`} className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => {
                          setConfirmingSiteId(null);
                          onDeleteSite(site);
                        }}
                        disabled={isPending}
                        className="rounded bg-red-700 px-3 py-2 text-sm font-medium text-white hover:bg-red-800 disabled:cursor-not-allowed disabled:opacity-50"
                      >
                        Confirm delete
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmingSiteId(null)}
                        className="rounded border border-gray-300 px-3 py-2 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <div className="flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => onSelectSite(site)}
                        className="rounded border border-gray-300 px-3 py-2 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
                      >
                        View details for {site.domain}
                      </button>
                      <button
                        type="button"
                        onClick={() => setConfirmingSiteId(site.id)}
                        disabled={isPending}
                        className="rounded border border-red-300 px-3 py-2 text-sm text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-950"
                      >
                        Delete {site.domain}
                      </button>
                    </div>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
