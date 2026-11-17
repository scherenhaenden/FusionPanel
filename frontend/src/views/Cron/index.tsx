import React, { useCallback, useEffect, useState } from 'react';
import { Clock, Play, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { api, CronJobRecord } from '../../api';

function extractErrorMessage(err: unknown, fallback: string): string {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const resp = (err as { response?: { data?: { error?: string; message?: string } } }).response;
    const msg = resp?.data?.message || resp?.data?.error;
    if (msg) return msg;
  }
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

export default function CronView() {
  const [jobs, setJobs] = useState<CronJobRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [pollingTicks, setPollingTicks] = useState(0);

  const [name, setName] = useState('');
  const [schedule, setSchedule] = useState('* * * * *');
  const [command, setCommand] = useState('');
  const [enabled, setEnabled] = useState(true);

  const loadJobs = useCallback(async () => {
    try {
      const list = await api.getCronJobs();
      setJobs(list);
      setError(null);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to load cron jobs'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadJobs();
  }, [loadJobs]);

  const hasPendingState = jobs.some(
    (job) => job.state === 'provisioning' || job.state === 'deleting',
  );

  useEffect(() => {
    if (!hasPendingState && pollingTicks <= 0) return;
    const timer = window.setTimeout(() => {
      if (pollingTicks > 0) {
        setPollingTicks((prev) => Math.max(0, prev - 1));
      }
      void loadJobs();
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [hasPendingState, pollingTicks, loadJobs]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !schedule.trim() || !command.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await api.createCronJob({
        name: name.trim(),
        schedule: schedule.trim(),
        command: command.trim(),
        enabled,
      });
      setName('');
      setSchedule('* * * * *');
      setCommand('');
      setEnabled(true);
      setShowCreate(false);
      await loadJobs();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to create cron job'));
    } finally {
      setBusy(false);
    }
  };

  const handleRunNow = async (cronId: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.runCronJobNow(cronId);
      setPollingTicks(8);
      await loadJobs();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to trigger cron job'));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (cronId: string) => {
    setBusy(true);
    setError(null);
    try {
      await api.deleteCronJob(cronId);
      await loadJobs();
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to delete cron job'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">Cron Jobs</h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Schedule recurring commands and trigger on-demand execution on your nodes.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() => void loadJobs()}
            className="inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
          >
            <RefreshCw className="w-4 h-4" />
            Refresh
          </button>
          <button
            type="button"
            onClick={() => setShowCreate((prev) => !prev)}
            className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            <Plus className="w-4 h-4" />
            New Cron Job
          </button>
        </div>
      </div>

      {error && (
        <div
          role="alert"
          className="rounded-lg border border-red-300 bg-red-50 px-4 py-3 text-sm text-red-800 dark:border-red-800 dark:bg-red-900/30 dark:text-red-200"
        >
          {error}
        </div>
      )}

      {showCreate && (
        <form
          onSubmit={handleCreate}
          className="rounded-lg border border-gray-200 bg-white p-5 space-y-4 dark:border-gray-700 dark:bg-gray-800"
        >
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">Create Cron Job</h2>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div>
              <label
                htmlFor="cron-name"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                Name
              </label>
              <input
                id="cron-name"
                type="text"
                required
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="nightly-cache"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label
                htmlFor="cron-schedule"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                Schedule
              </label>
              <input
                id="cron-schedule"
                type="text"
                required
                value={schedule}
                onChange={(e) => setSchedule(e.target.value)}
                placeholder="* * * * *"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
            <div>
              <label
                htmlFor="cron-command"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                Command
              </label>
              <input
                id="cron-command"
                type="text"
                required
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                placeholder="echo ok"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>
          </div>

          <label className="inline-flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="rounded border-gray-300"
            />
            Enabled
          </label>

          <div className="flex items-center gap-3">
            <button
              type="submit"
              disabled={busy}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              Create Cron Job
            </button>
            <button
              type="button"
              onClick={() => setShowCreate(false)}
              className="rounded-lg border border-gray-300 px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
            >
              Cancel
            </button>
          </div>
        </form>
      )}

      <div className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800 overflow-x-auto">
        {loading ? (
          <p className="p-6 text-sm text-gray-500 dark:text-gray-400">Loading cron jobs...</p>
        ) : jobs.length === 0 ? (
          <div className="p-6 text-center text-sm text-gray-500 dark:text-gray-400">
            No cron jobs configured yet.
          </div>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="bg-gray-50 text-xs uppercase text-gray-500 dark:bg-gray-700/50 dark:text-gray-400">
              <tr>
                <th className="px-4 py-3">Name</th>
                <th className="px-4 py-3">Schedule</th>
                <th className="px-4 py-3">Command</th>
                <th className="px-4 py-3">State</th>
                <th className="px-4 py-3">Last Run</th>
                <th className="px-4 py-3 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
              {jobs.map((job) => (
                <tr key={job.id} className="hover:bg-gray-50 dark:hover:bg-gray-700/40">
                  <td className="px-4 py-3 font-medium text-gray-900 dark:text-white">
                    <div className="flex items-center gap-2">
                      <Clock className="w-4 h-4 text-gray-400" />
                      <span>{job.name}</span>
                    </div>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-gray-700 dark:text-gray-300">
                    {job.schedule}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-gray-700 dark:text-gray-300">
                    {job.command}
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex rounded-full bg-gray-100 px-2.5 py-0.5 text-xs font-medium text-gray-800 dark:bg-gray-700 dark:text-gray-200">
                      {job.state}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-xs text-gray-600 dark:text-gray-300">
                    {job.last_run_status ? (
                      <span>{job.last_run_status}</span>
                    ) : (
                      <span className="text-gray-400">never</span>
                    )}
                    {job.last_error && (
                      <div className="text-red-600 dark:text-red-400">{job.last_error}</div>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right space-x-2">
                    <button
                      type="button"
                      disabled={busy || job.state === 'deleting'}
                      onClick={() => void handleRunNow(job.id)}
                      className="inline-flex items-center gap-1 rounded border border-blue-300 px-2.5 py-1 text-xs font-medium text-blue-700 hover:bg-blue-50 disabled:opacity-50 dark:border-blue-800 dark:text-blue-300 dark:hover:bg-blue-900/30"
                    >
                      <Play className="w-3.5 h-3.5" />
                      Run Now
                    </button>
                    <button
                      type="button"
                      disabled={busy || job.state === 'deleting'}
                      onClick={() => void handleDelete(job.id)}
                      className="inline-flex items-center gap-1 rounded border border-red-300 px-2.5 py-1 text-xs font-medium text-red-700 hover:bg-red-50 disabled:opacity-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900/30"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
