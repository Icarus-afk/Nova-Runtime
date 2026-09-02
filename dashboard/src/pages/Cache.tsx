import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { CacheStats } from '../types';
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
      setFormError('Key is required');
      return;
    }
    if (formTtl && (isNaN(parseInt(formTtl, 10)) || parseInt(formTtl, 10) <= 0)) {
      setFormError('TTL must be a positive number of seconds');
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
      const res: unknown = await api.getCacheEntry(key);
      const r = res as { value?: unknown };
      setFormValue(JSON.stringify(r.value ?? res, null, 2));
    } catch {
      setFormValue('{}');
    }
  };

  const openCreate = () => {
    setEditingKey(null);
    setFormKey('');
    setFormValue('{\n  "hello": "world"\n}');
    setFormTtl('');
    setFormError(null);
    setShowCreate(true);
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

  const keyColumns: unknown[] = [
    { key: 'key', header: 'Key', width: 'auto' },
    {
      key: 'actions',
      header: '',
      width: '148px',
      render: (_: unknown, row: unknown) => {
        const r = row as { key: string };
        return (
          <div className="actions" onClick={e => e.stopPropagation()}>
            <button className="btn btn-sm" onClick={() => handleViewKey(r.key)}>Inspect</button>
            <button className="btn btn-sm btn-danger" onClick={() => setDeleteKey(r.key)}>Delete</button>
          </div>
        );
      },
    },
  ];

  const hitRate = stats ? stats.hit_ratio * 100 : 0;
  const isFiltered = search.length > 0;
  const canSubmit = formKey.trim().length > 0 && !formLoading;

  return (
    <div>
      {/* Header — tight */}
      <div className="page-header" style={{ marginBottom: 12 }}>
        <div className="flex justify-between items-center">
          <div>
            <h1 style={{ margin: 0 }}>Cache</h1>
            <p style={{ margin: '2px 0 0', color: 'var(--text-muted)', fontSize: 13 }}>Low-latency key-value with TTL and LRU eviction</p>
          </div>
          <button className="btn btn-primary" onClick={openCreate} style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden><path d="M7 2.5v9M2.5 7h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
            Set Key
          </button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      {/* Stats — 4 cards, 12px gap */}
      <div className="grid grid-cols-4 mb-4" style={{ gap: 12, marginBottom: 12 }}>
        <MetricCard title="Hit Rate" value={hitRate.toFixed(1)} unit="%" color={hitRate > 90 ? 'success' : hitRate > 70 ? 'warning' : 'danger'} loading={statsLoading} />
        <MetricCard title="Entries" value={stats?.total_entries?.toLocaleString() ?? '-'} color="accent" loading={statsLoading} />
        <MetricCard title="Memory" value={stats ? formatBytes(stats.current_size_bytes) : '-'} unit={`/ ${stats ? formatBytes(stats.max_size_bytes) : ''}`} color="info" loading={statsLoading} />
        <MetricCard title="Evictions" value={stats?.eviction_count?.toLocaleString() ?? '-'} color="warning" loading={statsLoading} />
      </div>

      {/* Selected key details OR helpful tip — no empty half-column */}
      {selectedKey ? (
        <div className="card" style={{ marginBottom: 12, borderColor: 'var(--border)' }}>
          <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
              <span style={{ fontSize: 11, fontWeight: 600, letterSpacing: 0.4, textTransform: 'uppercase', color: 'var(--text-muted)' }}>Selected</span>
              <span style={{ fontFamily: 'monospace', fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{selectedKey}</span>
            </div>
            <div className="flex gap-2" style={{ flexShrink: 0 }}>
              <button className="btn btn-sm" onClick={() => openEdit(selectedKey)}>Edit</button>
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteKey(selectedKey)}>Delete</button>
              <button className="btn btn-sm" onClick={() => { setSelectedKey(null); setKeyValue(null); }} aria-label="Close details">✕</button>
            </div>
          </div>
          {valueLoading ? <div className="loading-spinner">Loading</div> : <div className="value-viewer" style={{ maxHeight: 260, overflow: 'auto' }}>{keyValue}</div>}
          <div style={{ marginTop: 10, display: 'flex', gap: 8 }}>
            <button className="btn btn-sm" onClick={() => navigator.clipboard.writeText(keyValue || '')}>Copy value</button>
            <span style={{ fontSize: 11, color: 'var(--text-muted)', alignSelf: 'center' }}>TTL and expiry are managed server-side</span>
          </div>
        </div>
      ) : (
        <div className="card" style={{ marginBottom: 12, background: 'var(--bg-subtle, #fafafa)', borderStyle: 'dashed', display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px' }}>
          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--bg-muted, #f1f5f9)', display: 'grid', placeItems: 'center', flexShrink: 0, color: 'var(--text-muted)' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden><path d="M12 16a2 2 0 100-4 2 2 0 000 4zM12 8v2" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" /><circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.5" /></svg>
          </div>
          <div style={{ fontSize: 13, color: 'var(--text-muted)', lineHeight: 1.4 }}>
            Click a key to view value, TTL and actions — or <button className="btn btn-sm" style={{ marginLeft: 4, verticalAlign: 'middle' }} onClick={openCreate}>Set Key</button> <span style={{ marginLeft: 4 }}>to create one.</span>
            <span style={{ marginLeft: 8, fontSize: 11, opacity: 0.8 }}>Tip: keys use <code style={{ fontSize: 11 }}>namespace:id</code> like <code style={{ fontSize: 11 }}>session:123</code></span>
          </div>
        </div>
      )}

      {/* Keys table + filter bar — tight 12px gaps */}
      <div className="card">
        <div className="flex items-center justify-between" style={{ marginBottom: 12, gap: 12, flexWrap: 'wrap' }}>
          <div className="card-title" style={{ margin: 0, fontSize: 13, fontWeight: 600 }}>Keys</div>
          <div className="flex gap-2" style={{ alignItems: 'center', flex: 1, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
            <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
              <input
                className="form-input"
                style={{ width: 320, maxWidth: '100%', paddingRight: isFiltered ? 28 : undefined }}
                placeholder="Filter keys (e.g. session:123)"
                value={search}
                onChange={(e) => { setSearch(e.target.value); setPage(1); }}
              />
              {isFiltered && (
                <button
                  onClick={() => { setSearch(''); setPage(1); }}
                  aria-label="Clear filter"
                  style={{ position: 'absolute', right: 6, background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--text-muted)', fontSize: 14, lineHeight: 1, padding: 2 }}
                >
                  ✕
                </button>
              )}
            </div>
            <button className="btn btn-sm" onClick={() => refetchKeys()}>Refresh</button>
            <button className="btn btn-sm btn-danger" onClick={() => setShowClearConfirm(true)}>Clear All</button>
          </div>
        </div>
        <DataTable
          columns={keyColumns as never}
          data={(keysData?.data || []) as unknown as Record<string, unknown>[]}
          loading={keysLoading}
          pagination={keysData?.pagination}
          onPageChange={setPage}
          onRowClick={(row) => handleViewKey((row as { key: string }).key)}
          emptyMessage="No keys yet — click Set Key to create your first entry"
        />
      </div>

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title={editingKey ? `Edit ${editingKey}` : 'Set Cache Key'} size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateOrUpdate} disabled={!canSubmit}>{formLoading ? 'Saving…' : editingKey ? 'Update' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Key <span style={{ color: 'var(--text-danger, #dc2626)' }}>*</span></label>
          <input className="form-input" value={formKey} onChange={e => setFormKey(e.target.value)} placeholder="session:123" disabled={!!editingKey} />
          {editingKey ? (
            <div className="form-hint" style={{ color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: 6 }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" aria-hidden><path d="M12 9v4M12 17h.01" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /><circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="1.4" /></svg>
              Key cannot be changed when editing — delete and recreate to rename.
            </div>
          ) : (
            <div className="form-hint">Examples: <code>session:123</code>, <code>user:42:profile</code>, <code>cache:query:abc</code> — use <code>namespace:id</code> for clarity.</div>
          )}
        </div>
        <div className="form-group">
          <label>Value (JSON)</label>
          <textarea className="form-input json-editor" value={formValue} onChange={e => setFormValue(e.target.value)} rows={6} placeholder='{"name": "alice"} or plain string' style={{ fontFamily: 'monospace', fontSize: 12 }} />
          <div className="form-hint">Valid JSON is stored as structured data; plain text is stored as a string.</div>
        </div>
        <div className="form-group">
          <label>TTL <span style={{ fontWeight: 400, color: 'var(--text-muted)' }}>— seconds, optional</span></label>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <input className="form-input" value={formTtl} onChange={e => setFormTtl(e.target.value)} placeholder="3600" type="number" min={1} style={{ flex: 1 }} />
            <span style={{ fontSize: 12, color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>seconds</span>
          </div>
          <div className="form-hint">Leave empty for no expiry. E.g. 3600 = 1 hour, 86400 = 1 day. Server default TTL applies if configured.</div>
        </div>
        {formError && <div className="callout error" style={{ marginTop: 8 }}>{formError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteKey} onClose={() => setDeleteKey(null)} onConfirm={handleDelete} title="Delete Key" message={`Delete key "${deleteKey}"?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={showClearConfirm} onClose={() => setShowClearConfirm(false)} onConfirm={handleClearAll} title="Clear Cache" message="Clear all cached entries? This cannot be undone (if backend supports it)." confirmText="Clear All" variant="danger" />
    </div>
  );
}
