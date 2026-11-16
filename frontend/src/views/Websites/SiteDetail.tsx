import React from 'react';
import { WebsiteRecord } from '../../api';

interface SiteDetailProps {
  site: WebsiteRecord;
  onClose: () => void;
}

const stateLabels: Record<WebsiteRecord['state'], string> = {
  provisioning: 'Provisioning',
  active: 'Active',
  failed: 'Failed',
  deleting: 'Deleting',
};

export const SiteDetail: React.FC<SiteDetailProps> = ({ site, onClose }) => (
  <section aria-labelledby="site-detail-heading" className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
    <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 id="site-detail-heading" className="text-xl font-semibold text-gray-900 dark:text-white">{site.domain}</h2>
        <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">Website details</p>
      </div>
      <button
        type="button"
        onClick={onClose}
        className="rounded border border-gray-300 px-3 py-2 text-sm hover:bg-gray-50 dark:border-gray-600 dark:hover:bg-gray-700"
      >
        Back to websites
      </button>
    </div>

    <dl className="grid gap-4 sm:grid-cols-2">
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Domain</dt>
        <dd className="mt-1 break-all text-gray-900 dark:text-white">{site.domain}</dd>
      </div>
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Type</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">{site.kind === 'static' ? 'Static site' : 'Reverse proxy'}</dd>
      </div>
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">State</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">{stateLabels[site.state]}</dd>
      </div>
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">TLS / SSL</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">Deferred to Sprint 4</dd>
      </div>
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">PHP-FPM</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">Deferred</dd>
      </div>
      {site.kind === 'static' ? (
        <div>
          <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Document root</dt>
          <dd className="mt-1 break-all text-gray-900 dark:text-white">{site.document_root}</dd>
        </div>
      ) : (
        <div>
          <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Upstream URL</dt>
          <dd className="mt-1 break-all text-gray-900 dark:text-white">{site.upstream_url}</dd>
        </div>
      )}
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Created</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">{site.created_at}</dd>
      </div>
      <div>
        <dt className="text-sm font-medium text-gray-600 dark:text-gray-300">Last updated</dt>
        <dd className="mt-1 text-gray-900 dark:text-white">{site.updated_at}</dd>
      </div>
    </dl>

    {site.last_error && (
      <div role="alert" className="mt-5 rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
        {site.last_error}
      </div>
    )}
  </section>
);
