import React, { useCallback, useEffect, useState } from 'react';
import { Folder, FileText, Trash2, Edit3, RefreshCw, ArrowUp, Plus } from 'lucide-react';
import { api, FileEntry, WebsiteRecord } from '../../api';

function extractErrorMessage(err: unknown, fallback: string): string {
  if (typeof err === 'object' && err !== null && 'response' in err) {
    const resp = (err as { response?: { data?: { error?: unknown; message?: unknown } } }).response;
    const data = resp?.data;
    if (typeof data?.message === 'string' && data.message) return data.message;
    if (typeof data?.error === 'string' && data.error) return data.error;
    if (
      typeof data?.error === 'object' &&
      data.error !== null &&
      'message' in data.error &&
      typeof (data.error as { message?: unknown }).message === 'string'
    ) {
      return (data.error as { message: string }).message;
    }
  }
  if (err instanceof Error && err.message) return err.message;
  return fallback;
}

export default function FilesView() {
  const [sites, setSites] = useState<WebsiteRecord[]>([]);
  const [selectedSiteId, setSelectedSiteId] = useState<string>('');
  const [currentPath, setCurrentPath] = useState<string>('');
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [editorPath, setEditorPath] = useState('index.html');
  const [editorContent, setEditorContent] = useState('');

  const [renameFrom, setRenameFrom] = useState<string | null>(null);
  const [renameTo, setRenameTo] = useState('');

  const staticSites = sites.filter((s) => s.kind === 'static');

  const loadSites = useCallback(async () => {
    try {
      const list = await api.getSites();
      setSites(list);
      const statics = list.filter((s) => s.kind === 'static');
      if (statics.length > 0 && !selectedSiteId) {
        setSelectedSiteId(statics[0].id);
      }
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to load websites'));
    } finally {
      setLoading(false);
    }
  }, [selectedSiteId]);

  const loadEntries = useCallback(async (siteId: string, path: string) => {
    if (!siteId) {
      setEntries([]);
      return;
    }
    setError(null);
    try {
      const items = await api.listFiles(siteId, path);
      setEntries(items);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to list files'));
    }
  }, []);

  useEffect(() => {
    void loadSites();
  }, [loadSites]);

  useEffect(() => {
    if (selectedSiteId) {
      void loadEntries(selectedSiteId, currentPath);
    }
  }, [selectedSiteId, currentPath, loadEntries]);

  const handleSelectEntry = async (entry: FileEntry) => {
    if (!selectedSiteId) return;
    if (entry.kind === 'dir') {
      setCurrentPath(entry.path);
      return;
    }
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const res = await api.readFile(selectedSiteId, entry.path);
      setEditorPath(res.path);
      setEditorContent(res.content);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to read file'));
    } finally {
      setBusy(false);
    }
  };

  const handleSaveFile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedSiteId || !editorPath.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const res = await api.writeFile(selectedSiteId, editorPath.trim(), editorContent);
      setNotice(`Saved ${res.path} (${res.size} bytes)`);
      await loadEntries(selectedSiteId, currentPath);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to save file'));
    } finally {
      setBusy(false);
    }
  };

  const handleDeleteEntry = async (entry: FileEntry) => {
    if (!selectedSiteId) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await api.deleteFile(selectedSiteId, entry.path);
      setNotice(`Deleted ${entry.path}`);
      await loadEntries(selectedSiteId, currentPath);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to delete entry'));
    } finally {
      setBusy(false);
    }
  };

  const handleRenameSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedSiteId || !renameFrom || !renameTo.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const res = await api.renameFile(selectedSiteId, renameFrom, renameTo.trim());
      setNotice(`Renamed ${res.from} to ${res.to}`);
      setRenameFrom(null);
      setRenameTo('');
      await loadEntries(selectedSiteId, currentPath);
    } catch (err) {
      setError(extractErrorMessage(err, 'Failed to rename entry'));
    } finally {
      setBusy(false);
    }
  };

  const handleUpDirectory = () => {
    if (!currentPath) return;
    const parts = currentPath.split('/').filter(Boolean);
    parts.pop();
    setCurrentPath(parts.join('/'));
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">File Manager</h1>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            Browse and edit files jailed inside each static website document root.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <label className="text-sm font-medium text-gray-700 dark:text-gray-300" htmlFor="file-site-select">
            Website
          </label>
          <select
            id="file-site-select"
            aria-label="Website"
            value={selectedSiteId}
            onChange={(e) => {
              setSelectedSiteId(e.target.value);
              setCurrentPath('');
            }}
            className="rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-800 dark:text-white"
          >
            {staticSites.length === 0 ? (
              <option value="">No static websites</option>
            ) : (
              staticSites.map((site) => (
                <option key={site.id} value={site.id}>
                  {site.domain}
                </option>
              ))
            )}
          </select>
          <button
            type="button"
            onClick={() => selectedSiteId && void loadEntries(selectedSiteId, currentPath)}
            className="inline-flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
          >
            <RefreshCw className="w-4 h-4" />
            Refresh
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

      {notice && (
        <div
          role="status"
          className="rounded-lg border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-800 dark:border-emerald-800 dark:bg-emerald-900/30 dark:text-emerald-200"
        >
          {notice}
        </div>
      )}

      {loading ? (
        <p className="text-sm text-gray-500 dark:text-gray-400">Loading websites...</p>
      ) : staticSites.length === 0 ? (
        <div className="rounded-lg border border-gray-200 bg-white p-6 text-sm text-gray-600 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300">
          Create a static website first to manage its files.
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <div className="rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800 overflow-hidden">
            <div className="flex items-center justify-between border-b border-gray-200 px-4 py-3 dark:border-gray-700">
              <div className="flex items-center gap-2 text-sm font-medium text-gray-700 dark:text-gray-200">
                <span>Path:</span>
                <code className="rounded bg-gray-100 px-2 py-0.5 text-xs dark:bg-gray-700">
                  /{currentPath}
                </code>
              </div>
              {currentPath && (
                <button
                  type="button"
                  onClick={handleUpDirectory}
                  className="inline-flex items-center gap-1 rounded border border-gray-300 px-2.5 py-1 text-xs font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200"
                >
                  <ArrowUp className="w-3.5 h-3.5" />
                  Up
                </button>
              )}
            </div>

            {renameFrom && (
              <form
                onSubmit={handleRenameSubmit}
                className="flex items-center gap-2 border-b border-gray-200 bg-gray-50 px-4 py-2 dark:border-gray-700 dark:bg-gray-900/40"
              >
                <input
                  type="text"
                  aria-label="New path"
                  value={renameTo}
                  onChange={(e) => setRenameTo(e.target.value)}
                  className="flex-1 rounded border border-gray-300 px-2.5 py-1 text-sm dark:border-gray-600 dark:bg-gray-800 dark:text-white"
                  placeholder="New relative path"
                />
                <button
                  type="submit"
                  disabled={busy}
                  className="rounded bg-blue-600 px-3 py-1 text-xs font-medium text-white hover:bg-blue-700"
                >
                  Apply Rename
                </button>
                <button
                  type="button"
                  onClick={() => setRenameFrom(null)}
                  className="rounded border border-gray-300 px-2.5 py-1 text-xs text-gray-700 dark:border-gray-600 dark:text-gray-300"
                >
                  Cancel
                </button>
              </form>
            )}

            <table className="w-full text-left text-sm">
              <thead className="bg-gray-50 text-xs uppercase text-gray-500 dark:bg-gray-700/50 dark:text-gray-400">
                <tr>
                  <th className="px-4 py-2.5">Name</th>
                  <th className="px-4 py-2.5">Size</th>
                  <th className="px-4 py-2.5 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-200 dark:divide-gray-700">
                {entries.length === 0 ? (
                  <tr>
                    <td colSpan={3} className="px-4 py-6 text-center text-gray-500 dark:text-gray-400">
                      Directory is empty.
                    </td>
                  </tr>
                ) : (
                  entries.map((entry) => (
                    <tr key={entry.path} className="hover:bg-gray-50 dark:hover:bg-gray-700/40">
                      <td className="px-4 py-2.5">
                        <button
                          type="button"
                          onClick={() => void handleSelectEntry(entry)}
                          className="inline-flex items-center gap-2 font-medium text-blue-600 hover:underline dark:text-blue-400"
                        >
                          {entry.kind === 'dir' ? (
                            <Folder className="w-4 h-4" />
                          ) : (
                            <FileText className="w-4 h-4" />
                          )}
                          {entry.name}
                        </button>
                      </td>
                      <td className="px-4 py-2.5 text-gray-600 dark:text-gray-300">
                        {entry.kind === 'dir' ? '-' : `${entry.size} B`}
                      </td>
                      <td className="px-4 py-2.5 text-right space-x-2">
                        <button
                          type="button"
                          aria-label={`Rename ${entry.name}`}
                          onClick={() => {
                            setRenameFrom(entry.path);
                            setRenameTo(entry.path);
                          }}
                          className="inline-flex items-center rounded border border-gray-300 px-2 py-1 text-xs text-gray-700 hover:bg-gray-100 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
                        >
                          <Edit3 className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          aria-label={`Delete ${entry.name}`}
                          onClick={() => void handleDeleteEntry(entry)}
                          className="inline-flex items-center rounded border border-red-300 px-2 py-1 text-xs text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-400 dark:hover:bg-red-900/30"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>

          <form
            onSubmit={handleSaveFile}
            className="rounded-lg border border-gray-200 bg-white p-4 space-y-4 dark:border-gray-700 dark:bg-gray-800"
          >
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-semibold text-gray-900 dark:text-white">File Editor</h2>
              <button
                type="button"
                onClick={() => {
                  setEditorPath(currentPath ? `${currentPath}/new-file.txt` : 'new-file.txt');
                  setEditorContent('');
                }}
                className="inline-flex items-center gap-1 rounded border border-gray-300 px-2.5 py-1 text-xs font-medium text-gray-700 hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200"
              >
                <Plus className="w-3.5 h-3.5" />
                New File
              </button>
            </div>

            <div>
              <label
                htmlFor="file-editor-path"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                File Path
              </label>
              <input
                id="file-editor-path"
                type="text"
                value={editorPath}
                onChange={(e) => setEditorPath(e.target.value)}
                placeholder="index.html"
                className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>

            <div>
              <label
                htmlFor="file-editor-content"
                className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1"
              >
                File Content
              </label>
              <textarea
                id="file-editor-content"
                rows={10}
                value={editorContent}
                onChange={(e) => setEditorContent(e.target.value)}
                className="w-full rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm text-gray-900 dark:border-gray-600 dark:bg-gray-900 dark:text-white"
              />
            </div>

            <button
              type="submit"
              disabled={busy || !editorPath.trim()}
              className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
            >
              Save File
            </button>
          </form>
        </div>
      )}
    </div>
  );
}
