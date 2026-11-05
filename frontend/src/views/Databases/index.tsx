import React, { useState, useMemo, useEffect } from 'react';
import { api } from '../../api';

interface Database {
  id?: string;
  name: string;
  owner: string;
}

interface User {
  name: string;
}

interface FormErrors {
  nameError: string;
  ownerError: string;
}

const DatabasesView: React.FC = () => {
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [databases, setDatabases] = useState<Database[]>([]);
  const [users, setUsers] = useState<User[]>([]);
  const [privileges, setPrivileges] = useState<Record<string, string>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const dbData = await api.getDatabases();
      setDatabases(dbData || []);

      // In a real app we'd fetch users and privileges too.
      // For now we'll just extract unique owners from databases as mock users if empty
      const uniqueOwners = Array.from(new Set((dbData || []).map((db: Database) => db.owner)));
      setUsers(uniqueOwners.map(name => ({ name: name as string })));
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to load databases');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const [newDb, setNewDb] = useState<Database>({ name: '', owner: '' });
  const [errors, setErrors] = useState<FormErrors>({ nameError: '', ownerError: '' });

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
    return newDb.name.length > 0 &&
           newDb.owner.length > 0 &&
           !errors.nameError &&
           !errors.ownerError;
  }, [newDb, errors]);

  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setNewDb(prev => ({ ...prev, name: val }));
    validateIdentifier(val, 'nameError');
  };

  const handleOwnerChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setNewDb(prev => ({ ...prev, owner: val }));
    validateIdentifier(val, 'ownerError');
  };

  const createDatabase = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isFormValid) {
      setError(null);
      try {
        const created = await api.createDatabase(newDb);
        setDatabases(prev => [...prev, created]);
        if (!users.find(u => u.name === created.owner)) {
          setUsers(prev => [...prev, { name: created.owner }]);
        }
        setShowCreateModal(false);
        setNewDb({ name: '', owner: '' });
      } catch (e: any) {
        setError(e?.response?.data?.error?.message || e.message || 'Failed to create database');
      }
    }
  };

  const triggerDump = (dbName: string) => {
    console.log(`Triggering dump for ${dbName}`);
    alert(`Dump download initiated for ${dbName}`);
  };

  const assignPrivilege = (dbName: string, userName: string, privilege: string) => {
    console.log(`Assigning ${privilege} to ${userName} on ${dbName}`);
    setPrivileges(prev => ({ ...prev, [`${dbName}-${userName}`]: privilege }));
  };

  return (
    <div style={{ padding: '20px' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
        <h1>Databases Management</h1>
        <button onClick={() => setShowCreateModal(true)}>Create Database</button>
      </header>

      {error && <div className="error-banner text-red-500 mb-4">{error} <button onClick={loadData} className="underline ml-2">Retry</button></div>}

      {isLoading ? (
        <div>Loading...</div>
      ) : (
        <>
          <section style={{ display: 'flex', gap: '20px', marginBottom: '20px' }}>
            <div style={{ flex: 1 }}>
              <h2>Databases</h2>
              <ul>
                {databases.map(db => (
                  <li key={db.name} style={{ marginBottom: '10px' }}>
                    {db.name} (Owner: {db.owner}){' '}
                    <button onClick={() => triggerDump(db.name)}>Download Dump</button>
                  </li>
                ))}
                {databases.length === 0 && <li>No databases found.</li>}
              </ul>
            </div>

            <div style={{ flex: 1 }}>
              <h2>Users</h2>
              <ul>
                {users.map(user => (
                  <li key={user.name}>
                    {user.name}
                  </li>
                ))}
                {users.length === 0 && <li>No users found.</li>}
              </ul>
            </div>
          </section>

          {databases.length > 0 && users.length > 0 && (
            <section>
              <h2>Privilege Assignment</h2>
              <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                <thead>
                  <tr>
                    <th style={{ border: '1px solid #ccc', padding: '8px', textAlign: 'left' }}>Database \ User</th>
                    {users.map(user => (
                      <th key={user.name} style={{ border: '1px solid #ccc', padding: '8px', textAlign: 'left' }}>{user.name}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {databases.map(db => (
                    <tr key={db.name}>
                      <td style={{ border: '1px solid #ccc', padding: '8px', textAlign: 'left' }}>{db.name}</td>
                      {users.map(user => (
                        <td key={user.name} style={{ border: '1px solid #ccc', padding: '8px', textAlign: 'left' }}>
                          <select
                            value={privileges[`${db.name}-${user.name}`] || ''}
                            onChange={(e) => assignPrivilege(db.name, user.name, e.target.value)}
                          >
                            <option value="">None</option>
                            <option value="SELECT">SELECT</option>
                            <option value="INSERT">INSERT</option>
                            <option value="ALL">ALL PRIVILEGES</option>
                          </select>
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )}
        </>
      )}

      {/* Create Database Modal */}
      {showCreateModal && (
        <div style={{
          position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
          background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center'
        }}>
          <div style={{ background: 'white', padding: '20px', borderRadius: '4px', minWidth: '300px' }}>
            <h2>Create New Database</h2>
            <form onSubmit={createDatabase} style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
              <div>
                <label style={{ display: 'block' }}>Database Name:</label>
                <input type="text" value={newDb.name} onChange={handleNameChange} style={{ width: '100%' }} />
                {errors.nameError && <span style={{ color: 'red', fontSize: '0.8em', display: 'block' }}>{errors.nameError}</span>}
              </div>
              <div>
                <label style={{ display: 'block' }}>Owner (Username):</label>
                <input type="text" value={newDb.owner} onChange={handleOwnerChange} style={{ width: '100%' }} />
                {errors.ownerError && <span style={{ color: 'red', fontSize: '0.8em', display: 'block' }}>{errors.ownerError}</span>}
              </div>
              <div style={{ display: 'flex', gap: '10px', marginTop: '10px' }}>
                <button type="submit" disabled={!isFormValid}>Create</button>
                <button type="button" onClick={() => setShowCreateModal(false)}>Cancel</button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export default DatabasesView;
