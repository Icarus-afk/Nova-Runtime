import { useState } from 'react';
import { useApi, useApiLazy } from '../hooks/useApi';
import { api } from '../api/client';
import type { DashboardUser, ApiKey } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import StatusBadge from '../components/StatusBadge';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';
import { CheckIcon, XIcon } from '../components/Icons';

export default function AuthPage() {
  const [activeTab, setActiveTab] = useState<'users' | 'apikeys'>('users');

  // Users
  const [showCreateUser, setShowCreateUser] = useState(false);
  const [userForm, setUserForm] = useState({ username: '', password: '', role: 'viewer' as const });
  const [userFormError, setUserFormError] = useState<string | null>(null);
  const [deleteUserId, setDeleteUserId] = useState<string | null>(null);

  // API Keys
  const [showCreateKey, setShowCreateKey] = useState(false);
  const [keyName, setKeyName] = useState('');
  const [keyRole, setKeyRole] = useState<'admin' | 'operator' | 'viewer'>('operator');
  const [keyExpiry, setKeyExpiry] = useState('');
  const [newKeyDisplay, setNewKeyDisplay] = useState<string | null>(null);
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null);

  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: users, loading: usersLoading, refetch: refetchUsers } = useApi<DashboardUser[]>(
    () => api.getUsers(), []
  );

  const { data: apiKeys, loading: apiKeysLoading, refetch: refetchKeys } = useApi<ApiKey[]>(
    () => api.getApiKeys(), []
  );

  const { execute: execCreateKey, loading: createKeyLoading } = useApiLazy<ApiKey & { full_key: string }>();
  const { execute: execCreateUser, loading: createUserLoading } = useApiLazy<any>();

  const handleCreateUser = async () => {
    if (!userForm.username.trim() || !userForm.password.trim()) {
      setUserFormError('Username and password required');
      return;
    }
    if (userForm.password.length < 8) {
      setUserFormError('Password must be at least 8 characters');
      return;
    }
    setUserFormError(null);
    try {
      await execCreateUser(() => api.createUser({ username: userForm.username, password: userForm.password, roles: [userForm.role] } as any));
      showToast(`User ${userForm.username} created`);
      setShowCreateUser(false);
      setUserForm({ username: '', password: '', role: 'viewer' });
      refetchUsers();
    } catch (err: unknown) {
      setUserFormError(err instanceof Error ? err.message : 'Save failed');
    }
  };

  const handleDeleteUser = async () => {
    if (!deleteUserId) return;
    try {
      await api.deleteUser(deleteUserId);
      showToast('User deleted');
      setDeleteUserId(null);
      refetchUsers();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const openCreateUser = () => {
    setUserForm({ username: '', password: '', role: 'viewer' });
    setUserFormError(null);
    setShowCreateUser(true);
  };

  const handleCreateKey = async () => {
    if (!keyName.trim()) {
      showToast('Key name required', 'error');
      return;
    }
    const result = await execCreateKey(() => api.createApiKey(keyName, keyRole));
    if (result && 'full_key' in result) {
      setNewKeyDisplay((result as ApiKey & { full_key: string }).full_key);
      setShowCreateKey(false);
      setKeyName('');
      refetchKeys();
      showToast(`API key ${keyName} created`);
    }
  };

  const handleDeleteKey = async () => {
    if (!deleteKeyId) return;
    try {
      await api.deleteApiKey(deleteKeyId);
      showToast('API key revoked');
      setDeleteKeyId(null);
      refetchKeys();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Revoke failed', 'error');
    }
  };

  const userColumns: any[] = [
    { key: 'username', header: 'Username' },
    { key: 'role', header: 'Role', width: '90px', render: (v: unknown) => <StatusBadge status={v === 'admin' ? 'healthy' : v === 'operator' ? 'degraded' : 'critical'} label={v as string} /> },
    { key: 'created_at', header: 'Created', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    { key: 'enabled', header: 'Status', width: '80px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    {
      key: 'actions',
      header: '',
      width: '90px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteUserId(row.id as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const keyColumns: any[] = [
    { key: 'name', header: 'Name' },
    { key: 'key_prefix', header: 'Prefix', width: '110px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{String(v)}...</span> },
    { key: 'role', header: 'Role', width: '80px' },
    { key: 'enabled', header: 'Enabled', width: '70px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    { key: 'last_used_at', header: 'Last Used', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : 'Never' },
    { key: 'expires_at', header: 'Expires', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : 'Never' },
    {
      key: 'actions',
      header: '',
      width: '100px',
      render: (_: unknown, row: any) => (
        <button className="btn btn-sm btn-danger" onClick={() => setDeleteKeyId(row.id as string)}>Revoke</button>
      ),
    },
  ];

  const activeUsers = users?.filter(u => u.enabled).length ?? 0;

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Users & API Keys</h1>
            <p>Manage access with roles, MFA, and scoped API keys</p>
          </div>
          <div className="flex gap-2">
            {activeTab === 'users' ? (
              <button className="btn btn-primary" onClick={openCreateUser}>+ Create User</button>
            ) : (
              <button className="btn btn-primary" onClick={() => { setShowCreateKey(!showCreateKey); setNewKeyDisplay(null); }}>{showCreateKey ? 'Cancel' : '+ Create Key'}</button>
            )}
          </div>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Users" value={users?.length ?? '-'} color="accent" loading={usersLoading} />
        <MetricCard title="Active" value={activeUsers} color="success" loading={usersLoading} />
        <MetricCard title="API Keys" value={apiKeys?.length ?? '-'} color="info" loading={apiKeysLoading} />
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === 'users' ? 'active' : ''}`} onClick={() => setActiveTab('users')}>Users ({users?.length ?? 0})</button>
        <button className={`tab ${activeTab === 'apikeys' ? 'active' : ''}`} onClick={() => setActiveTab('apikeys')}>API Keys ({apiKeys?.length ?? 0})</button>
      </div>

      {activeTab === 'users' ? (
        <div className="card">
          <div className="flex justify-between items-center mb-4">
            <div className="card-title" style={{ margin: 0 }}>Users</div>
            <button className="btn btn-sm" onClick={() => refetchUsers()}>Refresh</button>
          </div>
          <DataTable
            columns={userColumns}
            data={(users || []) as unknown as Record<string, unknown>[]}
            loading={usersLoading}
            emptyMessage="No users — create one"
          />
        </div>
      ) : (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div className="card-title" style={{ margin: 0 }}>API Keys</div>
            <div className="text-sm text-muted">Click Revoke to immediately invalidate</div>
          </div>

          {showCreateKey && (
            <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
              <div className="grid grid-cols-2 gap-3">
                <div className="form-group">
                  <label>Name *</label>
                  <input className="form-input" value={keyName} onChange={(e) => setKeyName(e.target.value)} placeholder="My service key" />
                </div>
                <div className="form-group">
                  <label>Role</label>
                  <select className="form-select" value={keyRole} onChange={(e) => setKeyRole(e.target.value as typeof keyRole)}>
                    <option value="viewer">Viewer (read)</option>
                    <option value="operator">Operator (read/write)</option>
                    <option value="admin">Admin (all)</option>
                  </select>
                </div>
                <div className="form-group">
                  <label>Expiry (optional)</label>
                  <input className="form-input" value={keyExpiry} onChange={e => setKeyExpiry(e.target.value)} placeholder="2025-12-31 or leave empty" type="date" />
                  <div className="form-hint">Leave empty for never-expires</div>
                </div>
              </div>
              <button className="btn btn-primary" onClick={handleCreateKey} disabled={createKeyLoading || !keyName.trim()}>
                {createKeyLoading ? 'Creating...' : 'Create Key'}
              </button>
            </div>
          )}

          {newKeyDisplay && (
            <div className="callout warning" style={{ marginBottom: 16 }}>
              <strong>Save now — won't be shown again!</strong>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, marginTop: 8, padding: 8, background: 'var(--bg-primary)', borderRadius: 6, wordBreak: 'break-all', border: '1px solid var(--border)' }}>
                {newKeyDisplay}
              </div>
              <div className="flex gap-2" style={{ marginTop: 8 }}>
                <button className="btn btn-sm btn-primary" onClick={() => navigator.clipboard.writeText(newKeyDisplay)}>Copy</button>
                <button className="btn btn-sm" onClick={() => setNewKeyDisplay(null)}>Dismiss</button>
              </div>
            </div>
          )}

          <DataTable
            columns={keyColumns}
            data={(apiKeys || []) as unknown as Record<string, unknown>[]}
            loading={apiKeysLoading}
            emptyMessage="No API keys — create one for programmatic access"
          />
        </div>
      )}

      <Modal isOpen={showCreateUser} onClose={() => setShowCreateUser(false)} title="Create User" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreateUser(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateUser} disabled={createUserLoading}>{createUserLoading ? 'Saving...' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Username *</label>
          <input className="form-input" value={userForm.username} onChange={e => setUserForm({ ...userForm, username: e.target.value })} placeholder="alice" />
        </div>
        <div className="form-group">
          <label>Password *</label>
          <input className="form-input" type="password" value={userForm.password} onChange={e => setUserForm({ ...userForm, password: e.target.value })} placeholder="•••••••• (min 8 chars)" />
        </div>
        <div className="form-group">
          <label>Role</label>
          <select className="form-select" value={userForm.role} onChange={e => setUserForm({ ...userForm, role: e.target.value as any })}>
            <option value="viewer">Viewer</option>
            <option value="operator">Operator</option>
            <option value="admin">Admin</option>
          </select>
        </div>
        {userFormError && <div className="callout error">{userFormError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteUserId} onClose={() => setDeleteUserId(null)} onConfirm={handleDeleteUser} title="Delete User" message={`Delete user ${users?.find(u => u.id === deleteUserId)?.username}?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={!!deleteKeyId} onClose={() => setDeleteKeyId(null)} onConfirm={handleDeleteKey} title="Revoke API Key" message="Revoke this API key? Existing clients will be disconnected immediately." confirmText="Revoke" variant="danger" />
    </div>
  );
}
