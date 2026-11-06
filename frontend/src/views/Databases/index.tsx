import React, { useState, useMemo, useEffect } from 'react';
import { api } from '../../api';

interface Database {
  id?: string;
  name: string;
  engine?: string;
  charset?: string;
  state?: 'creating' | 'ready' | 'failed';
  created_at?: string;
}

interface FormErrors {
  nameError: string;
}

interface Credentials {
  host: string;
  port: number;
  database: string;
  username: string;
  password?: string;
}

const DatabasesView: React.FC = () => {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [databases, setDatabases] = useState<Database[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [capabilities, setCapabilities] = useState<string[]>([]);
  const [unavailableCaps, setUnavailableCaps] = useState<string[]>([]);

  const [newDb, setNewDb] = useState<Database>({ name: '', engine: 'mariadb' });
  const [errors, setErrors] = useState<FormErrors>({ nameError: '' });

  const [showCredsModal, setShowCredsModal] = useState(false);
  const [credentials, setCredentials] = useState<Credentials | null>(null);
  const [creatingUserFor, setCreatingUserFor] = useState<string | null>(null);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [dbData, capsData] = await Promise.all([
        api.getDatabases(),
        api.getCapabilities().catch(() => ({ capabilities: [], unavailable: [] }))
      ]);
      setDatabases(dbData || []);
      setCapabilities(capsData.capabilities || []);
      setUnavailableCaps(capsData.unavailable || []);
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to load databases');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const validateIdentifier = (id: string, errorKey: keyof FormErrors) => {
    let errorMsg = '';
    if (!id) {
       errorMsg = 'Identifier is required';
    } else if (id.length > 63) {
       errorMsg = 'Identifier must be 1-63 characters';
    } else if (/^\d/.test(id)) {
       errorMsg = 'Identifier cannot start with a digit';
    } else if (!/^[a-zA-Z0-9_]+$/.test(id)) {
       errorMsg = 'Identifier can only contain alphanumeric characters and underscores';
    }

    setErrors(prev => ({ ...prev, [errorKey]: errorMsg }));
  };

  const isFormValid = useMemo(() => {
    return newDb.name.length > 0 && !errors.nameError;
  }, [newDb, errors]);

  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setNewDb(prev => ({ ...prev, name: val }));
    validateIdentifier(val, 'nameError');
  };

  const createDatabase = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isFormValid) {
      setError(null);
      try {
        await api.createDatabase(newDb);
        await loadData();
        setShowCreateModal(false);
        setNewDb({ name: '', engine: 'mariadb' });
      } catch (e: any) {
        setError(e?.response?.data?.error?.message || e.message || 'Failed to create database');
      }
    }
  };

  const createUser = async (dbName: string) => {
    setCreatingUserFor(dbName);
    setError(null);
    try {
      const creds = await api.createDatabaseUser(dbName);
      setCredentials(creds);
      setShowCredsModal(true);
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to create user');
    } finally {
      setCreatingUserFor(null);
    }
  };

  const closeCredsModal = () => {
    setShowCredsModal(false);
    setCredentials(null);
  };

  const isCapAvailable = (cap: string) => {
    return capabilities.includes(cap) && !unavailableCaps.includes(cap);
  };

  const isCapUnavailable = (cap: string) => {
    return unavailableCaps.includes(cap) || (!capabilities.includes(cap));
  };

  return (
    <div className="p-5">
      <header className="flex justify-between items-center mb-4">
        <h1 className="text-2xl font-bold">Databases</h1>
        <button
          onClick={() => setShowCreateModal(true)}
          className="bg-blue-600 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
        >
          Create Database
        </button>
      </header>

      {error && <div className="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4">{error} <button onClick={loadData} className="underline ml-2">Retry</button></div>}

      {isLoading ? (
        <div>Loading...</div>
      ) : (
        <section>
          <ul className="space-y-4">
            {databases.map(db => (
              <li key={db.id || db.name} className="border p-4 rounded bg-white shadow-sm flex flex-col md:flex-row justify-between md:items-center">
                <div>
                  <h3 className="text-lg font-semibold">{db.name}</h3>
                  <div className="text-sm text-gray-600">
                    Engine: {db.engine || 'unknown'} | State: <span className={`font-semibold ${db.state === 'ready' ? 'text-green-600' : db.state === 'failed' ? 'text-red-600' : 'text-yellow-600'}`}>{db.state || 'ready'}</span>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2 mt-2 md:mt-0">
                  <button
                    onClick={() => createUser(db.name)}
                    disabled={creatingUserFor === db.name}
                    className="bg-green-600 hover:bg-green-700 text-white py-1 px-3 rounded text-sm disabled:opacity-50"
                  >
                    {creatingUserFor === db.name ? 'Creating...' : 'Create SQL Account'}
                  </button>

                  <button
                    disabled={isCapUnavailable('database.backup')}
                    title={isCapUnavailable('database.backup') ? 'Not available yet' : ''}
                    className="bg-gray-200 hover:bg-gray-300 text-gray-800 py-1 px-3 rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Backup {isCapUnavailable('database.backup') && '(Not available yet)'}
                  </button>

                  <button
                    disabled={isCapUnavailable('database.restore')}
                    title={isCapUnavailable('database.restore') ? 'Not available yet' : ''}
                    className="bg-gray-200 hover:bg-gray-300 text-gray-800 py-1 px-3 rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Restore {isCapUnavailable('database.restore') && '(Not available yet)'}
                  </button>

                  <button
                    disabled={isCapUnavailable('database.delete')}
                    title={isCapUnavailable('database.delete') ? 'Not available yet' : ''}
                    className="bg-red-600 hover:bg-red-700 text-white py-1 px-3 rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed"
                  >
                    Delete {isCapUnavailable('database.delete') && '(Not available yet)'}
                  </button>
                </div>
              </li>
            ))}
            {databases.length === 0 && <li className="text-gray-500">No databases found.</li>}
          </ul>
        </section>
      )}

      {/* Create Database Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white p-6 rounded shadow-lg min-w-[300px]">
            <h2 className="text-xl font-bold mb-4">Create New Database</h2>
            <form onSubmit={createDatabase} className="flex flex-col gap-4">
              <div>
                <label htmlFor="dbNameInput" className="block text-sm font-medium mb-1">Database Name:</label>
                <input
                  type="text"
                  value={newDb.name}
                  onChange={handleNameChange}
                  id="dbNameInput" className="w-full border rounded p-2"
                />
                {errors.nameError && <span className="text-red-500 text-xs mt-1 block">{errors.nameError}</span>}
              </div>
              <div className="flex gap-2 justify-end mt-4">
                <button type="button" onClick={() => setShowCreateModal(false)} className="px-4 py-2 border rounded">Cancel</button>
                <button type="submit" disabled={!isFormValid} className="px-4 py-2 bg-blue-600 text-white rounded disabled:opacity-50">Create</button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Credentials Modal */}
      {showCredsModal && credentials && (
        <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div className="bg-white p-6 rounded shadow-lg max-w-md w-full">
            <h2 className="text-xl font-bold mb-2">Database Credentials</h2>
            <p className="text-red-600 font-semibold mb-4 text-sm">Warning: This password is shown exactly once. Please copy it now and store it securely.</p>

            <div className="space-y-2 mb-6">
              <div><span className="font-semibold">Host:</span> {credentials.host}</div>
              <div><span className="font-semibold">Port:</span> {credentials.port}</div>
              <div><span className="font-semibold">Database:</span> {credentials.database}</div>
              <div><span className="font-semibold">Username:</span> {credentials.username}</div>
              <div className="mt-2 p-3 bg-gray-100 rounded break-all flex justify-between items-center">
                <span className="font-mono">{credentials.password}</span>
                <button
                  onClick={() => navigator.clipboard.writeText(credentials.password || '')}
                  className="ml-2 text-blue-600 hover:text-blue-800 text-sm font-semibold"
                >
                  Copy
                </button>
              </div>
            </div>

            <div className="flex justify-end">
              <button onClick={closeCredsModal} className="px-4 py-2 bg-blue-600 text-white rounded">Close</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default DatabasesView;
