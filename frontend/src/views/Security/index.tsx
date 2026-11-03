import React, { useState } from 'react';

const initialRules = [
  { id: 1, port: '80', protocol: 'TCP', sourceCidr: '0.0.0.0/0', action: 'Allow', priority: 100 },
  { id: 2, port: '443', protocol: 'TCP', sourceCidr: '0.0.0.0/0', action: 'Allow', priority: 110 },
  { id: 3, port: '22', protocol: 'TCP', sourceCidr: '192.168.1.0/24', action: 'Allow', priority: 120 }
];

export default function SecurityRulesUI() {
  const [rules, setRules] = useState(initialRules);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [showSshWarning, setShowSshWarning] = useState(false);

  // Form State
  const [port, setPort] = useState('');
  const [protocol, setProtocol] = useState('TCP');
  const [sourceCidr, setSourceCidr] = useState('');
  const [action, setAction] = useState('Allow');
  const [priority, setPriority] = useState('');

  const [error, setError] = useState('');

  const validatePort = (portValue: string) => {
    const portRegex = /^(\d+)(?:-(\d+))?$/;
    const match = portValue.match(portRegex);

    if (!match) return false;

    const p1 = parseInt(match[1], 10);
    const p2 = match[2] ? parseInt(match[2], 10) : p1;

    if (p1 < 1 || p1 > 65535 || p2 < 1 || p2 > 65535) return false;
    if (match[2] && p1 >= p2) return false;

    return true;
  };

  const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    setPort(val);
    if (val === '22' || val.startsWith('22-') || val.endsWith('-22') || val.includes('-22-') || val === '22') {
      setShowSshWarning(true);
    } else {
      // Check range specifically for 22 if valid range
      const match = val.match(/^(\d+)(?:-(\d+))?$/);
      if (match) {
        const p1 = parseInt(match[1], 10);
        const p2 = match[2] ? parseInt(match[2], 10) : p1;
        if (p1 <= 22 && p2 >= 22) {
          setShowSshWarning(true);
        } else {
          setShowSshWarning(false);
        }
      } else {
        setShowSshWarning(false);
      }
    }
  };

  const handleAddRule = (e: React.FormEvent) => {
    e.preventDefault();
    if (!validatePort(port)) {
      setError('Invalid port or port range. Must be 1-65535 or a valid range (e.g., 8000-8080).');
      return;
    }

    setError('');
    const newRule = {
      id: Date.now(),
      port,
      protocol,
      sourceCidr: sourceCidr || '0.0.0.0/0',
      action,
      priority: parseInt(priority, 10) || 100
    };

    setRules([...rules, newRule]);
    setIsModalOpen(false);

    // Reset form
    setPort('');
    setProtocol('TCP');
    setSourceCidr('');
    setAction('Allow');
    setPriority('');
    setShowSshWarning(false);
  };

  return (
    <div style={{ padding: '20px', fontFamily: 'sans-serif' }}>
      <h1>Security & Firewall Rules</h1>

      {showSshWarning && (
        <div style={{ backgroundColor: '#ffcccc', color: '#cc0000', padding: '10px', marginBottom: '20px', borderRadius: '4px', border: '1px solid #cc0000' }}>
          <strong>Warning! Emergency SSH Lockout Risk:</strong> You are modifying a rule for port 22. Ensure you do not lock yourself out.
        </div>
      )}

      <button onClick={() => setIsModalOpen(true)} style={{ marginBottom: '15px', padding: '8px 16px', cursor: 'pointer' }}>
        Add Rule
      </button>

      {isModalOpen && (
        <div style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <div style={{ backgroundColor: 'white', padding: '20px', borderRadius: '5px', width: '400px' }}>
            <h2>Add Firewall Rule</h2>
            {error && <p style={{ color: 'red' }}>{error}</p>}
            <form onSubmit={handleAddRule} style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>

              <label>
                Port (e.g., 80, 8000-8080):
                <input type="text" value={port} onChange={handlePortChange} required style={{ width: '100%', marginTop: '5px' }} />
              </label>

              <label>
                Protocol:
                <select value={protocol} onChange={e => setProtocol(e.target.value)} style={{ width: '100%', marginTop: '5px' }}>
                  <option value="TCP">TCP</option>
                  <option value="UDP">UDP</option>
                  <option value="ICMP">ICMP</option>
                  <option value="ALL">ALL</option>
                </select>
              </label>

              <label>
                Source CIDR:
                <input type="text" value={sourceCidr} onChange={e => setSourceCidr(e.target.value)} placeholder="0.0.0.0/0" style={{ width: '100%', marginTop: '5px' }} />
              </label>

              <label>
                Action:
                <select value={action} onChange={e => setAction(e.target.value)} style={{ width: '100%', marginTop: '5px' }}>
                  <option value="Allow">Allow</option>
                  <option value="Deny">Deny</option>
                </select>
              </label>

              <label>
                Priority:
                <input type="number" value={priority} onChange={e => setPriority(e.target.value)} placeholder="100" style={{ width: '100%', marginTop: '5px' }} />
              </label>

              <div style={{ display: 'flex', gap: '10px', marginTop: '10px' }}>
                <button type="submit" style={{ flex: 1, padding: '8px' }}>Save Rule</button>
                <button type="button" onClick={() => { setIsModalOpen(false); setShowSshWarning(false); setError(''); }} style={{ flex: 1, padding: '8px' }}>Cancel</button>
              </div>
            </form>
          </div>
        </div>
      )}

      <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left' }}>
        <thead>
          <tr style={{ borderBottom: '2px solid #ddd' }}>
            <th style={{ padding: '10px' }}>Port</th>
            <th>Protocol</th>
            <th>Source CIDR</th>
            <th>Action</th>
            <th>Priority</th>
          </tr>
        </thead>
        <tbody>
          {rules.map(rule => (
            <tr key={rule.id} style={{ borderBottom: '1px solid #eee' }}>
              <td style={{ padding: '10px' }}>{rule.port}</td>
              <td>{rule.protocol}</td>
              <td>{rule.sourceCidr}</td>
              <td>{rule.action}</td>
              <td>{rule.priority}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
