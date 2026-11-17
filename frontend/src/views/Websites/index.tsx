import React, { useEffect, useRef, useState } from 'react';
import {
  api,
  ApplyWebsiteTlsRequest,
  createIdempotencyKey,
  CreateWebsiteRequest,
  WebsiteRecord,
  WebsiteTlsRecord,
} from '../../api';
import { JobRecord } from '../../types';
import { CreateSiteModal } from './CreateSiteModal';
import { SiteDetail } from './SiteDetail';
import { SiteList } from './SiteList';

const POLL_INTERVAL_MS = 2500;

type PendingOperation = {
  siteId: string;
  domain: string;
  action: 'create' | 'delete';
  job: JobRecord;
};

type PendingCreateRequest = {
  payloadSignature: string;
  idempotencyKey: string;
};

const messageFromError = (error: unknown, fallback: string): string => {
  const value = error as { response?: { data?: { error?: { message?: string } } }; message?: string };
  return value.response?.data?.error?.message || value.message || fallback;
};

const statusFromError = (error: unknown): number | undefined => {
  const value = error as { response?: { status?: number } };
  return value.response?.status;
};

const isUncertainMutationError = (error: unknown): boolean => {
  const status = statusFromError(error);
  return status === undefined || status === 408 || status === 425 || status === 429 || status >= 500;
};

const upsertSite = (sites: WebsiteRecord[], site: WebsiteRecord): WebsiteRecord[] => {
  const exists = sites.some((current) => current.id === site.id);
  return exists
    ? sites.map((current) => (current.id === site.id ? site : current))
    : [site, ...sites];
};

export const WebsitesView: React.FC = () => {
  const [sites, setSites] = useState<WebsiteRecord[]>([]);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [selectedSiteId, setSelectedSiteId] = useState<string | null>(null);
  const [selectedSiteTls, setSelectedSiteTls] = useState<WebsiteTlsRecord | null>(null);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [isTlsLoading, setIsTlsLoading] = useState(false);
  const [isTlsSubmitting, setIsTlsSubmitting] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [tlsError, setTlsError] = useState<string | null>(null);
  const [pendingOperations, setPendingOperations] = useState<PendingOperation[]>([]);
  const [deletingSiteIds, setDeletingSiteIds] = useState<string[]>([]);
  const pendingCreateRequestRef = useRef<PendingCreateRequest | null>(null);
  const pendingDeleteKeysRef = useRef<Map<string, string>>(new Map());
  const pendingTlsApplyKeysRef = useRef<Map<string, { signature: string; key: string }>>(new Map());
  const pendingTlsDeleteKeysRef = useRef<Map<string, string>>(new Map());

  const selectedSite = selectedSiteId ? sites.find((site) => site.id === selectedSiteId) || null : null;

  const loadSites = async () => {
    setLoadError(null);
    setIsLoading(true);
    try {
      setSites(await api.getSites());
    } catch (error) {
      setLoadError(messageFromError(error, 'Could not load websites.'));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    void loadSites();
  }, []);

  const pendingSiteIds = Array.from(new Set([
    ...pendingOperations.map((operation) => operation.siteId),
    ...deletingSiteIds,
  ]));
  const sitesToPoll = sites.filter((site) => (
    site.state === 'provisioning'
    || site.state === 'deleting'
    || pendingOperations.some((operation) => operation.siteId === site.id)
  ));
  const pollingSignature = sitesToPoll
    .map((site) => {
      const operation = pendingOperations.find((pending) => pending.siteId === site.id);
      return `${site.id}:${site.state}:${operation?.action || ''}:${operation?.job.id || ''}`;
    })
    .sort()
    .join('|');

  useEffect(() => {
    if (!pollingSignature) return undefined;

    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const poll = async () => {
      for (const site of sitesToPoll) {
        if (!current) return;
        const operation = pendingOperations.find((pending) => pending.siteId === site.id);

        try {
          const refreshedSite = await api.getSite(site.id);
          if (!current) return;
          setSites((currentSites) => upsertSite(currentSites, refreshedSite));

          let refreshedJob = operation?.job;
          if (operation) {
            try {
              refreshedJob = await api.getJob(operation.job.id);
              if (!current) return;
              setPendingOperations((currentOperations) => currentOperations.map((pending) => (
                pending.siteId === operation.siteId ? { ...pending, job: refreshedJob as JobRecord } : pending
              )));
            } catch (jobError) {
              if (current) setOperationError(messageFromError(jobError, `Could not refresh job for ${site.domain}.`));
            }
          }

          if (operation?.action === 'create' && (refreshedSite.state === 'active' || refreshedSite.state === 'failed')) {
            setPendingOperations((currentOperations) => currentOperations.filter((pending) => pending.siteId !== site.id));
            if (refreshedSite.state === 'active') {
              setOperationError(null);
              setNotice(`${refreshedSite.domain} is active.`);
            } else {
              setNotice(null);
              setOperationError(refreshedSite.last_error || `${refreshedSite.domain} provisioning failed.`);
            }
          } else if (
            operation?.action === 'delete'
            && refreshedJob
            && (refreshedJob.state === 'failed' || refreshedJob.state === 'cancelled')
            && refreshedSite.state !== 'deleting'
          ) {
            setPendingOperations((currentOperations) => currentOperations.filter((pending) => pending.siteId !== site.id));
            setNotice(null);
            setOperationError(refreshedJob.error_text || `${refreshedSite.domain} could not be deleted.`);
          }
        } catch (error) {
          if (!current) return;
          if (statusFromError(error) === 404) {
            setSites((currentSites) => currentSites.filter((currentSite) => currentSite.id !== site.id));
            if (operation?.action === 'delete') {
              setPendingOperations((currentOperations) => currentOperations.filter((pending) => pending.siteId !== site.id));
              setOperationError(null);
              setNotice(`${site.domain} was deleted and is no longer available.`);
            } else if (operation?.action === 'create') {
              setPendingOperations((currentOperations) => currentOperations.filter((pending) => pending.siteId !== site.id));
              setNotice(null);
              setOperationError(`${site.domain} disappeared while it was being provisioned.`);
            } else {
              setNotice(`${site.domain} is no longer available; the list has been updated.`);
            }
            setSelectedSiteId((currentId) => (currentId === site.id ? null : currentId));
          } else {
            setOperationError(messageFromError(error, `Could not refresh ${site.domain} state.`));
          }
        }
      }

      if (current) timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
    };

    timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      current = false;
      if (timer) clearTimeout(timer);
    };
  }, [pollingSignature]);

  const handleCreateSite = async (request: CreateWebsiteRequest) => {
    const payloadSignature = JSON.stringify(request);
    const previousRequest = pendingCreateRequestRef.current;
    const idempotencyKey = previousRequest?.payloadSignature === payloadSignature
      ? previousRequest.idempotencyKey
      : createIdempotencyKey();
    pendingCreateRequestRef.current = { payloadSignature, idempotencyKey };

    setCreateError(null);
    setOperationError(null);
    setNotice(null);
    setIsCreating(true);
    try {
      const response = await api.createSite(request, idempotencyKey);
      if (pendingCreateRequestRef.current?.idempotencyKey === idempotencyKey) {
        pendingCreateRequestRef.current = null;
      }
      setSites((currentSites) => upsertSite(currentSites, response.site));
      setIsCreateModalOpen(false);

      if (response.site.state === 'provisioning') {
        setPendingOperations((currentOperations) => [
          ...currentOperations.filter((operation) => operation.siteId !== response.site.id),
          { siteId: response.site.id, domain: response.site.domain, action: 'create', job: response.job },
        ]);
        setNotice(`Provisioning ${response.site.domain}; waiting for the agent to report its persisted state.`);
      } else if (response.site.state === 'active') {
        setNotice(`${response.site.domain} is active.`);
      } else {
        setOperationError(response.site.last_error || `${response.site.domain} provisioning failed.`);
      }
    } catch (error) {
      if (!isUncertainMutationError(error) && pendingCreateRequestRef.current?.idempotencyKey === idempotencyKey) {
        pendingCreateRequestRef.current = null;
      }
      setCreateError(messageFromError(error, 'Could not create website.'));
    } finally {
      setIsCreating(false);
    }
  };

  const handleSelectSite = async (site: WebsiteRecord) => {
    setSelectedSiteId(site.id);
    setDetailError(null);
    setTlsError(null);
    setIsDetailLoading(true);
    setIsTlsLoading(true);
    try {
      const [detail, tlsRecord] = await Promise.all([
        api.getSite(site.id),
        api.getSiteTls(site.id).catch((error) => {
          setTlsError(messageFromError(error, `Could not load TLS status for ${site.domain}.`));
          return null;
        }),
      ]);
      setSites((currentSites) => upsertSite(currentSites, detail));
      setSelectedSiteTls(tlsRecord);
    } catch (error) {
      setDetailError(messageFromError(error, `Could not load details for ${site.domain}.`));
    } finally {
      setIsDetailLoading(false);
      setIsTlsLoading(false);
    }
  };

  const handleDeleteSite = async (site: WebsiteRecord) => {
    const idempotencyKey = pendingDeleteKeysRef.current.get(site.id) || createIdempotencyKey();
    pendingDeleteKeysRef.current.set(site.id, idempotencyKey);

    setOperationError(null);
    setNotice(null);
    setDeletingSiteIds((currentIds) => currentIds.includes(site.id) ? currentIds : [...currentIds, site.id]);
    try {
      const response = await api.deleteSite(site.id, idempotencyKey);
      if (pendingDeleteKeysRef.current.get(site.id) === idempotencyKey) {
        pendingDeleteKeysRef.current.delete(site.id);
      }
      setSites((currentSites) => upsertSite(currentSites, response.site));
      setPendingOperations((currentOperations) => [
        ...currentOperations.filter((operation) => operation.siteId !== site.id),
        { siteId: site.id, domain: site.domain, action: 'delete', job: response.job },
      ]);
      setSelectedSiteId((currentId) => (currentId === site.id ? null : currentId));
      setNotice(`Deletion queued for ${site.domain}; waiting for the API to confirm it is gone.`);
    } catch (error) {
      if (!isUncertainMutationError(error) && pendingDeleteKeysRef.current.get(site.id) === idempotencyKey) {
        pendingDeleteKeysRef.current.delete(site.id);
      }
      setOperationError(messageFromError(error, `Could not delete ${site.domain}.`));
    } finally {
      setDeletingSiteIds((currentIds) => currentIds.filter((id) => id !== site.id));
    }
  };

  useEffect(() => {
    if (!selectedSiteId || (selectedSiteTls?.state !== 'provisioning' && selectedSiteTls?.state !== 'deleting')) {
      return undefined;
    }

    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const pollTls = async () => {
      try {
        const refreshed = await api.getSiteTls(selectedSiteId);
        if (!current) return;
        setSelectedSiteTls(refreshed);
        if (refreshed?.state === 'active') {
          setTlsError(null);
          setNotice(`TLS is active for ${selectedSite?.domain || selectedSiteId}.`);
        } else if (refreshed?.state === 'failed') {
          setNotice(null);
          setTlsError(refreshed.last_error || 'TLS provisioning failed.');
        } else if (!refreshed || refreshed.state === 'disabled') {
          setTlsError(null);
          setNotice(`TLS removed for ${selectedSite?.domain || selectedSiteId}.`);
        }
      } catch (error) {
        if (current) {
          setTlsError(messageFromError(error, 'Could not refresh TLS status.'));
        }
      }

      if (current) timer = setTimeout(() => void pollTls(), POLL_INTERVAL_MS);
    };

    timer = setTimeout(() => void pollTls(), POLL_INTERVAL_MS);
    return () => {
      current = false;
      if (timer) clearTimeout(timer);
    };
  }, [selectedSiteId, selectedSite?.domain, selectedSiteTls?.state]);

  const handleApplyTls = async (site: WebsiteRecord, request: ApplyWebsiteTlsRequest): Promise<boolean> => {
    const signature = request.mode === 'acme_http01'
      ? `acme:${request.acme_directory_url}:${request.contact_email || ''}`
      : `custom:${request.certificate_pem.length}`;
    const existing = pendingTlsApplyKeysRef.current.get(site.id);
    const idempotencyKey = existing?.signature === signature ? existing.key : createIdempotencyKey();
    pendingTlsApplyKeysRef.current.set(site.id, { signature, key: idempotencyKey });

    setTlsError(null);
    setNotice(null);
    setIsTlsSubmitting(true);
    try {
      const response = await api.applySiteTls(site.id, request, idempotencyKey);
      if (pendingTlsApplyKeysRef.current.get(site.id)?.key === idempotencyKey) {
        pendingTlsApplyKeysRef.current.delete(site.id);
      }
      setSelectedSiteTls(response.tls);
      if (response.tls.state === 'active') {
        setNotice(`TLS is active for ${site.domain}.`);
      } else if (response.tls.state === 'failed') {
        setTlsError(response.tls.last_error || `TLS failed for ${site.domain}.`);
      } else {
        setNotice(`TLS provisioning queued for ${site.domain}.`);
      }
      return true;
    } catch (error) {
      if (!isUncertainMutationError(error) && pendingTlsApplyKeysRef.current.get(site.id)?.key === idempotencyKey) {
        pendingTlsApplyKeysRef.current.delete(site.id);
      }
      setTlsError(messageFromError(error, `Could not apply TLS for ${site.domain}.`));
      return false;
    } finally {
      setIsTlsSubmitting(false);
    }
  };

  const handleDeleteTls = async (site: WebsiteRecord): Promise<void> => {
    const idempotencyKey = pendingTlsDeleteKeysRef.current.get(site.id) || createIdempotencyKey();
    pendingTlsDeleteKeysRef.current.set(site.id, idempotencyKey);

    setTlsError(null);
    setNotice(null);
    setIsTlsSubmitting(true);
    try {
      const response = await api.deleteSiteTls(site.id, idempotencyKey);
      if (pendingTlsDeleteKeysRef.current.get(site.id) === idempotencyKey) {
        pendingTlsDeleteKeysRef.current.delete(site.id);
      }
      setSelectedSiteTls(response.tls);
      if (response.tls.state === 'disabled') {
        setNotice(`TLS removed for ${site.domain}.`);
      } else {
        setNotice(`TLS removal queued for ${site.domain}.`);
      }
    } catch (error) {
      if (!isUncertainMutationError(error) && pendingTlsDeleteKeysRef.current.get(site.id) === idempotencyKey) {
        pendingTlsDeleteKeysRef.current.delete(site.id);
      }
      setTlsError(messageFromError(error, `Could not remove TLS for ${site.domain}.`));
    } finally {
      setIsTlsSubmitting(false);
    }
  };

  const handleCloseDetails = () => {
    setSelectedSiteId(null);
    setSelectedSiteTls(null);
    setDetailError(null);
    setTlsError(null);
    setIsDetailLoading(false);
    setIsTlsLoading(false);
  };

  return (
    <main className="space-y-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Websites</h1>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">Create and manage static sites and reverse proxies.</p>
        </div>
        {!selectedSiteId && (
          <button
            type="button"
            onClick={() => { setCreateError(null); setIsCreateModalOpen(true); }}
            className="rounded bg-blue-600 px-4 py-2 font-medium text-white hover:bg-blue-700"
          >
            Create website
          </button>
        )}
      </header>

      {notice && <div role="status" aria-live="polite" className="rounded border border-blue-200 bg-blue-50 px-4 py-3 text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200">{notice}</div>}
      {loadError && (
        <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          <p>{loadError}</p>
          <button type="button" onClick={() => void loadSites()} className="mt-2 underline">Retry</button>
        </div>
      )}
      {operationError && (
        <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          {operationError}
        </div>
      )}

      {pendingOperations.length > 0 && (
        <section aria-labelledby="pending-website-operations" className="rounded border border-gray-200 bg-white px-4 py-3 dark:border-gray-700 dark:bg-gray-800">
          <h2 id="pending-website-operations" className="font-medium text-gray-900 dark:text-white">Pending website operations</h2>
          <ul className="mt-2 space-y-1 text-sm text-gray-700 dark:text-gray-200">
            {pendingOperations.map((operation) => (
              <li key={operation.siteId}>
                {operation.action === 'create' ? 'Provisioning' : 'Deleting'} {operation.domain}; job {operation.job.id} is {operation.job.state}.
              </li>
            ))}
          </ul>
        </section>
      )}

      {isLoading && sites.length === 0 ? (
        <p role="status" className="py-8 text-center text-gray-600 dark:text-gray-300">Loading websites…</p>
      ) : loadError && sites.length === 0 ? (
        <p role="status" className="py-8 text-center text-gray-600 dark:text-gray-300">Website list is unavailable until the request succeeds.</p>
      ) : selectedSiteId ? (
        <section>
          {isDetailLoading && <p role="status" className="mb-3 text-gray-600 dark:text-gray-300">Refreshing website details…</p>}
          {detailError && (
            <div role="alert" className="mb-4 rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
              <p>{detailError}</p>
              {selectedSite && <button type="button" onClick={() => void handleSelectSite(selectedSite)} className="mt-2 underline">Retry</button>}
            </div>
          )}
          {selectedSite && (
            <SiteDetail
              site={selectedSite}
              tls={selectedSiteTls}
              isTlsLoading={isTlsLoading}
              isTlsSubmitting={isTlsSubmitting}
              tlsError={tlsError}
              onApplyTls={handleApplyTls}
              onDeleteTls={handleDeleteTls}
              onClose={handleCloseDetails}
            />
          )}
          {!selectedSite && !isDetailLoading && <p role="status">Website not found.</p>}
        </section>
      ) : (
        <SiteList
          sites={sites}
          pendingSiteIds={pendingSiteIds}
          onSelectSite={(site) => { void handleSelectSite(site); }}
          onDeleteSite={(site) => { void handleDeleteSite(site); }}
        />
      )}

      <CreateSiteModal
        isOpen={isCreateModalOpen}
        isSubmitting={isCreating}
        submitError={createError}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSite}
      />
    </main>
  );
};

export default WebsitesView;
