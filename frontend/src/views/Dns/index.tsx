import React, { useCallback, useEffect, useState } from 'react';
import {
  api,
  DnsRecordEntry,
  DnsRecordType,
  DnsZoneRecord,
  DnsZoneState,
} from '../../api';

const STATE_BADGE: Record<DnsZoneState, string> = {
  provisioning: 'bg-amber-950/60 text-amber-300 border-amber-800',
  ready: 'bg-emerald-950/60 text-emerald-300 border-emerald-800',
  failed: 'bg-red-950/60 text-red-300 border-red-800',
  deleting: 'bg-slate-800 text-slate-300 border-slate-700',
};

export const DnsView: React.FC = () => {
  const [zones, setZones] = useState<DnsZoneRecord[]>([]);
  const [selectedZoneId, setSelectedZoneId] = useState<string | null>(null);
  const [selectedZone, setSelectedZone] = useState<DnsZoneRecord | null>(null);
  const [records, setRecords] = useState<DnsRecordEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Create zone form
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [newDomain, setNewDomain] = useState('');
  const [newDefaultTtl, setNewDefaultTtl] = useState('300');

  // Record form (create or edit)
  const [editingRecordId, setEditingRecordId] = useState<string | null>(null);
  const [recName, setRecName] = useState('@');
  const [recType, setRecType] = useState<DnsRecordType>('A');
  const [recValue, setRecValue] = useState('');
  const [recTtl, setRecTtl] = useState('300');
  const [recPriority, setRecPriority] = useState('10');

  // DKIM quick-sync form
  const [dkimSelector, setDkimSelector] = useState('default');
  const [dkimValue, setDkimValue] = useState('');
  const [dkimTtl, setDkimTtl] = useState('300');

  // Propagation probe form
  const [probeName, setProbeName] = useState('default._domainkey');
  const [probeType, setProbeType] = useState<DnsRecordType>('TXT');
  const [probeSubstring, setProbeSubstring] = useState('v=DKIM1;');

  const loadZones = useCallback(async () => {
    try {
      const list = await api.getDnsZones();
      const normalized = Array.isArray(list) ? list : [];
      setZones(normalized);
      setError(null);
      setSelectedZoneId((prev) => prev ?? (normalized[0]?.id ?? null));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load DNS zones.');
    } finally {
      setLoading(false);
    }
  }, []);

  const loadZoneDetail = useCallback(async (zoneId: string) => {
    try {
      const detail = await api.getDnsZone(zoneId);
      setSelectedZone(detail.zone);
      setRecords(Array.isArray(detail.records) ? detail.records : []);
      setZones((prev) => prev.map((z) => (z.id === detail.zone.id ? detail.zone : z)));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load DNS zone details.');
    }
  }, []);

  useEffect(() => {
    void loadZones();
  }, [loadZones]);

  useEffect(() => {
    if (selectedZoneId) {
      void loadZoneDetail(selectedZoneId);
    } else {
      setSelectedZone(null);
      setRecords([]);
    }
  }, [selectedZoneId, loadZoneDetail]);

  const hasPendingZone = zones.some(
    (z) => z.state === 'provisioning' || z.state === 'deleting',
  );

  useEffect(() => {
    if (!hasPendingZone) return;
    const timer = window.setInterval(() => {
      void loadZones();
      if (selectedZoneId) {
        void loadZoneDetail(selectedZoneId);
      }
    }, 1500);
    return () => window.clearInterval(timer);
  }, [hasPendingZone, selectedZoneId, loadZones, loadZoneDetail]);

  const handleCreateZone = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const res = await api.createDnsZone({
        domain: newDomain.trim(),
        default_ttl: Number(newDefaultTtl) || 300,
      });
      setNewDomain('');
      setIsCreateOpen(false);
      setZones((prev) => [res.zone, ...prev]);
      setSelectedZoneId(res.zone.id);
      setSelectedZone(res.zone);
      setRecords(res.records ?? []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create DNS zone.');
    }
  };

  const handleDeleteZone = async (zoneId: string) => {
    setError(null);
    try {
      await api.deleteDnsZone(zoneId);
      await loadZones();
      if (selectedZoneId === zoneId) {
        setSelectedZoneId(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete DNS zone.');
    }
  };

  const handleSubmitRecord = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedZoneId) return;
    setError(null);
    const payload = {
      name: recName.trim(),
      record_type: recType,
      value: recValue.trim(),
      ttl: Number(recTtl) || 300,
      ...(recType === 'MX' ? { priority: Number(recPriority) || 10 } : {}),
    };
    try {
      const res = editingRecordId
        ? await api.updateDnsRecord(selectedZoneId, editingRecordId, payload)
        : await api.createDnsRecord(selectedZoneId, payload);
      setSelectedZone(res.zone);
      if (res.records) setRecords(res.records);
      setEditingRecordId(null);
      setRecName('@');
      setRecValue('');
      await loadZones();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save DNS record.');
    }
  };

  const handleDeleteRecord = async (recordId: string) => {
    if (!selectedZoneId) return;
    setError(null);
    try {
      const res = await api.deleteDnsRecord(selectedZoneId, recordId);
      setSelectedZone(res.zone);
      if (res.records) setRecords(res.records);
      await loadZones();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete DNS record.');
    }
  };

  const handleSyncDkim = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedZoneId) return;
    setError(null);
    try {
      const res = await api.upsertDkimRecord(selectedZoneId, {
        selector: dkimSelector.trim(),
        value: dkimValue.trim(),
        ttl: Number(dkimTtl) || 300,
      });
      setSelectedZone(res.zone);
      if (res.records) setRecords(res.records);
      await loadZones();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to sync DKIM TXT record.');
    }
  };

  const handleProbe = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedZoneId) return;
    setError(null);
    try {
      await api.probeDnsZone(selectedZoneId, {
        record_name: probeName.trim(),
        record_type: probeType,
        ...(probeSubstring.trim() ? { expected_substring: probeSubstring.trim() } : {}),
      });
      window.setTimeout(() => {
        void loadZoneDetail(selectedZoneId);
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to probe DNS propagation.');
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <div>
          <h2 className="text-xl font-semibold text-white">DNS Zones & DKIM Sync</h2>
          <p className="text-sm text-slate-400">
            Manage authoritative DNS zones, A/AAAA/CNAME/MX/TXT records, DKIM keys, and live propagation probes.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setIsCreateOpen(true)}
          className="px-4 py-2 bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium rounded-md"
        >
          Create Zone
        </button>
      </div>

      {error && (
        <div className="p-3 bg-red-950/60 border border-red-800 rounded text-sm text-red-300">
          {error}
        </div>
      )}

      {isCreateOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
          role="dialog"
          aria-modal="true"
          aria-labelledby="create-dns-zone-title"
        >
          <div className="bg-slate-900 border border-slate-800 rounded-lg max-w-md w-full p-6">
            <h3 id="create-dns-zone-title" className="text-lg font-medium text-white mb-4">
              Create DNS Zone
            </h3>
            <form onSubmit={handleCreateZone} className="space-y-4">
              <div>
                <label htmlFor="dns-zone-domain" className="block text-sm text-slate-300 mb-1">
                  Domain
                </label>
                <input
                  id="dns-zone-domain"
                  type="text"
                  value={newDomain}
                  onChange={(e) => setNewDomain(e.target.value)}
                  placeholder="example.com"
                  required
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
              <div>
                <label htmlFor="dns-zone-ttl" className="block text-sm text-slate-300 mb-1">
                  Default TTL (seconds)
                </label>
                <input
                  id="dns-zone-ttl"
                  type="number"
                  min={30}
                  value={newDefaultTtl}
                  onChange={(e) => setNewDefaultTtl(e.target.value)}
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                />
              </div>
              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setIsCreateOpen(false)}
                  className="px-4 py-2 text-sm text-slate-400 hover:text-slate-200"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="px-4 py-2 bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium rounded"
                >
                  Save Zone
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {loading ? (
        <div className="p-8 text-center text-slate-400">Loading DNS zones...</div>
      ) : zones.length === 0 ? (
        <div className="p-10 text-center bg-slate-900 border border-slate-800 rounded-lg text-slate-400">
          No DNS zones configured yet.
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
          <div className="bg-slate-900 border border-slate-800 rounded-lg p-3 space-y-2">
            <h3 className="text-xs font-semibold text-slate-400 uppercase tracking-wider px-2">
              Zones
            </h3>
            <ul className="space-y-1">
              {zones.map((zone) => (
                <li key={zone.id}>
                  <button
                    type="button"
                    onClick={() => setSelectedZoneId(zone.id)}
                    className={`w-full text-left px-3 py-2 rounded text-sm flex items-center justify-between ${
                      selectedZoneId === zone.id
                        ? 'bg-slate-800 text-white'
                        : 'text-slate-300 hover:bg-slate-800/50'
                    }`}
                  >
                    <span className="font-mono truncate">{zone.domain}</span>
                    <span
                      className={`text-[11px] px-1.5 py-0.5 rounded border ${STATE_BADGE[zone.state]}`}
                    >
                      {zone.state}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </div>

          {selectedZone && (
            <div className="lg:col-span-3 space-y-6">
              <div className="bg-slate-900 border border-slate-800 rounded-lg p-4 flex flex-wrap items-center justify-between gap-4">
                <div>
                  <div className="flex items-center gap-3">
                    <h3 className="text-lg font-mono font-semibold text-white">
                      {selectedZone.domain}
                    </h3>
                    <span
                      className={`text-xs px-2 py-0.5 rounded border ${STATE_BADGE[selectedZone.state]}`}
                    >
                      {selectedZone.state}
                    </span>
                  </div>
                  <p className="text-xs text-slate-400 mt-1">
                    SOA Serial: <span className="font-mono text-slate-200">{selectedZone.serial}</span>
                    {' · '}Default TTL: <span className="font-mono text-slate-200">{selectedZone.default_ttl}s</span>
                  </p>
                  {selectedZone.last_probe_result && (
                    <p className="text-xs text-emerald-300 font-mono mt-1">
                      Last Probe: {selectedZone.last_probe_result}
                    </p>
                  )}
                  {selectedZone.last_error && (
                    <p className="text-xs text-red-400 mt-1">{selectedZone.last_error}</p>
                  )}
                </div>
                <button
                  type="button"
                  onClick={() => void handleDeleteZone(selectedZone.id)}
                  className="px-3 py-1.5 text-xs bg-red-950/60 hover:bg-red-900/70 text-red-300 border border-red-800 rounded"
                >
                  Delete Zone
                </button>
              </div>

              <div className="bg-slate-900 border border-slate-800 rounded-lg overflow-hidden">
                <table className="w-full text-left border-collapse">
                  <thead>
                    <tr className="border-b border-slate-800 bg-slate-950/50 text-xs font-medium text-slate-400 uppercase">
                      <th className="py-2.5 px-4">Name</th>
                      <th className="py-2.5 px-4">Type</th>
                      <th className="py-2.5 px-4">Value</th>
                      <th className="py-2.5 px-4">TTL</th>
                      <th className="py-2.5 px-4 text-right">Actions</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-800/60 text-sm">
                    {records.length === 0 ? (
                      <tr>
                        <td colSpan={5} className="py-6 text-center text-slate-500">
                          No records in this zone yet.
                        </td>
                      </tr>
                    ) : (
                      records.map((rec) => (
                        <tr key={rec.id} className="hover:bg-slate-800/30">
                          <td className="py-2.5 px-4 font-mono text-xs text-slate-200">{rec.name}</td>
                          <td className="py-2.5 px-4 font-mono text-xs text-sky-300">
                            {rec.record_type}
                            {rec.priority !== null ? ` (${rec.priority})` : ''}
                          </td>
                          <td className="py-2.5 px-4 font-mono text-xs text-slate-300 break-all">
                            {rec.value}
                          </td>
                          <td className="py-2.5 px-4 font-mono text-xs text-slate-400">{rec.ttl}s</td>
                          <td className="py-2.5 px-4 text-right space-x-2">
                            <button
                              type="button"
                              onClick={() => {
                                setEditingRecordId(rec.id);
                                setRecName(rec.name);
                                setRecType(rec.record_type);
                                setRecValue(rec.value);
                                setRecTtl(String(rec.ttl));
                                if (rec.priority !== null) setRecPriority(String(rec.priority));
                              }}
                              className="text-xs text-sky-400 hover:text-sky-300"
                            >
                              Edit
                            </button>
                            <button
                              type="button"
                              onClick={() => void handleDeleteRecord(rec.id)}
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
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <form
                  onSubmit={handleSubmitRecord}
                  className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-3"
                >
                  <h4 className="text-sm font-semibold text-white">
                    {editingRecordId ? 'Update DNS Record' : 'Add DNS Record'}
                  </h4>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label htmlFor="dns-rec-name" className="block text-xs text-slate-400 mb-1">
                        Record Name
                      </label>
                      <input
                        id="dns-rec-name"
                        type="text"
                        value={recName}
                        onChange={(e) => setRecName(e.target.value)}
                        placeholder="@ or www"
                        required
                        className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                      />
                    </div>
                    <div>
                      <label htmlFor="dns-rec-type" className="block text-xs text-slate-400 mb-1">
                        Record Type
                      </label>
                      <select
                        id="dns-rec-type"
                        value={recType}
                        onChange={(e) => setRecType(e.target.value as DnsRecordType)}
                        className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                      >
                        <option value="A">A</option>
                        <option value="AAAA">AAAA</option>
                        <option value="CNAME">CNAME</option>
                        <option value="MX">MX</option>
                        <option value="TXT">TXT</option>
                      </select>
                    </div>
                  </div>
                  <div>
                    <label htmlFor="dns-rec-value" className="block text-xs text-slate-400 mb-1">
                      Record Value
                    </label>
                    <input
                      id="dns-rec-value"
                      type="text"
                      value={recValue}
                      onChange={(e) => setRecValue(e.target.value)}
                      placeholder="203.0.113.10"
                      required
                      className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <label htmlFor="dns-rec-ttl" className="block text-xs text-slate-400 mb-1">
                        TTL
                      </label>
                      <input
                        id="dns-rec-ttl"
                        type="number"
                        min={30}
                        value={recTtl}
                        onChange={(e) => setRecTtl(e.target.value)}
                        className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                      />
                    </div>
                    {recType === 'MX' && (
                      <div>
                        <label htmlFor="dns-rec-priority" className="block text-xs text-slate-400 mb-1">
                          MX Priority
                        </label>
                        <input
                          id="dns-rec-priority"
                          type="number"
                          min={0}
                          value={recPriority}
                          onChange={(e) => setRecPriority(e.target.value)}
                          className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                        />
                      </div>
                    )}
                  </div>
                  <div className="flex gap-2">
                    <button
                      type="submit"
                      className="px-3 py-1.5 bg-sky-600 hover:bg-sky-500 text-white text-xs font-medium rounded"
                    >
                      {editingRecordId ? 'Save Record' : 'Add Record'}
                    </button>
                    {editingRecordId && (
                      <button
                        type="button"
                        onClick={() => {
                          setEditingRecordId(null);
                          setRecName('@');
                          setRecValue('');
                        }}
                        className="px-3 py-1.5 text-xs text-slate-400 hover:text-slate-200"
                      >
                        Cancel Edit
                      </button>
                    )}
                  </div>
                </form>

                <div className="space-y-6">
                  <form
                    onSubmit={handleSyncDkim}
                    className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-3"
                  >
                    <h4 className="text-sm font-semibold text-white">DKIM TXT Sync</h4>
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label htmlFor="dkim-selector" className="block text-xs text-slate-400 mb-1">
                          DKIM Selector
                        </label>
                        <input
                          id="dkim-selector"
                          type="text"
                          value={dkimSelector}
                          onChange={(e) => setDkimSelector(e.target.value)}
                          required
                          className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                        />
                      </div>
                      <div>
                        <label htmlFor="dkim-ttl" className="block text-xs text-slate-400 mb-1">
                          DKIM TTL
                        </label>
                        <input
                          id="dkim-ttl"
                          type="number"
                          min={30}
                          value={dkimTtl}
                          onChange={(e) => setDkimTtl(e.target.value)}
                          className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                        />
                      </div>
                    </div>
                    <div>
                      <label htmlFor="dkim-value" className="block text-xs text-slate-400 mb-1">
                        DKIM TXT Record
                      </label>
                      <input
                        id="dkim-value"
                        type="text"
                        value={dkimValue}
                        onChange={(e) => setDkimValue(e.target.value)}
                        placeholder="v=DKIM1; k=rsa; p=MIIBIjAN..."
                        required
                        className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                      />
                    </div>
                    <button
                      type="submit"
                      className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium rounded"
                    >
                      Sync DKIM
                    </button>
                  </form>

                  <form
                    onSubmit={handleProbe}
                    className="bg-slate-900 border border-slate-800 rounded-lg p-4 space-y-3"
                  >
                    <h4 className="text-sm font-semibold text-white">Propagation Probe</h4>
                    <div className="grid grid-cols-2 gap-3">
                      <div>
                        <label htmlFor="probe-name" className="block text-xs text-slate-400 mb-1">
                          Probe Owner
                        </label>
                        <input
                          id="probe-name"
                          type="text"
                          value={probeName}
                          onChange={(e) => setProbeName(e.target.value)}
                          required
                          className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                        />
                      </div>
                      <div>
                        <label htmlFor="probe-type" className="block text-xs text-slate-400 mb-1">
                          Probe Type
                        </label>
                        <select
                          id="probe-type"
                          value={probeType}
                          onChange={(e) => setProbeType(e.target.value as DnsRecordType)}
                          className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm"
                        >
                          <option value="A">A</option>
                          <option value="AAAA">AAAA</option>
                          <option value="CNAME">CNAME</option>
                          <option value="MX">MX</option>
                          <option value="TXT">TXT</option>
                        </select>
                      </div>
                    </div>
                    <div>
                      <label htmlFor="probe-expected" className="block text-xs text-slate-400 mb-1">
                        Expected Substring (optional)
                      </label>
                      <input
                        id="probe-expected"
                        type="text"
                        value={probeSubstring}
                        onChange={(e) => setProbeSubstring(e.target.value)}
                        className="w-full px-2.5 py-1.5 bg-slate-950 border border-slate-800 rounded text-slate-200 text-sm font-mono"
                      />
                    </div>
                    <button
                      type="submit"
                      className="px-3 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-medium rounded"
                    >
                      Probe Propagation
                    </button>
                  </form>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default DnsView;
