import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  api,
  createIdempotencyKey,
  DnsRecordEntry,
  DnsZoneRecord,
} from '../../api';
import {
  MailAliasRecord,
  MailDkimRecord,
  MailDomainRecord,
  MailboxRecord,
  MailResourceState,
  NodeRecord,
} from '../../types';

const STATE_BADGE: Record<MailResourceState, string> = {
  provisioning: 'bg-amber-950/60 text-amber-300 border-amber-800',
  ready: 'bg-emerald-950/60 text-emerald-300 border-emerald-800',
  failed: 'bg-red-950/60 text-red-300 border-red-800',
  deleting: 'bg-slate-800 text-slate-300 border-slate-700',
};

const MIB = 1024 * 1024;
const POLL_INTERVAL_MS = 2000;

function extractErrorMessage(err: unknown, fallback: string): string {
  const anyErr = err as {
    response?: { data?: { error?: { message?: string } } };
    message?: string;
  };
  return anyErr?.response?.data?.error?.message || anyErr?.message || fallback;
}

function isForbiddenError(err: unknown): boolean {
  const anyErr = err as {
    response?: { status?: number; data?: { error?: { code?: string } } };
  };
  return (
    anyErr?.response?.status === 403 ||
    anyErr?.response?.data?.error?.code === 'forbidden'
  );
}

function isValidDomainName(value: string): boolean {
  const trimmed = value.trim().toLowerCase();
  if (!trimmed || trimmed.length > 253) return false;
  return /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$/.test(
    trimmed,
  );
}

function isValidLocalPart(value: string): boolean {
  const trimmed = value.trim().toLowerCase();
  if (!trimmed || trimmed.length > 64) return false;
  return /^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+$/.test(trimmed) && !trimmed.includes('..');
}

function isValidEmailAddress(value: string): boolean {
  const trimmed = value.trim().toLowerCase();
  const parts = trimmed.split('@');
  if (parts.length !== 2) return false;
  return isValidLocalPart(parts[0]) && isValidDomainName(parts[1]);
}

function isValidDkimSelector(value: string): boolean {
  const trimmed = value.trim().toLowerCase();
  if (!trimmed || trimmed.length > 63) return false;
  return /^[a-z0-9](?:[a-z0-9._-]{0,61}[a-z0-9])?$/.test(trimmed);
}

function hasControlChars(value: string): boolean {
  return /[\0\r\n]/.test(value);
}

type ConfirmTarget =
  | { kind: 'domain'; id: string; label: string }
  | { kind: 'mailbox'; id: string; label: string }
  | { kind: 'alias'; id: string; label: string }
  | { kind: 'dkim'; id: string; label: string };

export const MailView: React.FC = () => {
  const [domains, setDomains] = useState<MailDomainRecord[]>([]);
  const [nodes, setNodes] = useState<NodeRecord[]>([]);
  const [dnsZones, setDnsZones] = useState<DnsZoneRecord[]>([]);
  const [selectedDomainId, setSelectedDomainId] = useState<string | null>(null);
  const [selectedDomain, setSelectedDomain] = useState<MailDomainRecord | null>(null);
  const [mailboxes, setMailboxes] = useState<MailboxRecord[]>([]);
  const [aliases, setAliases] = useState<MailAliasRecord[]>([]);
  const [dkim, setDkim] = useState<MailDkimRecord | null>(null);
  const [linkedZoneRecords, setLinkedZoneRecords] = useState<DnsRecordEntry[]>([]);

  const [loading, setLoading] = useState(true);
  const [deniedMessage, setDeniedMessage] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // Domain create/edit modal state
  const [isDomainModalOpen, setIsDomainModalOpen] = useState(false);
  const [editingDomainId, setEditingDomainId] = useState<string | null>(null);
  const [domainName, setDomainName] = useState('');
  const [domainNodeId, setDomainNodeId] = useState('');
  const [domainDnsZoneId, setDomainDnsZoneId] = useState('');
  const [domainActive, setDomainActive] = useState(true);
  const [domainValidationError, setDomainValidationError] = useState<string | null>(null);

  // Mailbox form state
  const [editingMailboxId, setEditingMailboxId] = useState<string | null>(null);
  const [mailboxLocalPart, setMailboxLocalPart] = useState('');
  const [mailboxPassword, setMailboxPassword] = useState('');
  const [mailboxQuotaMb, setMailboxQuotaMb] = useState('1024');
  const [mailboxActive, setMailboxActive] = useState(true);
  const [mailboxValidationError, setMailboxValidationError] = useState<string | null>(null);

  // Alias form state
  const [editingAliasId, setEditingAliasId] = useState<string | null>(null);
  const [aliasLocalPart, setAliasLocalPart] = useState('');
  const [aliasDestinations, setAliasDestinations] = useState('');
  const [aliasActive, setAliasActive] = useState(true);
  const [aliasValidationError, setAliasValidationError] = useState<string | null>(null);

  // DKIM form state
  const [dkimSelector, setDkimSelector] = useState('default');
  const [dkimEnabled, setDkimEnabled] = useState(true);
  const [dkimDnsZoneId, setDkimDnsZoneId] = useState('');
  const [dkimRotate, setDkimRotate] = useState(false);
  const [dkimValidationError, setDkimValidationError] = useState<string | null>(null);

  // Confirmation state for removals
  const [confirmTarget, setConfirmTarget] = useState<ConfirmTarget | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Idempotency key cache keyed by operation slot so failed submissions preserve their key on retry
  const pendingIdempotencyKeys = useRef<Map<string, { signature: string; key: string }>>(
    new Map(),
  );

  const getIdempotencyKey = (slot: string, signature: string): string => {
    const existing = pendingIdempotencyKeys.current.get(slot);
    if (existing && existing.signature === signature) {
      return existing.key;
    }
    const key = createIdempotencyKey();
    pendingIdempotencyKeys.current.set(slot, { signature, key });
    return key;
  };

  const clearIdempotencyKey = (slot: string) => {
    pendingIdempotencyKeys.current.delete(slot);
  };

  const loadInitialData = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    setDeniedMessage(null);
    try {
      const caps = await api
        .getCapabilities()
        .catch(() => ({ capabilities: [] as string[], unavailable: [] as string[] }));
      if (caps.unavailable?.includes('mail')) {
        setDeniedMessage('Mail management capability is unavailable for this workspace.');
        setLoading(false);
        return;
      }

      const [domainList, nodeList, zoneList] = await Promise.all([
        api.getMailDomains(),
        api.getNodes().catch(() => [] as NodeRecord[]),
        api.getDnsZones().catch(() => [] as DnsZoneRecord[]),
      ]);

      const normalizedDomains = Array.isArray(domainList) ? domainList : [];
      const normalizedNodes = Array.isArray(nodeList) ? nodeList : [];
      const normalizedZones = Array.isArray(zoneList) ? zoneList : [];

      setDomains(normalizedDomains);
      setNodes(normalizedNodes);
      setDnsZones(normalizedZones);
      setDomainNodeId((prev) => prev || normalizedNodes[0]?.node_id || '');
      setSelectedDomainId((prev) => {
        if (prev && normalizedDomains.some((d) => d.id === prev)) return prev;
        return normalizedDomains[0]?.id ?? null;
      });
    } catch (err) {
      if (isForbiddenError(err)) {
        setDeniedMessage(
          extractErrorMessage(
            err,
            'Access denied: MailManage permission is required to manage mail domains.',
          ),
        );
      } else {
        setLoadError(extractErrorMessage(err, 'Failed to load mail domains.'));
      }
    } finally {
      setLoading(false);
    }
  }, []);

  const loadDomainDetails = useCallback(async (domainId: string) => {
    try {
      const details = await api.getMailDomain(domainId);
      setSelectedDomain(details.domain);
      setMailboxes(Array.isArray(details.mailboxes) ? details.mailboxes : []);
      setAliases(Array.isArray(details.aliases) ? details.aliases : []);
      setDkim(details.dkim ?? null);
      setDomains((prev) => prev.map((d) => (d.id === details.domain.id ? details.domain : d)));

      if (details.dkim) {
        setDkimSelector(details.dkim.selector);
        setDkimEnabled(details.dkim.enabled);
        setDkimDnsZoneId(details.dkim.dns_zone_id ?? details.domain.dns_zone_id ?? '');
      } else {
        setDkimSelector('default');
        setDkimEnabled(true);
        setDkimDnsZoneId(details.domain.dns_zone_id ?? '');
      }

      const effectiveZoneId = details.dkim?.dns_zone_id ?? details.domain.dns_zone_id;
      if (effectiveZoneId) {
        try {
          const zoneDetail = await api.getDnsZone(effectiveZoneId);
          setLinkedZoneRecords(Array.isArray(zoneDetail.records) ? zoneDetail.records : []);
          setDnsZones((prev) =>
            prev.some((z) => z.id === zoneDetail.zone.id)
              ? prev.map((z) => (z.id === zoneDetail.zone.id ? zoneDetail.zone : z))
              : [...prev, zoneDetail.zone],
          );
        } catch {
          setLinkedZoneRecords([]);
        }
      } else {
        setLinkedZoneRecords([]);
      }
    } catch (err) {
      if (isForbiddenError(err)) {
        setDeniedMessage(
          extractErrorMessage(err, 'Access denied: MailManage permission is required.'),
        );
      } else {
        setMutationError(extractErrorMessage(err, 'Failed to load mail domain details.'));
      }
    }
  }, []);

  useEffect(() => {
    void loadInitialData();
  }, [loadInitialData]);

  useEffect(() => {
    if (selectedDomainId && !deniedMessage) {
      void loadDomainDetails(selectedDomainId);
    } else if (!selectedDomainId) {
      setSelectedDomain(null);
      setMailboxes([]);
      setAliases([]);
      setDkim(null);
      setLinkedZoneRecords([]);
    }
  }, [selectedDomainId, deniedMessage, loadDomainDetails]);

  const hasPendingOperation = useMemo(() => {
    const domainPending =
      selectedDomain?.state === 'provisioning' || selectedDomain?.state === 'deleting';
    const dkimPending = dkim?.state === 'provisioning' || dkim?.state === 'deleting';
    return Boolean(domainPending || dkimPending);
  }, [selectedDomain, dkim]);

  useEffect(() => {
    if (!hasPendingOperation || !selectedDomainId) return undefined;
    const timer = window.setTimeout(() => {
      void loadDomainDetails(selectedDomainId);
    }, POLL_INTERVAL_MS);
    return () => window.clearTimeout(timer);
  }, [hasPendingOperation, selectedDomainId, loadDomainDetails]);

  const openCreateDomainModal = () => {
    setEditingDomainId(null);
    setDomainName('');
    setDomainNodeId(nodes[0]?.node_id || '');
    setDomainDnsZoneId('');
    setDomainActive(true);
    setDomainValidationError(null);
    setIsDomainModalOpen(true);
  };

  const openEditDomainModal = (domain: MailDomainRecord) => {
    setEditingDomainId(domain.id);
    setDomainName(domain.domain);
    setDomainNodeId(domain.node_id);
    setDomainDnsZoneId(domain.dns_zone_id ?? '');
    setDomainActive(domain.active);
    setDomainValidationError(null);
    setIsDomainModalOpen(true);
  };

  const handleDomainSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMutationError(null);
    setNotice(null);

    const normalizedDomain = domainName.trim().toLowerCase();
    if (!editingDomainId && !isValidDomainName(normalizedDomain)) {
      setDomainValidationError('Enter a valid canonical domain name (e.g. example.com).');
      return;
    }
    if (!domainNodeId.trim()) {
      setDomainValidationError('Select a target node.');
      return;
    }
    setDomainValidationError(null);

    const slot = editingDomainId ? `domain-update:${editingDomainId}` : 'domain-create';
    const payload = editingDomainId
      ? {
          node_id: domainNodeId.trim(),
          dns_zone_id: domainDnsZoneId ? domainDnsZoneId : null,
          active: domainActive,
        }
      : {
          domain: normalizedDomain,
          node_id: domainNodeId.trim(),
          dns_zone_id: domainDnsZoneId ? domainDnsZoneId : null,
          active: domainActive,
        };
    const idempotencyKey = getIdempotencyKey(slot, JSON.stringify(payload));

    setIsSubmitting(true);
    try {
      const res = editingDomainId
        ? await api.updateMailDomain(editingDomainId, payload, idempotencyKey)
        : await api.createMailDomain(
            payload as { domain: string; node_id: string; dns_zone_id?: string | null; active?: boolean },
            idempotencyKey,
          );
      clearIdempotencyKey(slot);
      setIsDomainModalOpen(false);
      setDomains((prev) =>
        prev.some((d) => d.id === res.domain.id)
          ? prev.map((d) => (d.id === res.domain.id ? res.domain : d))
          : [res.domain, ...prev],
      );
      setSelectedDomainId(res.domain.id);
      setSelectedDomain(res.domain);
      setNotice(
        `Queued ${editingDomainId ? 'update' : 'creation'} for ${res.domain.domain} (job ${res.job.id}).`,
      );
      await loadDomainDetails(res.domain.id);
    } catch (err) {
      setMutationError(
        extractErrorMessage(
          err,
          `Failed to ${editingDomainId ? 'update' : 'create'} mail domain.`,
        ),
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleMailboxSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedDomain) return;
    setMutationError(null);
    setNotice(null);

    const lp = mailboxLocalPart.trim().toLowerCase();
    if (!editingMailboxId && !isValidLocalPart(lp)) {
      setMailboxValidationError('Enter a valid mailbox local part (e.g. alice).');
      return;
    }
    if (!editingMailboxId && !mailboxPassword) {
      setMailboxValidationError('Password is required for new mailboxes.');
      return;
    }
    if (mailboxPassword) {
      if (mailboxPassword.length < 8 || hasControlChars(mailboxPassword)) {
        setMailboxValidationError(
          'Password must be at least 8 characters without control characters.',
        );
        return;
      }
    }
    const quotaMbNum = Number(mailboxQuotaMb);
    if (!Number.isFinite(quotaMbNum) || quotaMbNum <= 0) {
      setMailboxValidationError('Quota must be a positive number of MiB.');
      return;
    }
    setMailboxValidationError(null);

    const quotaBytes = Math.round(quotaMbNum * MIB);
    const slot = editingMailboxId
      ? `mailbox-update:${editingMailboxId}`
      : `mailbox-create:${selectedDomain.id}`;
    const signature = JSON.stringify({
      domainId: selectedDomain.id,
      editingMailboxId,
      lp,
      hasPassword: Boolean(mailboxPassword),
      pwLen: mailboxPassword.length,
      quotaBytes,
      mailboxActive,
    });
    const idempotencyKey = getIdempotencyKey(slot, signature);

    setIsSubmitting(true);
    try {
      const res = editingMailboxId
        ? await api.updateMailbox(
            selectedDomain.id,
            editingMailboxId,
            {
              ...(mailboxPassword ? { password: mailboxPassword } : {}),
              quota_bytes: quotaBytes,
              active: mailboxActive,
            },
            idempotencyKey,
          )
        : await api.createMailbox(
            selectedDomain.id,
            {
              local_part: lp,
              password: mailboxPassword,
              quota_bytes: quotaBytes,
              active: mailboxActive,
            },
            idempotencyKey,
          );
      clearIdempotencyKey(slot);
      // Clear credentials from form state immediately after submission succeeds
      setMailboxPassword('');
      setEditingMailboxId(null);
      setMailboxLocalPart('');
      setMailboxQuotaMb('1024');
      setMailboxActive(true);
      setSelectedDomain(res.domain);
      setNotice(`Queued mailbox ${editingMailboxId ? 'update' : 'creation'} (job ${res.job.id}).`);
      await loadDomainDetails(selectedDomain.id);
    } catch (err) {
      setMutationError(
        extractErrorMessage(err, `Failed to ${editingMailboxId ? 'update' : 'create'} mailbox.`),
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleAliasSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedDomain) return;
    setMutationError(null);
    setNotice(null);

    const lp = aliasLocalPart.trim().toLowerCase();
    if (!editingAliasId && !isValidLocalPart(lp)) {
      setAliasValidationError('Enter a valid alias local part (e.g. support).');
      return;
    }
    const destList = aliasDestinations
      .split(/[\n,]+/)
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean);
    if (destList.length === 0) {
      setAliasValidationError('Specify at least one destination address.');
      return;
    }
    if (destList.some((d) => !isValidEmailAddress(d))) {
      setAliasValidationError('All alias destinations must be valid email addresses.');
      return;
    }
    setAliasValidationError(null);

    const slot = editingAliasId
      ? `alias-update:${editingAliasId}`
      : `alias-create:${selectedDomain.id}`;
    const signature = JSON.stringify({
      domainId: selectedDomain.id,
      editingAliasId,
      lp,
      destList,
      aliasActive,
    });
    const idempotencyKey = getIdempotencyKey(slot, signature);

    setIsSubmitting(true);
    try {
      const res = editingAliasId
        ? await api.updateMailAlias(
            selectedDomain.id,
            editingAliasId,
            { destinations: destList, active: aliasActive },
            idempotencyKey,
          )
        : await api.createMailAlias(
            selectedDomain.id,
            { local_part: lp, destinations: destList, active: aliasActive },
            idempotencyKey,
          );
      clearIdempotencyKey(slot);
      setEditingAliasId(null);
      setAliasLocalPart('');
      setAliasDestinations('');
      setAliasActive(true);
      setSelectedDomain(res.domain);
      setNotice(`Queued alias ${editingAliasId ? 'update' : 'creation'} (job ${res.job.id}).`);
      await loadDomainDetails(selectedDomain.id);
    } catch (err) {
      setMutationError(
        extractErrorMessage(err, `Failed to ${editingAliasId ? 'update' : 'create'} alias.`),
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDkimSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedDomain) return;
    setMutationError(null);
    setNotice(null);

    const selector = dkimSelector.trim().toLowerCase();
    if (!isValidDkimSelector(selector)) {
      setDkimValidationError('Enter a valid DKIM selector (e.g. default).');
      return;
    }
    setDkimValidationError(null);

    const payload = {
      selector,
      enabled: dkimEnabled,
      dns_zone_id: dkimDnsZoneId ? dkimDnsZoneId : null,
      rotate: dkimRotate,
    };
    const slot = `dkim-upsert:${selectedDomain.id}`;
    const idempotencyKey = getIdempotencyKey(slot, JSON.stringify(payload));

    setIsSubmitting(true);
    try {
      const res = await api.upsertMailDkim(selectedDomain.id, payload, idempotencyKey);
      clearIdempotencyKey(slot);
      setDkimRotate(false);
      setDkim(res.dkim);
      setSelectedDomain(res.domain);
      setNotice(`Queued DKIM configuration for ${selectedDomain.domain} (job ${res.job.id}).`);
      await loadDomainDetails(selectedDomain.id);
    } catch (err) {
      setMutationError(extractErrorMessage(err, 'Failed to save DKIM configuration.'));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleConfirmDelete = async () => {
    if (!confirmTarget) return;
    setMutationError(null);
    setNotice(null);

    const slot = `delete:${confirmTarget.kind}:${confirmTarget.id}`;
    const idempotencyKey = getIdempotencyKey(slot, slot);
    setIsSubmitting(true);
    try {
      if (confirmTarget.kind === 'domain') {
        const res = await api.deleteMailDomain(confirmTarget.id, idempotencyKey);
        clearIdempotencyKey(slot);
        setConfirmTarget(null);
        setNotice(`Queued deletion for ${confirmTarget.label} (job ${res.job.id}).`);
        await loadInitialData();
      } else if (confirmTarget.kind === 'mailbox' && selectedDomain) {
        const res = await api.deleteMailbox(selectedDomain.id, confirmTarget.id, idempotencyKey);
        clearIdempotencyKey(slot);
        setConfirmTarget(null);
        setSelectedDomain(res.domain);
        setNotice(`Queued mailbox removal for ${confirmTarget.label} (job ${res.job.id}).`);
        await loadDomainDetails(selectedDomain.id);
      } else if (confirmTarget.kind === 'alias' && selectedDomain) {
        const res = await api.deleteMailAlias(selectedDomain.id, confirmTarget.id, idempotencyKey);
        clearIdempotencyKey(slot);
        setConfirmTarget(null);
        setSelectedDomain(res.domain);
        setNotice(`Queued alias removal for ${confirmTarget.label} (job ${res.job.id}).`);
        await loadDomainDetails(selectedDomain.id);
      } else if (confirmTarget.kind === 'dkim' && selectedDomain) {
        const res = await api.deleteMailDkim(selectedDomain.id, idempotencyKey);
        clearIdempotencyKey(slot);
        setConfirmTarget(null);
        setDkim(res.dkim);
        setSelectedDomain(res.domain);
        setNotice(`Queued DKIM disable/removal for ${selectedDomain.domain} (job ${res.job.id}).`);
        await loadDomainDetails(selectedDomain.id);
      }
    } catch (err) {
      setMutationError(extractErrorMessage(err, `Failed to delete ${confirmTarget.label}.`));
    } finally {
      setIsSubmitting(false);
    }
  };

  const ownedDkimZone = useMemo(() => {
    const zoneId = dkim?.dns_zone_id ?? selectedDomain?.dns_zone_id;
    if (!zoneId) return null;
    return dnsZones.find((z) => z.id === zoneId) ?? null;
  }, [dkim, selectedDomain, dnsZones]);

  const dkimPublicationStatus = useMemo(() => {
    if (!ownedDkimZone) return null;
    const expectedName = `${dkim?.selector || dkimSelector}._domainkey`;
    const matchingTxt = linkedZoneRecords.find(
      (r) => r.record_type === 'TXT' && r.name.toLowerCase() === expectedName.toLowerCase(),
    );
    if (ownedDkimZone.state === 'failed') {
      return { label: 'DNS zone failed', tone: 'text-red-300 bg-red-950/60 border-red-800' };
    }
    if (matchingTxt && ownedDkimZone.state === 'ready') {
      return {
        label: 'Published in DNS zone',
        tone: 'text-emerald-300 bg-emerald-950/60 border-emerald-800',
      };
    }
    if (matchingTxt) {
      return {
        label: 'DNS sync provisioning',
        tone: 'text-amber-300 bg-amber-950/60 border-amber-800',
      };
    }
    return {
      label: 'Not yet published in DNS zone',
      tone: 'text-slate-300 bg-slate-800 border-slate-700',
    };
  }, [ownedDkimZone, linkedZoneRecords, dkim, dkimSelector]);

  if (loading) {
    return <div className="p-8 text-center text-slate-400">Loading mail domains...</div>;
  }

  if (deniedMessage) {
    return (
      <div
        role="alert"
        className="p-6 bg-red-950/50 border border-red-800 rounded-lg text-red-200 space-y-2"
      >
        <h2 className="text-lg font-semibold text-red-100">Access Denied</h2>
        <p className="text-sm">{deniedMessage}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap justify-between items-center gap-4">
        <div>
          <h2 className="text-xl font-semibold text-white">Mail Domains &amp; DKIM</h2>
          <p className="text-sm text-slate-400">
            Manage mail domains, mailboxes, local aliases, and DKIM signing configuration.
          </p>
        </div>
        <button
          type="button"
          onClick={openCreateDomainModal}
          className="px-4 py-2 bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium rounded-md"
        >
          Create Mail Domain
        </button>
      </div>

      {loadError && (
        <div
          role="alert"
          className="p-3 bg-red-950/60 border border-red-800 rounded text-sm text-red-300 flex items-center justify-between gap-3"
        >
          <span>{loadError}</span>
          <button
            type="button"
            onClick={() => void loadInitialData()}
            className="px-3 py-1 bg-red-900/70 hover:bg-red-800 text-red-100 rounded text-xs"
          >
            Retry
          </button>
        </div>
      )}

      {mutationError && (
        <div
          role="alert"
          className="p-3 bg-red-950/60 border border-red-800 rounded text-sm text-red-300"
        >
          {mutationError}
        </div>
      )}

      {notice && (
        <div className="p-3 bg-sky-950/60 border border-sky-800 rounded text-sm text-sky-200">
          {notice}
        </div>
      )}

      {confirmTarget && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-delete-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
        >
          <div className="bg-slate-900 border border-slate-800 rounded-lg max-w-md w-full p-6 space-y-4">
            <h3 id="confirm-delete-title" className="text-lg font-medium text-white">
              Confirm Removal
            </h3>
            <p className="text-sm text-slate-300">
              Are you sure you want to remove <span className="font-mono text-white">{confirmTarget.label}</span>?
            </p>
            <div className="flex justify-end gap-3">
              <button
                type="button"
                onClick={() => setConfirmTarget(null)}
                className="px-4 py-2 text-sm text-slate-400 hover:text-slate-200"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={isSubmitting}
                onClick={() => void handleConfirmDelete()}
                className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-sm font-medium rounded disabled:opacity-50"
              >
                Confirm Delete
              </button>
            </div>
          </div>
        </div>
      )}

      {isDomainModalOpen && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="mail-domain-modal-title"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
        >
          <div className="bg-slate-900 border border-slate-800 rounded-lg max-w-md w-full p-6">
            <h3 id="mail-domain-modal-title" className="text-lg font-medium text-white mb-4">
              {editingDomainId ? 'Update Mail Domain' : 'Create Mail Domain'}
            </h3>
            <form onSubmit={handleDomainSubmit} noValidate className="space-y-4">
              {domainValidationError && (
                <div className="p-2.5 bg-red-950/60 border border-red-800 rounded text-xs text-red-300">
                  {domainValidationError}
                </div>
              )}
              <div>
                <label htmlFor="mail-domain-name" className="block text-sm text-slate-300 mb-1">
                  Domain Name
                </label>
                <input
                  id="mail-domain-name"
                  type="text"
                  value={domainName}
                  onChange={(e) => setDomainName(e.target.value)}
                  disabled={Boolean(editingDomainId)}
                  placeholder="example.com"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono disabled:opacity-60"
                />
              </div>
              <div>
                <label htmlFor="mail-domain-node" className="block text-sm text-slate-300 mb-1">
                  Target Node
                </label>
                {nodes.length > 0 ? (
                  <select
                    id="mail-domain-node"
                    value={domainNodeId}
                    onChange={(e) => setDomainNodeId(e.target.value)}
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                  >
                    <option value="">Select node...</option>
                    {nodes.map((node) => (
                      <option key={node.node_id} value={node.node_id}>
                        {node.name} ({node.state})
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    id="mail-domain-node"
                    type="text"
                    value={domainNodeId}
                    onChange={(e) => setDomainNodeId(e.target.value)}
                    placeholder="Node UUID"
                    className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                  />
                )}
              </div>
              <div>
                <label htmlFor="mail-domain-dns-zone" className="block text-sm text-slate-300 mb-1">
                  Owned DNS Zone (Optional)
                </label>
                <select
                  id="mail-domain-dns-zone"
                  value={domainDnsZoneId}
                  onChange={(e) => setDomainDnsZoneId(e.target.value)}
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                >
                  <option value="">None</option>
                  {dnsZones.map((zone) => (
                    <option key={zone.id} value={zone.id}>
                      {zone.domain} ({zone.state})
                    </option>
                  ))}
                </select>
              </div>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={domainActive}
                  onChange={(e) => setDomainActive(e.target.checked)}
                />
                <span>Active</span>
              </label>
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setIsDomainModalOpen(false)}
                  className="px-4 py-2 text-sm text-slate-400 hover:text-slate-200"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={isSubmitting}
                  className="px-4 py-2 bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium rounded disabled:opacity-50"
                >
                  Save Domain
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {domains.length === 0 ? (
        <div className="p-10 text-center bg-slate-900 border border-slate-800 rounded-lg text-slate-400">
          No mail domains configured yet.
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-3 space-y-2">
            <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider px-2">
              Mail Domains
            </h3>
            <ul className="space-y-1">
              {domains.map((dom) => (
                <li key={dom.id}>
                  <button
                    type="button"
                    onClick={() => setSelectedDomainId(dom.id)}
                    className={`w-full text-left px-3 py-2 rounded text-sm flex items-center justify-between gap-2 ${
                      selectedDomainId === dom.id
                        ? 'bg-slate-800 text-white'
                        : 'text-slate-300 hover:bg-slate-800/50'
                    }`}
                  >
                    <span className="font-mono truncate">{dom.domain}</span>
                    <span
                      className={`text-[11px] px-1.5 py-0.5 rounded border ${STATE_BADGE[dom.state]}`}
                    >
                      {dom.state}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>

          {selectedDomain && (
            <div className="lg:col-span-3 space-y-6">
              {/* Domain Status & Revisions */}
              <div className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-4">
                  <div>
                    <div className="flex items-center gap-3">
                      <h3 className="text-lg font-mono font-semibold text-white">
                        {selectedDomain.domain}
                      </h3>
                      <span
                        className={`text-xs px-2 py-0.5 rounded border ${STATE_BADGE[selectedDomain.state]}`}
                      >
                        {selectedDomain.state}
                      </span>
                      <span className="text-xs px-2 py-0.5 rounded border border-slate-700 text-slate-300">
                        {selectedDomain.active ? 'active' : 'disabled'}
                      </span>
                    </div>
                    <div className="text-xs text-slate-400 mt-1 flex flex-wrap gap-x-4 gap-y-1">
                      <span>Desired Rev: {selectedDomain.desired_revision}</span>
                      <span>Queued Rev: {selectedDomain.queued_revision ?? '—'}</span>
                      <span>Applied Rev: {selectedDomain.applied_revision ?? '—'}</span>
                      <span>Failed Rev: {selectedDomain.failed_revision ?? '—'}</span>
                      {selectedDomain.pending_job_id && (
                        <span className="font-mono text-amber-300">
                          Pending Job: {selectedDomain.pending_job_id}
                        </span>
                      )}
                    </div>
                    {selectedDomain.last_error && (
                      <p className="text-xs text-red-400 mt-1">{selectedDomain.last_error}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => openEditDomainModal(selectedDomain)}
                      className="px-3 py-1.5 text-xs bg-slate-800 hover:bg-slate-700 text-slate-200 border border-slate-700 rounded"
                    >
                      Edit Domain
                    </button>
                    <button
                      type="button"
                      onClick={() =>
                        setConfirmTarget({
                          kind: 'domain',
                          id: selectedDomain.id,
                          label: selectedDomain.domain,
                        })
                      }
                      className="px-3 py-1.5 text-xs bg-red-950/60 hover:bg-red-900/70 text-red-300 border border-red-800 rounded"
                    >
                      Delete Domain
                    </button>
                  </div>
                </div>
              </div>

              {/* Mailboxes */}
              <div className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-4">
                <h4 className="text-sm font-semibold text-white">Mailboxes</h4>
                <table className="w-full text-left border-collapse">
                  <thead>
                    <tr className="border-b border-slate-800 text-xs font-medium text-slate-400 uppercase">
                      <th className="py-2 px-3">Address</th>
                      <th className="py-2 px-3">Quota</th>
                      <th className="py-2 px-3">Status</th>
                      <th className="py-2 px-3 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-800/60 text-sm">
                    {mailboxes.length === 0 ? (
                      <tr>
                        <td colSpan={4} className="py-4 text-center text-slate-500">
                          No mailboxes configured for this domain.
                        </td>
                      </tr>
                    ) : (
                      mailboxes.map((mb) => (
                        <tr key={mb.id}>
                          <td className="py-2 px-3 font-mono text-xs text-slate-200">
                            {mb.address}
                          </td>
                          <td className="py-2 px-3 font-mono text-xs text-slate-300">
                            {Math.round(mb.quota_bytes / MIB)} MiB
                          </td>
                          <td className="py-2 px-3 text-xs text-slate-300">
                            {mb.active ? 'active' : 'disabled'}
                          </td>
                          <td className="py-2 px-3 text-right space-x-2">
                            <button
                              type="button"
                              onClick={() => {
                                setEditingMailboxId(mb.id);
                                setMailboxLocalPart(mb.local_part);
                                setMailboxPassword('');
                                setMailboxQuotaMb(String(Math.round(mb.quota_bytes / MIB)));
                                setMailboxActive(mb.active);
                                setMailboxValidationError(null);
                              }}
                              className="text-xs text-sky-400 hover:text-sky-300"
                            >
                              Edit
                            </button>
                            <button
                              type="button"
                              onClick={() =>
                                setConfirmTarget({
                                  kind: 'mailbox',
                                  id: mb.id,
                                  label: mb.address,
                                })
                              }
                              className="text-xs text-red-400 hover:text-red-300"
                            >
                              Delete
                            </button>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>

                <form onSubmit={handleMailboxSubmit} noValidate className="grid grid-cols-1 md:grid-cols-4 gap-3 pt-2 border-t border-slate-800">
                  {mailboxValidationError && (
                    <div className="md:col-span-4 p-2 bg-red-950/60 border border-red-800 rounded text-xs text-red-300">
                      {mailboxValidationError}
                    </div>
                  )}
                  <div>
                    <label htmlFor="mailbox-local-part" className="block text-xs text-slate-400 mb-1">
                      Mailbox Local Part
                    </label>
                    <input
                      id="mailbox-local-part"
                      type="text"
                      value={mailboxLocalPart}
                      onChange={(e) => setMailboxLocalPart(e.target.value)}
                      disabled={Boolean(editingMailboxId)}
                      placeholder="alice"
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono disabled:opacity-60"
                    />
                  </div>
                  <div>
                    <label htmlFor="mailbox-password" className="block text-xs text-slate-400 mb-1">
                      {editingMailboxId ? 'New Password (Optional)' : 'Mailbox Password'}
                    </label>
                    <input
                      id="mailbox-password"
                      type="password"
                      value={mailboxPassword}
                      onChange={(e) => setMailboxPassword(e.target.value)}
                      placeholder="Min 8 characters"
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                    />
                  </div>
                  <div>
                    <label htmlFor="mailbox-quota" className="block text-xs text-slate-400 mb-1">
                      Quota (MiB)
                    </label>
                    <input
                      id="mailbox-quota"
                      type="number"
                      min={1}
                      value={mailboxQuotaMb}
                      onChange={(e) => setMailboxQuotaMb(e.target.value)}
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                    />
                  </div>
                  <div className="flex items-end justify-between gap-2">
                    <label className="flex items-center gap-1.5 text-xs text-slate-300 pb-2">
                      <input
                        type="checkbox"
                        checked={mailboxActive}
                        onChange={(e) => setMailboxActive(e.target.checked)}
                      />
                      <span>Active</span>
                    </label>
                    <div className="flex gap-2">
                      {editingMailboxId && (
                        <button
                          type="button"
                          onClick={() => {
                            setEditingMailboxId(null);
                            setMailboxLocalPart('');
                            setMailboxPassword('');
                            setMailboxQuotaMb('1024');
                            setMailboxActive(true);
                          }}
                          className="px-2.5 py-1.5 text-xs text-slate-400 hover:text-slate-200"
                        >
                          Cancel
                        </button>
                      )}
                      <button
                        type="submit"
                        disabled={isSubmitting}
                        className="px-3 py-1.5 bg-sky-600 hover:bg-sky-500 text-white text-xs font-medium rounded disabled:opacity-50"
                      >
                        {editingMailboxId ? 'Update Mailbox' : 'Add Mailbox'}
                      </button>
                    </div>
                  </div>
                </form>
              </div>

              {/* Local Aliases */}
              <div className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-4">
                <h4 className="text-sm font-semibold text-white">Local Aliases</h4>
                <table className="w-full text-left border-collapse">
                  <thead>
                    <tr className="border-b border-slate-800 text-xs font-medium text-slate-400 uppercase">
                      <th className="py-2 px-3">Source</th>
                      <th className="py-2 px-3">Destinations</th>
                      <th className="py-2 px-3">Status</th>
                      <th className="py-2 px-3 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-800/60 text-sm">
                    {aliases.length === 0 ? (
                      <tr>
                        <td colSpan={4} className="py-4 text-center text-slate-500">
                          No aliases configured for this domain.
                        </td>
                      </tr>
                    ) : (
                      aliases.map((al) => (
                        <tr key={al.id}>
                          <td className="py-2 px-3 font-mono text-xs text-slate-200">
                            {al.source}
                          </td>
                          <td className="py-2 px-3 font-mono text-xs text-slate-300">
                            {al.destinations.join(', ')}
                          </td>
                          <td className="py-2 px-3 text-xs text-slate-300">
                            {al.active ? 'active' : 'disabled'}
                          </td>
                          <td className="py-2 px-3 text-right space-x-2">
                            <button
                              type="button"
                              onClick={() => {
                                setEditingAliasId(al.id);
                                setAliasLocalPart(al.source.split('@')[0] || '');
                                setAliasDestinations(al.destinations.join(', '));
                                setAliasActive(al.active);
                                setAliasValidationError(null);
                              }}
                              className="text-xs text-sky-400 hover:text-sky-300"
                            >
                              Edit
                            </button>
                            <button
                              type="button"
                              onClick={() =>
                                setConfirmTarget({
                                  kind: 'alias',
                                  id: al.id,
                                  label: al.source,
                                })
                              }
                              className="text-xs text-red-400 hover:text-red-300"
                            >
                              Delete
                            </button>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>

                <form onSubmit={handleAliasSubmit} noValidate className="grid grid-cols-1 md:grid-cols-3 gap-3 pt-2 border-t border-slate-800">
                  {aliasValidationError && (
                    <div className="md:col-span-3 p-2 bg-red-950/60 border border-red-800 rounded text-xs text-red-300">
                      {aliasValidationError}
                    </div>
                  )}
                  <div>
                    <label htmlFor="alias-local-part" className="block text-xs text-slate-400 mb-1">
                      Alias Local Part
                    </label>
                    <input
                      id="alias-local-part"
                      type="text"
                      value={aliasLocalPart}
                      onChange={(e) => setAliasLocalPart(e.target.value)}
                      disabled={Boolean(editingAliasId)}
                      placeholder="support"
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono disabled:opacity-60"
                    />
                  </div>
                  <div>
                    <label htmlFor="alias-destinations" className="block text-xs text-slate-400 mb-1">
                      Destinations (comma-separated)
                    </label>
                    <input
                      id="alias-destinations"
                      type="text"
                      value={aliasDestinations}
                      onChange={(e) => setAliasDestinations(e.target.value)}
                      placeholder="alice@example.com"
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                    />
                  </div>
                  <div className="flex items-end justify-between gap-2">
                    <label className="flex items-center gap-1.5 text-xs text-slate-300 pb-2">
                      <input
                        type="checkbox"
                        checked={aliasActive}
                        onChange={(e) => setAliasActive(e.target.checked)}
                      />
                      <span>Active</span>
                    </label>
                    <div className="flex gap-2">
                      {editingAliasId && (
                        <button
                          type="button"
                          onClick={() => {
                            setEditingAliasId(null);
                            setAliasLocalPart('');
                            setAliasDestinations('');
                            setAliasActive(true);
                          }}
                          className="px-2.5 py-1.5 text-xs text-slate-400 hover:text-slate-200"
                        >
                          Cancel
                        </button>
                      )}
                      <button
                        type="submit"
                        disabled={isSubmitting}
                        className="px-3 py-1.5 bg-sky-600 hover:bg-sky-500 text-white text-xs font-medium rounded disabled:opacity-50"
                      >
                        {editingAliasId ? 'Update Alias' : 'Add Alias'}
                      </button>
                    </div>
                  </div>
                </form>
              </div>

              {/* DKIM Configuration & DNS Publication */}
              <div className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <h4 className="text-sm font-semibold text-white">DKIM Signing &amp; DNS Publication</h4>
                    {dkim && (
                      <span
                        className={`text-xs px-2 py-0.5 rounded border ${STATE_BADGE[dkim.state]}`}
                      >
                        {dkim.state}
                      </span>
                    )}
                  </div>
                  {dkim && (
                    <button
                      type="button"
                      onClick={() =>
                        setConfirmTarget({
                          kind: 'dkim',
                          id: selectedDomain.id,
                          label: `${dkim.selector}._domainkey.${selectedDomain.domain}`,
                        })
                      }
                      className="px-2.5 py-1 text-xs bg-red-950/60 hover:bg-red-900/70 text-red-300 border border-red-800 rounded"
                    >
                      Remove DKIM
                    </button>
                  )}
                </div>

                {dkim && (
                  <div className="p-3 bg-slate-950/60 border border-slate-800 rounded space-y-2 text-xs">
                    <div className="flex flex-wrap gap-x-4 gap-y-1 text-slate-400">
                      <span>
                        Selector: <span className="font-mono text-slate-200">{dkim.selector}</span>
                      </span>
                      <span>Queued Rev: {dkim.queued_revision ?? '—'}</span>
                      <span>Applied Rev: {dkim.applied_revision ?? '—'}</span>
                      <span>Failed Rev: {dkim.failed_revision ?? '—'}</span>
                      {dkim.pending_job_id && (
                        <span className="font-mono text-amber-300">
                          Pending Job: {dkim.pending_job_id}
                        </span>
                      )}
                    </div>
                    {dkim.last_error && <p className="text-red-400">{dkim.last_error}</p>}
                    <div>
                      <span className="text-slate-400">Public TXT Record:</span>
                      <pre className="mt-1 p-2 bg-slate-900 border border-slate-800 rounded font-mono text-slate-200 break-all whitespace-pre-wrap">
                        {dkim.txt_record || 'Pending agent key generation...'}
                      </pre>
                    </div>
                    {ownedDkimZone && dkimPublicationStatus && (
                      <div className="flex flex-wrap items-center gap-2 pt-1">
                        <span className="text-slate-400">
                          Owned DNS Zone:{' '}
                          <span className="font-mono text-slate-200">{ownedDkimZone.domain}</span> (
                          {ownedDkimZone.state})
                        </span>
                        <span className={`px-2 py-0.5 rounded border ${dkimPublicationStatus.tone}`}>
                          {dkimPublicationStatus.label}
                        </span>
                      </div>
                    )}
                  </div>
                )}

                <form onSubmit={handleDkimSubmit} noValidate className="grid grid-cols-1 md:grid-cols-4 gap-3">
                  {dkimValidationError && (
                    <div className="md:col-span-4 p-2 bg-red-950/60 border border-red-800 rounded text-xs text-red-300">
                      {dkimValidationError}
                    </div>
                  )}
                  <div>
                    <label htmlFor="dkim-selector" className="block text-xs text-slate-400 mb-1">
                      DKIM Selector
                    </label>
                    <input
                      id="dkim-selector"
                      type="text"
                      value={dkimSelector}
                      onChange={(e) => setDkimSelector(e.target.value)}
                      placeholder="default"
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                    />
                  </div>
                  <div>
                    <label htmlFor="dkim-dns-zone" className="block text-xs text-slate-400 mb-1">
                      Publish to DNS Zone
                    </label>
                    <select
                      id="dkim-dns-zone"
                      value={dkimDnsZoneId}
                      onChange={(e) => setDkimDnsZoneId(e.target.value)}
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                    >
                      <option value="">None</option>
                      {dnsZones.map((zone) => (
                        <option key={zone.id} value={zone.id}>
                          {zone.domain} ({zone.state})
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="flex items-end gap-4 pb-1.5">
                    <label className="flex items-center gap-1.5 text-xs text-slate-300">
                      <input
                        type="checkbox"
                        checked={dkimEnabled}
                        onChange={(e) => setDkimEnabled(e.target.checked)}
                      />
                      <span>Enabled</span>
                    </label>
                    <label className="flex items-center gap-1.5 text-xs text-slate-300">
                      <input
                        type="checkbox"
                        checked={dkimRotate}
                        onChange={(e) => setDkimRotate(e.target.checked)}
                      />
                      <span>Rotate Key</span>
                    </label>
                  </div>
                  <div className="flex items-end justify-end">
                    <button
                      type="submit"
                      disabled={isSubmitting}
                      className="px-3 py-1.5 bg-sky-600 hover:bg-sky-500 text-white text-xs font-medium rounded disabled:opacity-50"
                    >
                      Save DKIM
                    </button>
                  </div>
                </form>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default MailView;
