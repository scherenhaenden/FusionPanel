import React, { useEffect, useState } from 'react';
import {
  ApplyWebsiteTlsRequest,
  WebsiteRecord,
  WebsiteTlsMode,
  WebsiteTlsRecord,
  WebsiteTlsState,
} from '../../api';

interface SiteDetailProps {
  site: WebsiteRecord;
  tls?: WebsiteTlsRecord | null;
  isTlsLoading?: boolean;
  isTlsSubmitting?: boolean;
  tlsError?: string | null;
  onApplyTls?: (site: WebsiteRecord, request: ApplyWebsiteTlsRequest) => Promise<boolean>;
  onDeleteTls?: (site: WebsiteRecord) => Promise<void>;
  onClose: () => void;
}

const DEFAULT_ACME_DIRECTORY_URL = 'https://acme-v02.api.letsencrypt.org/directory';

const stateLabels: Record<WebsiteRecord['state'], string> = {
  provisioning: 'Provisioning',
  active: 'Active',
  failed: 'Failed',
  deleting: 'Deleting',
};

const tlsStateLabels: Record<WebsiteTlsState, string> = {
  disabled: 'Disabled',
  provisioning: 'Provisioning',
  active: 'Active',
  failed: 'Failed',
  deleting: 'Deleting',
};

const tlsModeLabels: Record<WebsiteTlsMode, string> = {
  acme_http01: 'ACME HTTP-01',
  custom_pem: 'Custom PEM',
};

export const SiteDetail: React.FC<SiteDetailProps> = ({
  site,
  tls = null,
  isTlsLoading = false,
  isTlsSubmitting = false,
  tlsError = null,
  onApplyTls,
  onDeleteTls,
  onClose,
}) => {
  const [mode, setMode] = useState<WebsiteTlsMode>(tls?.mode || 'acme_http01');
  const [acmeDirectoryUrl, setAcmeDirectoryUrl] = useState(
    tls?.acme_directory_url || DEFAULT_ACME_DIRECTORY_URL,
  );
  const [contactEmail, setContactEmail] = useState(tls?.contact_email || '');
  const [certificatePem, setCertificatePem] = useState('');
  const [privateKeyPem, setPrivateKeyPem] = useState('');
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    setMode(tls?.mode || 'acme_http01');
    setAcmeDirectoryUrl(tls?.acme_directory_url || DEFAULT_ACME_DIRECTORY_URL);
    setContactEmail(tls?.contact_email || '');
    setCertificatePem('');
    setPrivateKeyPem('');
    setValidationError(null);
  }, [site.id, tls?.mode, tls?.acme_directory_url, tls?.contact_email]);

  const tlsState: WebsiteTlsState = tls?.state || 'disabled';
  const isTlsBusy = isTlsLoading || isTlsSubmitting || tlsState === 'provisioning' || tlsState === 'deleting';
  const canConfigureTls = site.state === 'active' && !isTlsBusy;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!onApplyTls || !canConfigureTls) return;

    setValidationError(null);
    let payload: ApplyWebsiteTlsRequest;

    if (mode === 'acme_http01') {
      const directory = acmeDirectoryUrl.trim();
      if (!directory || (!directory.startsWith('https://') && !directory.startsWith('http://'))) {
        setValidationError('ACME directory URL must start with http:// or https://.');
        return;
      }
      const trimmedEmail = contactEmail.trim();
      payload = {
        mode: 'acme_http01',
        acme_directory_url: directory,
        ...(trimmedEmail ? { contact_email: trimmedEmail } : {}),
      };
    } else {
      const cert = certificatePem.trim();
      const key = privateKeyPem.trim();
      if (!cert.includes('-----BEGIN CERTIFICATE-----') || !key.includes('-----BEGIN')) {
        setValidationError('Provide both a valid PEM certificate and private key.');
        return;
      }
      payload = {
        mode: 'custom_pem',
        certificate_pem: cert,
        private_key_pem: key,
      };
    }

    const ok = await onApplyTls(site, payload);
    if (ok) {
      setCertificatePem('');
      setPrivateKeyPem('');
    }
  };

  return (
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
          <dd className="mt-1 text-gray-900 dark:text-white">
            {isTlsLoading ? 'Loading…' : `${tlsStateLabels[tlsState]}${tls && tlsState !== 'disabled' ? ` (${tlsModeLabels[tls.mode]})` : ''}`}
          </dd>
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

    <div className="mt-6 border-t border-gray-200 pt-5 dark:border-gray-700">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-base font-semibold text-gray-900 dark:text-white">TLS / SSL configuration</h3>
          <p className="text-sm text-gray-600 dark:text-gray-300">
            Configure HTTPS using ACME HTTP-01 or a custom PEM certificate and private key.
          </p>
        </div>
        {tls && tlsState !== 'disabled' && onDeleteTls && (
          <button
            type="button"
            onClick={() => void onDeleteTls(site)}
            disabled={isTlsBusy || site.state !== 'active'}
            className="rounded border border-red-300 px-3 py-1.5 text-sm font-medium text-red-700 hover:bg-red-50 disabled:opacity-50 dark:border-red-700 dark:text-red-300 dark:hover:bg-red-950"
          >
            Remove TLS
          </button>
        )}
      </div>

      {(tlsError || tls?.last_error || validationError) && (
        <div role="alert" className="mb-4 rounded border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          {validationError || tlsError || tls?.last_error}
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-4">
        <div className="flex flex-wrap gap-4">
          <label className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200">
            <input
              type="radio"
              name="tls-mode"
              value="acme_http01"
              checked={mode === 'acme_http01'}
              onChange={() => setMode('acme_http01')}
              disabled={!canConfigureTls}
            />
            ACME HTTP-01
          </label>
          <label className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200">
            <input
              type="radio"
              name="tls-mode"
              value="custom_pem"
              checked={mode === 'custom_pem'}
              onChange={() => setMode('custom_pem')}
              disabled={!canConfigureTls}
            />
            Custom PEM
          </label>
        </div>

        {mode === 'acme_http01' ? (
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label htmlFor="tls-acme-directory" className="block text-sm font-medium text-gray-700 dark:text-gray-200">
                ACME directory URL
              </label>
              <input
                id="tls-acme-directory"
                type="url"
                value={acmeDirectoryUrl}
                onChange={(event) => setAcmeDirectoryUrl(event.target.value)}
                disabled={!canConfigureTls}
                className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label htmlFor="tls-contact-email" className="block text-sm font-medium text-gray-700 dark:text-gray-200">
                Contact email (optional)
              </label>
              <input
                id="tls-contact-email"
                type="email"
                value={contactEmail}
                onChange={(event) => setContactEmail(event.target.value)}
                disabled={!canConfigureTls}
                placeholder="ops@example.com"
                className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
          </div>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2">
            <div>
              <label htmlFor="tls-certificate-pem" className="block text-sm font-medium text-gray-700 dark:text-gray-200">
                Certificate PEM
              </label>
              <textarea
                id="tls-certificate-pem"
                rows={5}
                value={certificatePem}
                onChange={(event) => setCertificatePem(event.target.value)}
                disabled={!canConfigureTls}
                placeholder="-----BEGIN CERTIFICATE-----"
                className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label htmlFor="tls-private-key-pem" className="block text-sm font-medium text-gray-700 dark:text-gray-200">
                Private key PEM
              </label>
              <textarea
                id="tls-private-key-pem"
                rows={5}
                value={privateKeyPem}
                onChange={(event) => setPrivateKeyPem(event.target.value)}
                disabled={!canConfigureTls}
                placeholder="-----BEGIN PRIVATE KEY-----"
                className="mt-1 w-full rounded border border-gray-300 bg-white px-3 py-2 font-mono text-xs text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
          </div>
        )}

        <div className="flex items-center gap-3">
          <button
            type="submit"
            disabled={!canConfigureTls || !onApplyTls}
            className="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
          >
            {isTlsSubmitting
              ? 'Applying TLS…'
              : tlsState === 'active'
                ? 'Update TLS'
                : 'Enable TLS'}
          </button>
          {site.state !== 'active' && (
            <span className="text-xs text-gray-500 dark:text-gray-400">
              Website must be active before configuring TLS.
            </span>
          )}
        </div>
      </form>
    </div>
  </section>
  );
};
