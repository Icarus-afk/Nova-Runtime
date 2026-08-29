import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { CacheStats, CacheEntry } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export default function CachePage() {
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [keyValue, setKeyValue] = useState<string | null>(null);
  const [valueLoading, setValueLoading] = useState(false);

  // Create/Edit
  const [showCreate, setShowCreate] = useState(false);
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [formKey, setFormKey] = useState('');
  const [formValue, setFormValue] = useState('{\n  "hello": "world"\n}');
  const [formTtl, setFormTtl] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const [formLoading, setFormLoading] = useState(false);

  // Delete
  const [deleteKey, setDeleteKey] = useState<string | null>(null);
  const [showClearConfirm, setShowClearConfirm] = useState(false);

  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: stats, loading: statsLoading } = useApi<CacheStats>(() => api.getCacheStats(), []);
  const { data: keysData, loading: keysLoading, refetch: refetchKeys } = useApi(
    () => api.getCacheKeys(search || undefined, page),
    [search, page]
  );

  const handleViewKey = async (key: string) => {
    setSelectedKey(key);
    setValueLoading(true);
    try {
      const res = await api.getCacheEntry(key);
      setKeyValue(JSON.stringify(res.value ?? res, null, 2));
    } catch {
      // Fallback to direct fetch
      try {
        const res = await fetch(`/api/v1/cache/${encodeURIComponent(key)}`, {
          headers: { Authorization: `Bearer ${localStorage.getItem('nova_token') || ''}` },
        });
        const data = await res.json();
        setKeyValue(JSON.stringify(data.value ?? data, null, 2));
      } catch {
        setKeyValue('Error loading value');
      }
    } finally {
      setValueLoading(false);
    }
  };

  const handleCreateOrUpdate = async () => {
    if (!formKey.trim()) {
      setFormError('Key required');
      return;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(formValue);
    } catch {
      parsed = formValue;
    }
    setFormLoading(true);
    setFormError(null);
    try {
      const ttl = formTtl ? parseInt(formTtl, 10) : undefined;
      await api.setCacheKey(formKey.trim(), parsed, ttl);
      showToast(editingKey ? `Updated ${formKey}` : `Created ${formKey}`);
      setShowCreate(false);
      setEditingKey(null);
      setFormKey('');
      setFormValue('{\n  "hello": "world"\n}');
      setFormTtl('');
      refetchKeys();
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : 'Save failed');
    } finally {
      setFormLoading(false);
    }
  };

  const openEdit = async (key: string) => {
    setEditingKey(key);
    setFormKey(key);
    setFormTtl('');
    setFormError(null);
    setShowCreate(true);
    try {
      const res: any = await api.getCacheEntry(key);
      setFormValue(JSON.stringify(res.value ?? res, null, 2));
    } catch {
      setFormValue('{}');
    }
  };

  const handleDelete = async () => {
    if (!deleteKey) return;
    try {
      await api.deleteCacheKey(deleteKey);
      showToast(`Deleted ${deleteKey}`);
      setDeleteKey(null);
      if (selectedKey === deleteKey) {
        setSelectedKey(null);
        setKeyValue(null);
      }
      refetchKeys();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleClearAll = async () => {
    try {
      await api.clearCache();
      showToast('Cache cleared (if supported)');
      setShowClearConfirm(false);
      refetchKeys();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Clear failed', 'error');
    }
  };

  const keyColumns: any[] = [
    { key: 'key', header: 'Key', width: 'auto' },
    {
      key: 'actions',
      header: '',
      width: '160px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => handleViewKey(row.key as string)}>View</button>
          <button className="btn btn-sm" onClick={() => openEdit(row.key as string)}>Edit</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteKey(row.key as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const hitRate = stats ? stats.hit_ratio * 100 : 0;

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Cache</h1>
            <p>Low-latency key-value with TTL and LRU eviction</p>
          </div>
          <button className="btn btn-primary" onClick={() => { setEditingKey(null); setFormKey(''); setFormValue('{\n  "hello": "world"\n}'); setFormTtl(''); setFormError(null); setShowCreate(true); }}>+ Set Key</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Hit Rate" value={hitRate.toFixed(1)} unit="%" color={hitRate > 90 ? 'success' : hitRate > 70 ? 'warning' : 'danger'} loading={statsLoading} />
        <MetricCard title="Entries" value={stats?.total_entries?.toLocaleString() ?? '-'} color="accent" loading={statsLoading} />
        <MetricCard title="Memory" value={stats ? formatBytes(stats.current_size_bytes) : '-'} unit={`/ ${stats ? formatBytes(stats.max_size_bytes) : ''}`} color="info" loading={statsLoading} />
        <MetricCard title="Evictions" value={stats?.eviction_count?.toLocaleString() ?? '-'} color="warning" loading={statsLoading} />
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="card">
          <div className="card-title">Statistics</div>
          <div style={{ marginTop: 8 }}>
            <div className="detail-row"><span className="detail-label">Hit Count</span><span className="detail-value">{stats?.hit_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Miss Count</span><span className="detail-value">{stats?.miss_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">TTL Expired</span><span className="detail-value">{stats?.ttl_expired_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Oldest</span><span className="detail-value">{stats ? `${Math.round(stats.oldest_entry_age_seconds / 60)}m` : '-'}</span></div>
          </div>
        </div>
        {selectedKey && (
          <div className="card">
            <div className="flex justify-between items-center mb-2">
              <div className="card-title" style={{ margin: 0 }}>Key: {selectedKey}</div>
              <div className="flex gap-2">
                <button className="btn btn-sm" onClick={() => openEdit(selectedKey)}>Edit</button>
                <button className="btn btn-sm btn-danger" onClick={() => setDeleteKey(selectedKey)}>Delete</button>
                <button className="btn btn-sm" onClick={() => { setSelectedKey(null); setKeyValue(null); }}>Close</button>
              </div>
            </div>
            {valueLoading ? <div className="loading-spinner">Loading</div> : <div className="value-viewer">{keyValue}</div>}
            <div className="flex gap-2" style={{ marginTop: 8 }}>
              <button className="btn btn-sm" onClick={() => navigator.clipboard.writeText(keyValue || '')}>Copy</button>
            </div>
          </div>
        )}
      </div>

      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <div className="card-title" style={{ margin: 0 }}>Keys</div>
          <div className="flex gap-2">
            <input className="form-input" style={{ width: 200 }} placeholder="Search pattern: user:*" value={search} onChange={(e) => { setSearch(e.target.value); setPage(1); }} />
            <button className="btn btn-sm" onClick={() => refetchKeys()}>Refresh</button>
            <button className="btn btn-sm btn-danger" onClick={() => setShowClearConfirm(true)}>Clear All</button>
          </div>
        </div>
        <DataTable
          columns={keyColumns}
          data={(keysData?.data || []) as unknown as Record<string, unknown>[]}
          loading={keysLoading}
          pagination={keysData?.pagination}
          onPageChange={setPage}
          onRowClick={(row) => handleViewKey(row.key as string)}
          emptyMessage="No cached entries — set a key to get started"
        />
      </div>

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title={editingKey ? `Edit ${editingKey}` : 'Set Cache Key'} size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateOrUpdate} disabled={formLoading}>{formLoading ? 'Saving...' : editingKey ? 'Update' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Key</label>
          <input className="form-input" value={formKey} onChange={e => setFormKey(e.target.value)} placeholder="user:123" disabled={!!editingKey} />
          {editingKey && <div className="form-hint">Key cannot be changed when editing — delete and recreate to rename</div>}
        </div>
        <div className="form-group">
          <label>Value (JSON)</label>
          <textarea className="form-input json-editor" value={formValue} onChange={e => setFormValue(e.target.value)} rows={6} placeholder='{"name": "alice"} or plain string' />
        </div>
        <div className="form-group">
          <label>TTL (seconds, optional)</label>
          <input className="form-input" value={formTtl} onChange={e => setFormTtl(e.target.value)} placeholder="3600 (1 hour)" type="number" />
          <div className="form-hint">Leave empty for no expiry. Server default TTL is applied if configured.</div>
        </div>
        {formError && <div className="callout error">{formError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteKey} onClose={() => setDeleteKey(null)} onConfirm={handleDelete} title="Delete Key" message={`Delete key "${deleteKey}"?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={showClearConfirm} onClose={() => setShowClearConfirm(false)} onConfirm={handleClearAll} title="Clear Cache" message="Clear all cached entries? This cannot be undone (if backend supports it)." confirmText="Clear All" variant="danger" />
    </div>
  );
}
