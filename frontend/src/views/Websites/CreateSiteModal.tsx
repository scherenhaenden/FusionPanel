import React, { useEffect, useRef, useState } from 'react';
import { CreateWebsiteRequest } from '../../api';
import { isValidFQDN, validateDocumentRoot, validateUpstreamUrl } from './utils';

interface CreateSiteModalProps {
  isOpen: boolean;
  isSubmitting: boolean;
  submitError: string | null;
  onClose: () => void;
  onSubmit: (data: CreateWebsiteRequest) => Promise<void>;
}

type WebsiteKind = CreateWebsiteRequest['kind'];

export const CreateSiteModal: React.FC<CreateSiteModalProps> = ({
  isOpen,
  isSubmitting,
  submitError,
  onClose,
  onSubmit,
}) => {
  const [domain, setDomain] = useState('');
  const [kind, setKind] = useState<WebsiteKind>('static');
  const [documentRoot, setDocumentRoot] = useState('');
  const [upstreamUrl, setUpstreamUrl] = useState('');
  const [validationError, setValidationError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(onClose);
  const isSubmittingRef = useRef(isSubmitting);

  useEffect(() => { onCloseRef.current = onClose; }, [onClose]);
  useEffect(() => { isSubmittingRef.current = isSubmitting; }, [isSubmitting]);

  useEffect(() => {
    if (!isOpen) {
      previousFocusRef.current?.focus();
      previousFocusRef.current = null;
      setDomain('');
      setKind('static');
      setDocumentRoot('');
      setUpstreamUrl('');
      setValidationError(null);
      return undefined;
    }

    if (!previousFocusRef.current && document.activeElement instanceof HTMLElement) {
      previousFocusRef.current = document.activeElement;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isSubmittingRef.current) {
        event.preventDefault();
        onCloseRef.current();
      }

      if (event.key !== 'Tab') return;
      const focusable = dialogRef.current?.querySelectorAll<HTMLElement>(
        'input:not(:disabled), button:not(:disabled)',
      );
      if (!focusable?.length) {
        event.preventDefault();
        dialogRef.current?.focus();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    dialogRef.current?.querySelector<HTMLElement>('input:not(:disabled)')?.focus();
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen]);

  if (!isOpen) return null;

  const handleDomainChange = (value: string) => {
    const previousDomain = domain.trim().toLowerCase();
    const previousDefaultRoot = previousDomain ? `/srv/fusionpanel/sites/${previousDomain}` : '';
    const nextDomain = value.trim().toLowerCase();
    setDomain(value);
    if (!documentRoot || documentRoot === previousDefaultRoot) {
      setDocumentRoot(nextDomain ? `/srv/fusionpanel/sites/${nextDomain}` : '');
    }
    setValidationError(null);
  };

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const canonicalDomain = domain.trim().toLowerCase();
    if (!isValidFQDN(canonicalDomain)) {
      setValidationError('Enter a valid ASCII domain name, such as app.local.');
      return;
    }

    if (kind === 'static') {
      const pathError = validateDocumentRoot(documentRoot, canonicalDomain);
      if (pathError) {
        setValidationError(pathError);
        return;
      }
      setValidationError(null);
      await onSubmit({ domain: canonicalDomain, kind: 'static', document_root: documentRoot });
      return;
    }

    const upstreamError = validateUpstreamUrl(upstreamUrl);
    if (upstreamError) {
      setValidationError(upstreamError);
      return;
    }
    setValidationError(null);
    await onSubmit({ domain: canonicalDomain, kind: 'reverse_proxy', upstream_url: upstreamUrl });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="create-site-heading"
        tabIndex={-1}
        className="w-full max-w-xl rounded-lg bg-white p-6 shadow-xl dark:bg-gray-800"
      >
        <h2 id="create-site-heading" className="mb-5 text-xl font-semibold text-gray-900 dark:text-white">Create website</h2>
        {(validationError || submitError) && (
          <div role="alert" className="mb-4 rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
            {validationError || submitError}
          </div>
        )}

        <form onSubmit={handleSubmit} noValidate className="space-y-5">
          <div>
            <label htmlFor="website-domain" className="mb-1 block text-sm font-medium text-gray-800 dark:text-gray-100">Domain name</label>
            <input
              id="website-domain"
              name="domain"
              type="text"
              autoComplete="url"
              value={domain}
              onChange={(event) => handleDomainChange(event.target.value)}
              aria-describedby="website-domain-help"
              aria-invalid={Boolean(validationError && !isValidFQDN(domain.trim()))}
              disabled={isSubmitting}
              className="w-full rounded border border-gray-300 p-2 text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              required
            />
            <p id="website-domain-help" className="mt-1 text-sm text-gray-600 dark:text-gray-300">Use an ASCII hostname such as app.local.</p>
          </div>

          <fieldset disabled={isSubmitting} className="space-y-2">
            <legend className="mb-2 text-sm font-medium text-gray-800 dark:text-gray-100">Website type</legend>
            <label htmlFor="website-kind-static" className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-100">
              <input
                id="website-kind-static"
                type="radio"
                name="website-kind"
                value="static"
                checked={kind === 'static'}
                onChange={() => { setKind('static'); setValidationError(null); }}
              />
              Static site
            </label>
            <label htmlFor="website-kind-proxy" className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-100">
              <input
                id="website-kind-proxy"
                type="radio"
                name="website-kind"
                value="reverse_proxy"
                checked={kind === 'reverse_proxy'}
                onChange={() => { setKind('reverse_proxy'); setValidationError(null); }}
              />
              Reverse proxy
            </label>
          </fieldset>

          {kind === 'static' ? (
            <div>
              <label htmlFor="website-document-root" className="mb-1 block text-sm font-medium text-gray-800 dark:text-gray-100">Document root</label>
              <input
                id="website-document-root"
                name="document_root"
                type="text"
                value={documentRoot}
                onChange={(event) => { setDocumentRoot(event.target.value); setValidationError(null); }}
                aria-describedby="website-document-root-help"
                disabled={isSubmitting}
                className="w-full rounded border border-gray-300 p-2 text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
                required
              />
              <p id="website-document-root-help" className="mt-1 text-sm text-gray-600 dark:text-gray-300">Must stay under /srv/fusionpanel/sites/{domain.trim().toLowerCase() || 'domain'}.</p>
            </div>
          ) : (
            <div>
              <label htmlFor="website-upstream-url" className="mb-1 block text-sm font-medium text-gray-800 dark:text-gray-100">Upstream URL</label>
              <input
                id="website-upstream-url"
                name="upstream_url"
                type="url"
                value={upstreamUrl}
                onChange={(event) => { setUpstreamUrl(event.target.value); setValidationError(null); }}
                aria-describedby="website-upstream-url-help"
                disabled={isSubmitting}
                className="w-full rounded border border-gray-300 p-2 text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
                placeholder="http://127.0.0.1:8080"
                required
              />
              <p id="website-upstream-url-help" className="mt-1 text-sm text-gray-600 dark:text-gray-300">Use an absolute HTTP or HTTPS URL without credentials, query, or fragment.</p>
            </div>
          )}

          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={onClose}
              disabled={isSubmitting}
              className="rounded border border-gray-300 px-4 py-2 text-gray-800 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-700"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="rounded bg-blue-600 px-4 py-2 font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              {isSubmitting ? 'Submitting…' : 'Create website'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
