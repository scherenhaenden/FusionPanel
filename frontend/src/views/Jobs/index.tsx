import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../../api';
import { JobLogEntry, JobRecord, JobState } from '../../types';

const messageFromError = (error: unknown): string => {
  const value = error as { response?: { data?: { error?: { message?: string } } }; message?: string };
  return value.response?.data?.error?.message || value.message || 'The request could not be completed.';
};

const formatDate = (value: string): string => {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
};

const activeStates: JobState[] = ['queued', 'running'];
const POLL_INTERVAL_MS = 3000;
const RETRY_INTERVAL_MS = 5000;

const stateStyles: Record<JobState, string> = {
  queued: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200',
  running: 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200',
  succeeded: 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200',
  failed: 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-200',
  cancelled: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200',
};

const JobStatus: React.FC<{ state: JobState }> = ({ state }) => (
  <span className={`inline-flex rounded-full px-2.5 py-1 text-xs font-medium ${stateStyles[state]}`}>
    {state}
  </span>
);

export const JobsView: React.FC = () => {
  const [jobs, setJobs] = useState<JobRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isCreating, setIsCreating] = useState(false);
  const [jobsError, setJobsError] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<JobRecord | null>(null);
  const [logs, setLogs] = useState<JobLogEntry[]>([]);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsError, setDetailsError] = useState<string | null>(null);
  const [detailsRetry, setDetailsRetry] = useState(0);

  const loadJobs = useCallback(async () => {
    setJobsError(null);
    setIsLoading(true);
    try {
      setJobs(await api.getJobs());
    } catch (loadError) {
      setJobsError(messageFromError(loadError));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadJobs();
  }, [loadJobs]);

  useEffect(() => {
    if (!selectedJobId) return undefined;

    let current = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const [job, jobLogs] = await Promise.all([
          api.getJob(selectedJobId),
          api.getJobLogs(selectedJobId),
        ]);
        if (!current) return;
        setSelectedJob(job);
        setJobs((currentJobs) => {
          const alreadyListed = currentJobs.some((currentJob) => currentJob.id === job.id);
          return alreadyListed
            ? currentJobs.map((currentJob) => (currentJob.id === job.id ? job : currentJob))
            : [job, ...currentJobs];
        });
        setLogs(jobLogs);
        setDetailsError(null);
        if (activeStates.includes(job.state)) {
          timer = setTimeout(() => void poll(), POLL_INTERVAL_MS);
        }
      } catch (loadError) {
        if (current) {
          setDetailsError(messageFromError(loadError));
          if (!selectedJob || activeStates.includes(selectedJob.state)) {
            timer = setTimeout(() => void poll(), RETRY_INTERVAL_MS);
          }
        }
      } finally {
        if (current) setDetailsLoading(false);
      }
    };

    setDetailsLoading(true);
    setDetailsError(null);
    void poll();
    return () => {
      current = false;
      if (timer) clearTimeout(timer);
    };
  }, [selectedJobId, detailsRetry]);

  const runSystemInfo = async () => {
    setIsCreating(true);
    setCreateError(null);
    try {
      const job = await api.createSystemInfoJob();
      setSelectedJob(job);
      setSelectedJobId(job.id);
      setJobs((currentJobs) => [job, ...currentJobs.filter((currentJob) => currentJob.id !== job.id)]);
      setLogs([]);
      await loadJobs();
    } catch (createJobError) {
      setCreateError(messageFromError(createJobError));
    } finally {
      setIsCreating(false);
    }
  };

  const selectJob = (job: JobRecord) => {
    setSelectedJob(job);
    setSelectedJobId(job.id);
  };

  return (
    <main className="space-y-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-gray-900 dark:text-white">Jobs</h1>
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
            Queue a supported agent operation and follow its persisted status and logs.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void loadJobs()}
            disabled={isLoading}
            className="rounded border border-gray-300 px-4 py-2 text-gray-800 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-gray-600 dark:text-gray-100 dark:hover:bg-gray-700"
          >
            {isLoading ? 'Refreshing…' : 'Refresh jobs'}
          </button>
          <button
            type="button"
            onClick={() => void runSystemInfo()}
            disabled={isCreating}
            className="rounded bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isCreating ? 'Queueing…' : 'Run System Info Job'}
          </button>
        </div>
      </header>

      {createError && <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">Could not queue the job: {createError}</div>}
      {jobsError && (
        <div role="alert" className="rounded border border-red-300 bg-red-50 px-4 py-3 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200">
          <p>{jobsError}</p>
          <button type="button" onClick={() => void loadJobs()} className="mt-2 underline">Retry</button>
        </div>
      )}

      <section aria-labelledby="jobs-list-heading" className="overflow-hidden rounded-lg border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-800">
        <div className="border-b border-gray-200 px-5 py-4 dark:border-gray-700">
          <h2 id="jobs-list-heading" className="font-semibold text-gray-900 dark:text-white">Job history</h2>
        </div>
        {isLoading && jobs.length === 0 ? (
          <p role="status" className="p-6 text-gray-600 dark:text-gray-300">Loading jobs…</p>
        ) : jobs.length === 0 ? (
          <div className="p-6">
            <p className="font-medium text-gray-900 dark:text-white">No jobs yet.</p>
            <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">Use “Run System Info Job” to enqueue a supported operation.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-200 text-left dark:divide-gray-700">
              <thead className="bg-gray-50 text-xs uppercase tracking-wide text-gray-500 dark:bg-gray-900 dark:text-gray-400">
                <tr>
                  <th scope="col" className="px-5 py-3">Job</th>
                  <th scope="col" className="px-5 py-3">Status</th>
                  <th scope="col" className="px-5 py-3">Created</th>
                  <th scope="col" className="px-5 py-3">Worker</th>
                  <th scope="col" className="px-5 py-3"><span className="sr-only">Actions</span></th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {jobs.map((job) => (
                  <tr key={job.id} className={selectedJobId === job.id ? 'bg-blue-50 dark:bg-gray-700/50' : undefined}>
                    <td className="px-5 py-4">
                      <div className="font-medium text-gray-900 dark:text-white">{job.kind}</div>
                      <div className="mt-1 font-mono text-xs text-gray-500 dark:text-gray-400">{job.id}</div>
                    </td>
                    <td className="px-5 py-4"><JobStatus state={job.state} /></td>
                    <td className="px-5 py-4 text-sm text-gray-700 dark:text-gray-200">{formatDate(job.created_at)}</td>
                    <td className="px-5 py-4 text-sm text-gray-700 dark:text-gray-200">{job.worker_id || '—'}</td>
                    <td className="px-5 py-4 text-right">
                      <button type="button" onClick={() => selectJob(job)} className="text-sm font-medium text-blue-700 underline dark:text-blue-300">
                        View details
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {selectedJobId && (
        <section aria-labelledby="job-details-heading" className="rounded-lg border border-gray-200 bg-white p-5 shadow-sm dark:border-gray-700 dark:bg-gray-800">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <h2 id="job-details-heading" className="font-semibold text-gray-900 dark:text-white">Job details</h2>
              <p className="mt-1 break-all font-mono text-xs text-gray-500 dark:text-gray-400">{selectedJobId}</p>
            </div>
            {selectedJob && <JobStatus state={selectedJob.state} />}
          </div>

          {detailsLoading && !selectedJob && <p role="status" className="mt-4 text-sm text-gray-600 dark:text-gray-300">Loading job details…</p>}
          {detailsError && (
            <div role="alert" className="mt-4 text-sm text-red-700 dark:text-red-300">
              <p>{detailsError}</p>
              <button type="button" onClick={() => setDetailsRetry((attempt) => attempt + 1)} className="mt-1 underline">Retry details</button>
            </div>
          )}
          {selectedJob && (
            <div className="mt-4 grid gap-5 md:grid-cols-2">
              <div className="space-y-3">
                <div><span className="text-sm text-gray-500 dark:text-gray-400">State</span><div className="mt-1"><JobStatus state={selectedJob.state} /></div></div>
                <div><span className="text-sm text-gray-500 dark:text-gray-400">Worker</span><p className="mt-1 break-all text-sm text-gray-900 dark:text-white">{selectedJob.worker_id || 'Not claimed'}</p></div>
                <div><span className="text-sm text-gray-500 dark:text-gray-400">Updated</span><p className="mt-1 text-sm text-gray-900 dark:text-white">{formatDate(selectedJob.updated_at)}</p></div>
                {selectedJob.error_text && <div role="alert"><span className="text-sm text-gray-500 dark:text-gray-400">Error</span><p className="mt-1 whitespace-pre-wrap text-sm text-red-700 dark:text-red-300">{selectedJob.error_text}</p></div>}
                <div><span className="text-sm text-gray-500 dark:text-gray-400">Request</span><pre className="mt-1 overflow-x-auto rounded bg-gray-50 p-3 text-xs text-gray-800 dark:bg-gray-900 dark:text-gray-200">{JSON.stringify(selectedJob.payload, null, 2)}</pre></div>
              </div>

              <div>
                <h3 className="text-sm font-medium text-gray-900 dark:text-white">Execution logs</h3>
                {detailsLoading && logs.length === 0 ? (
                  <p role="status" className="mt-2 text-sm text-gray-600 dark:text-gray-300">Loading logs…</p>
                ) : logs.length === 0 ? (
                  <p className="mt-2 text-sm text-gray-600 dark:text-gray-300">No logs have been reported for this job.</p>
                ) : (
                  <ol className="mt-2 max-h-96 space-y-3 overflow-auto rounded bg-gray-50 p-3 dark:bg-gray-900">
                    {logs.map((log) => (
                      <li key={log.sequence} className="border-b border-gray-200 pb-3 last:border-0 last:pb-0 dark:border-gray-700">
                        <div className="mb-1 flex flex-wrap justify-between gap-2 text-xs text-gray-500 dark:text-gray-400">
                          <span className="font-medium uppercase">{log.stream}</span>
                          <time dateTime={log.created_at}>{formatDate(log.created_at)}</time>
                        </div>
                        <pre className="whitespace-pre-wrap break-words text-xs text-gray-800 dark:text-gray-200">{log.text}</pre>
                      </li>
                    ))}
                  </ol>
                )}
              </div>
            </div>
          )}
          {selectedJob && activeStates.includes(selectedJob.state) && (
            <p className="mt-4 text-xs text-gray-500 dark:text-gray-400">Status and logs refresh automatically while this job is queued or running.</p>
          )}
        </section>
      )}
    </main>
  );
};

export default JobsView;
