import { useState } from 'react';
import { useApi, useApiLazy } from '../hooks/useApi';
import { api } from '../api/client';
import type { DashboardUser, ApiKey } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';
import { CheckIcon, XIcon } from '../components/Icons';

function roleBadge(role: string) {
  const cls =
    role === 'admin' ? 'badge-danger' : role === 'operator' ? 'badge-warning' : 'badge';
  return <span className={`badge ${cls}`}>{role}</span>;
}

export default function AuthPage() {
  const [activeTab, setActiveTab] = useState<'users' | 'apikeys'>('users');

  // Users
  const [showCreateUser, setShowCreateUser] = useState(false);
  const [userForm, setUserForm] = useState({ username: '', password: '', role: 'viewer' as const });
  const [userFormError, setUserFormError] = useState<string | null>(null);
  const [deleteUserId, setDeleteUserId] = useState<string | null>(null);
  const [viewUserId, setViewUserId] = useState<string | null>(null);

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
    { key: 'id', header: 'ID', width: '90px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-muted)' }}>{String(v).slice(0, 8)}</span> },
    { key: 'username', header: 'Username', render: (v: unknown) => <span style={{ fontWeight: 500, color: 'var(--text-primary)' }}>{String(v)}</span> },
    { key: 'email', header: 'Email', render: (v: unknown) => <span style={{ color: 'var(--text-secondary)', fontSize: 12 }}>{v ? String(v) : '—'}</span> },
    { key: 'role', header: 'Role', width: '90px', render: (v: unknown) => roleBadge(String(v)) },
    { key: 'created_at', header: 'Created', width: '130px', render: (v: unknown) => <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{v ? new Date(v as number).toLocaleDateString() : '-'}</span> },
    { key: 'enabled', header: 'Status', width: '70px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    {
      key: 'actions',
      header: '',
      width: '140px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => setViewUserId(row.id as string)}>View</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteUserId(row.id as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const keyColumns: any[] = [
    { key: 'id', header: 'ID', width: '80px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-muted)' }}>{String(v).slice(0, 8)}</span> },
    { key: 'name', header: 'Name', render: (v: unknown) => <span style={{ fontWeight: 500 }}>{String(v)}</span> },
    { key: 'key_prefix', header: 'Prefix', width: '110px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, background: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 4 }}>{String(v ?? '').slice(0, 10)}…</span> },
    { key: 'role', header: 'Role', width: '90px', render: (v: unknown) => roleBadge(String(v)) },
    { key: 'permissions', header: 'Permissions', width: '140px', render: (v: unknown) => Array.isArray(v) && v.length ? <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>{(v as string[]).join(', ')}</span> : <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>—</span> },
    { key: 'enabled', header: 'Enabled', width: '70px', render: (v: unknown) => v ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : <XIcon size={14} style={{ color: 'var(--danger)' }} /> },
    { key: 'expires_at', header: 'Expires', width: '110px', render: (v: unknown) => <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{v ? new Date(v as number).toLocaleDateString() : 'Never'}</span> },
    {
      key: 'actions',
      header: '',
      width: '140px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => showToast(`${row.name}: ${row.permissions?.join(', ') || row.role}`)}>View</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteKeyId(row.id as string)}>Revoke</button>
        </div>
      ),
    },
  ];

  const activeUsers = users?.filter(u => u.enabled).length ?? 0;
  const viewUser = viewUserId ? users?.find(u => u.id === viewUserId) : null;
  const isUsersEmpty = !usersLoading && (!users || users.length === 0);
  const isKeysEmpty = !apiKeysLoading && (!apiKeys || apiKeys.length === 0);

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Users &amp; Keys</h1>
            <p>Manage users, roles, and scoped API keys</p>
          </div>
          <div className="flex gap-2">
            <button className={`btn ${activeTab === 'users' ? 'btn-primary' : ''}`} onClick={openCreateUser}>+ Create User</button>
            <button className={`btn ${activeTab === 'apikeys' ? 'btn-primary' : ''}`} onClick={() => { setShowCreateKey(!showCreateKey); setNewKeyDisplay(null); if (activeTab !== 'apikeys') setActiveTab('apikeys'); }}>{showCreateKey ? 'Cancel' : '+ Create API Key'}</button>
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
          {isUsersEmpty && (
            <div style={{ textAlign: 'center', marginTop: 12 }}>
              <button className="btn btn-primary btn-sm" onClick={openCreateUser}>Create your first user</button>
            </div>
          )}
        </div>
      ) : (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div className="card-title" style={{ margin: 0 }}>API Keys</div>
            <div className="text-sm text-muted">Revoke invalidates immediately</div>
          </div>

          {showCreateKey && (
            <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
              <div className="grid grid-cols-2 gap-3">
                <div className="form-group" style={{ marginBottom: 0 }}>
                  <label>Name *</label>
                  <input className="form-input" value={keyName} onChange={(e) => setKeyName(e.target.value)} placeholder="My service key" />
                </div>
                <div className="form-group" style={{ marginBottom: 0 }}>
                  <label>Role</label>
                  <select className="form-select" value={keyRole} onChange={(e) => setKeyRole(e.target.value as typeof keyRole)}>
                    <option value="viewer">Viewer (read)</option>
                    <option value="operator">Operator (read/write)</option>
                    <option value="admin">Admin (all)</option>
                  </select>
                </div>
                <div className="form-group" style={{ marginBottom: 0 }}>
                  <label>Expiry (optional)</label>
                  <input className="form-input" value={keyExpiry} onChange={e => setKeyExpiry(e.target.value)} placeholder="Leave empty for never-expires" type="date" />
                  <div className="form-hint">Leave empty for never-expires</div>
                </div>
              </div>
              <div style={{ marginTop: 12 }}>
                <button className="btn btn-primary" onClick={handleCreateKey} disabled={createKeyLoading || !keyName.trim()}>
                  {createKeyLoading ? 'Creating...' : 'Create Key'}
                </button>
              </div>
            </div>
          )}

          {newKeyDisplay && (
            <div className="callout warning" style={{ marginBottom: 16 }}>
              <strong>Copy now — this key won’t be shown again</strong>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 12, marginTop: 8, padding: 8, background: 'var(--bg-primary)', borderRadius: 6, wordBreak: 'break-all', border: '1px solid var(--border)' }}>
                {newKeyDisplay}
              </div>
              <div className="flex gap-2" style={{ marginTop: 8 }}>
                <button className="btn btn-sm btn-primary" onClick={() => { navigator.clipboard.writeText(newKeyDisplay); showToast('Copied to clipboard'); }}>Copy</button>
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
          {isKeysEmpty && !showCreateKey && (
            <div style={{ textAlign: 'center', marginTop: 12 }}>
              <button className="btn btn-primary btn-sm" onClick={() => setShowCreateKey(true)}>Create API key</button>
            </div>
          )}
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
          <input className="form-input" value={userForm.username} onChange={e => setUserForm({ ...userForm, username: e.target.value })} placeholder="alice" autoFocus />
        </div>
        <div className="form-group">
          <label>Password *</label>
          <input className="form-input" type="password" value={userForm.password} onChange={e => setUserForm({ ...userForm, password: e.target.value })} placeholder="•••••••• (min 8 chars)" />
        </div>
        <div className="form-group" style={{ marginBottom: 0 }}>
          <label>Role</label>
          <select className="form-select" value={userForm.role} onChange={e => setUserForm({ ...userForm, role: e.target.value as any })}>
            <option value="viewer">Viewer</option>
            <option value="operator">Operator</option>
            <option value="admin">Admin</option>
          </select>
        </div>
        {userFormError && <div className="callout error" style={{ marginTop: 12 }}>{userFormError}</div>}
      </Modal>

      <Modal isOpen={!!viewUser} onClose={() => setViewUserId(null)} title={viewUser?.username ?? 'User'} size="md"
        footer={<button className="btn" onClick={() => setViewUserId(null)}>Close</button>}>
        {viewUser && (
          <div>
            <div className="detail-row"><span className="detail-label">ID</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{viewUser.id}</span></div>
            <div className="detail-row"><span className="detail-label">Username</span><span className="detail-value">{viewUser.username}</span></div>
            <div className="detail-row"><span className="detail-label">Email</span><span className="detail-value">{viewUser.email || '—'}</span></div>
            <div className="detail-row"><span className="detail-label">Role</span><span className="detail-value">{roleBadge(viewUser.role)}</span></div>
            <div className="detail-row"><span className="detail-label">Status</span><span className="detail-value">{viewUser.enabled ? 'Active' : 'Disabled'}</span></div>
            <div className="detail-row"><span className="detail-label">Created</span><span className="detail-value">{viewUser.created_at ? new Date(viewUser.created_at).toLocaleString() : '—'}</span></div>
          </div>
        )}
      </Modal>

      <ConfirmDialog isOpen={!!deleteUserId} onClose={() => setDeleteUserId(null)} onConfirm={handleDeleteUser} title="Delete User" message={`Delete user ${users?.find(u => u.id === deleteUserId)?.username}?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={!!deleteKeyId} onClose={() => setDeleteKeyId(null)} onConfirm={handleDeleteKey} title="Revoke API Key" message="Revoke this API key? Existing clients will be disconnected immediately." confirmText="Revoke" variant="danger" />
    </div>
  );
}
