import { useState } from 'react';
import { useApi, useApiLazy } from '../hooks/useApi';
import { api } from '../api/client';
import type { DashboardUser, ApiKey } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import StatusBadge from '../components/StatusBadge';
import { CheckIcon, XIcon } from '../components/Icons';

export default function AuthPage() {
  const [activeTab, setActiveTab] = useState<'users' | 'apikeys'>('users');
  const [showCreateKey, setShowCreateKey] = useState(false);
  const [keyName, setKeyName] = useState('');
  const [keyRole, setKeyRole] = useState<'admin' | 'operator' | 'viewer'>('operator');
  const [newKeyDisplay, setNewKeyDisplay] = useState<string | null>(null);

  const { data: users, loading: usersLoading, refetch: refetchUsers } = useApi<DashboardUser[]>(
    () => api.getUsers(), []
  );

  const { data: apiKeys, loading: apiKeysLoading, refetch: refetchKeys } = useApi<ApiKey[]>(
    () => api.getApiKeys(), []
  );

  const { execute: execCreateKey, loading: createKeyLoading } = useApiLazy<ApiKey & { full_key: string }>();

  const handleCreateKey = async () => {
    const result = await execCreateKey(() => api.createApiKey(keyName, keyRole));
    if (result && 'full_key' in result) {
      setNewKeyDisplay((result as ApiKey & { full_key: string }).full_key);
      setShowCreateKey(false);
      refetchKeys();
    }
  };

  const handleDeleteKey = async (id: string) => {
    try {
      await api.deleteApiKey(id);
      refetchKeys();
    } catch {}
  };

  const handleDeleteUser = async (id: string) => {
    try {
      await api.deleteUser(id);
      refetchUsers();
    } catch {}
  };

  const userColumns = [
    { key: 'username', header: 'Username' },
    { key: 'email', header: 'Email' },
    { key: 'role', header: 'Role', width: '80px', render: (v: unknown) => <StatusBadge status={v === 'admin' ? 'healthy' : v === 'operator' ? 'degraded' : 'critical'} label={v as string} /> },
    { key: 'mfa_enabled', header: 'MFA', width: '60px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : '-' },
    { key: 'enabled', header: 'Enabled', width: '60px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    { key: 'last_login_at', header: 'Last Login', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : 'Never' },
    { key: 'created_at', header: 'Created', width: '140px', render: (v: unknown) => new Date(v as number).toLocaleString() },
  ];

  const keyColumns = [
    { key: 'name', header: 'Name' },
    { key: 'key_prefix', header: 'Prefix', width: '100px', render: (v: unknown) => `${v}...` },
    { key: 'role', header: 'Role', width: '80px' },
    { key: 'enabled', header: 'Enabled', width: '60px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    { key: 'last_used_at', header: 'Last Used', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : 'Never' },
    { key: 'expires_at', header: 'Expires', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : 'Never' },
  ];

  const activeUsers = users?.filter(u => u.enabled).length ?? 0;

  return (
    <div>
      <div className="page-header">
        <h1>Users & API Keys</h1>
        <p>Manage dashboard users and API access keys</p>
      </div>

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Users" value={users?.length ?? '-'} color="accent" loading={usersLoading} />
        <MetricCard title="Active Users" value={activeUsers} color="success" loading={usersLoading} />
        <MetricCard title="API Keys" value={apiKeys?.length ?? '-'} color="info" loading={apiKeysLoading} />
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === 'users' ? 'active' : ''}`} onClick={() => setActiveTab('users')}>Users</button>
        <button className={`tab ${activeTab === 'apikeys' ? 'active' : ''}`} onClick={() => setActiveTab('apikeys')}>API Keys</button>
      </div>

      {activeTab === 'users' ? (
        <div className="card">
          <DataTable
            columns={userColumns}
            data={(users || []) as unknown as Record<string, unknown>[]}
            loading={usersLoading}
            emptyMessage="No users found"
          />
        </div>
      ) : (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div className="card-title" style={{ margin: 0 }}>API Keys</div>
            <button className="btn btn-sm btn-primary" onClick={() => { setShowCreateKey(!showCreateKey); setNewKeyDisplay(null); }}>
              {showCreateKey ? 'Cancel' : 'Create Key'}
            </button>
          </div>

          {showCreateKey && (
            <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
              <div className="grid grid-cols-2 gap-3">
                <div className="form-group">
                  <label>Key Name</label>
                  <input className="form-input" value={keyName} onChange={(e) => setKeyName(e.target.value)} placeholder="My API Key" />
                </div>
                <div className="form-group">
                  <label>Role</label>
                  <select className="form-select" value={keyRole} onChange={(e) => setKeyRole(e.target.value as typeof keyRole)}>
                    <option value="admin">Admin</option>
                    <option value="operator">Operator</option>
                    <option value="viewer">Viewer</option>
                  </select>
                </div>
              </div>
              <button className="btn btn-primary" onClick={handleCreateKey} disabled={createKeyLoading || !keyName}>
                {createKeyLoading ? 'Creating...' : 'Create'}
              </button>
            </div>
          )}

          {newKeyDisplay && (
            <div className="callout warning" style={{ marginBottom: 16 }}>
              <strong>New API Key created! Save it now — it won't be shown again.</strong>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 13, marginTop: 8, padding: 8, background: 'var(--bg-primary)', borderRadius: 'var(--radius-sm)', wordBreak: 'break-all' }}>
                {newKeyDisplay}
              </div>
              <button className="btn btn-sm" style={{ marginTop: 8 }} onClick={() => setNewKeyDisplay(null)}>Dismiss</button>
            </div>
          )}

          <DataTable
            columns={keyColumns}
            data={(apiKeys || []) as unknown as Record<string, unknown>[]}
            loading={apiKeysLoading}
            onRowClick={(row) => handleDeleteKey(row.id as string)}
            emptyMessage="No API keys created"
          />
        </div>
      )}
    </div>
  );
}
