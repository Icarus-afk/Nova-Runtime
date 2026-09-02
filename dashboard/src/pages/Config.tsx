import { useState, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { ConfigEntry } from '../types';
import { CheckIcon, AlertIcon } from '../components/Icons';
import Modal from '../components/Modal';

function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function parseValue(raw: string, type: string): unknown {
  if (type === 'number') {
    const n = Number(raw);
    if (Number.isNaN(n)) throw new Error('Invalid number');
    return n;
  }
  if (type === 'boolean') {
    if (raw === 'true') return true;
    if (raw === 'false') return false;
    throw new Error('Must be true/false');
  }
  if (type === 'object' || type === 'array') {
    return JSON.parse(raw);
  }
  return raw;
}

export default function ConfigPage() {
  const [search, setSearch] = useState('');
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [editingEntry, setEditingEntry] = useState<ConfigEntry | null>(null);
  const [editValue, setEditValue] = useState('');
  const [editError, setEditError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: entries, loading, error, refetch } = useApi<ConfigEntry[]>(() => api.getConfig(), []);

  const sections = useMemo(() => {
    if (!entries) return {};
    const groups: Record<string, ConfigEntry[]> = {};
    for (const entry of entries) {
      const section = entry.key.split('.')[0] || 'general';
      if (!groups[section]) groups[section] = [];
      groups[section].push(entry);
    }
    return groups;
  }, [entries]);

  const filteredEntries = useMemo(() => {
    if (!search.trim() || !entries) return entries;
    const q = search.toLowerCase();
    return entries.filter(
      (e) => e.key.toLowerCase().includes(q) || e.description.toLowerCase().includes(q)
    );
  }, [entries, search]);

  const selectedEntry = selectedKey ? entries?.find(e => e.key === selectedKey) : null;

  const openEdit = (entry: ConfigEntry) => {
    if (!entry.mutable) {
      showToast('This key is not mutable — requires restart', 'error');
      return;
    }
    setEditingEntry(entry);
    setEditValue(typeof entry.value === 'object' ? JSON.stringify(entry.value, null, 2) : String(entry.value ?? ''));
    setEditError(null);
  };

  const handleSave = async () => {
    if (!editingEntry) return;
    setEditError(null);
    setSaving(true);
    try {
      const parsed = parseValue(editValue, editingEntry.type);
      // Build nested patch object from dotted key
      const patch: Record<string, unknown> = {};
      const parts = editingEntry.key.split('.');
      let cur: any = patch;
      for (let i = 0; i < parts.length - 1; i++) {
        cur[parts[i]] = {};
        cur = cur[parts[i]];
      }
      cur[parts[parts.length - 1]] = parsed;

      await api.updateConfig(patch);
      showToast(`Updated ${editingEntry.key}`);
      setEditingEntry(null);
      refetch();
    } catch (err: unknown) {
      setEditError(err instanceof Error ? err.message : 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Configuration</h1>
            <p>Runtime config — editable if mutable, otherwise requires restart</p>
          </div>
          <button className="btn btn-sm" onClick={() => refetch()}>Refresh</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="callout info mb-4" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span>Edits hit <code>PUT /admin/config</code> and hot-reload where supported. Non-mutable keys need restart via <code>SIGHUP</code> or container restart.</span>
        <button className="btn btn-sm" onClick={() => { navigator.clipboard.writeText(JSON.stringify(Object.fromEntries((entries || []).map(e => [e.key, e.value])), null, 2)); showToast('Copied config JSON'); }}>Copy JSON</button>
      </div>

      <div className="flex gap-4">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="flex items-center gap-2 mb-4">
            <input
              className="form-input"
              style={{ width: 300 }}
              placeholder="Search keys or description..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <span className="text-sm text-muted">{filteredEntries?.length ?? 0} / {entries?.length ?? 0} entries</span>
          </div>

          {loading ? (
            <div className="loading-spinner">Loading configuration</div>
          ) : error ? (
            <div className="card">
              <div className="callout error" style={{ marginBottom: 12 }}>Failed to load config: {error}</div>
              <div className="text-muted" style={{ textAlign: 'center', padding: 12 }}>Check that <code>novad</code> is running and you are logged in.</div>
            </div>
          ) : !entries || entries.length === 0 ? (
            <div className="card">
              <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>No configuration entries</div>
            </div>
          ) : (
            <div>
              {search.trim() ? (
                <div className="card">
                  <div className="data-table-wrapper">
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th>Key</th>
                          <th>Value</th>
                          <th>Type</th>
                          <th>Mutable</th>
                          <th></th>
                        </tr>
                      </thead>
                      <tbody>
                        {(filteredEntries || []).map((entry) => (
                          <tr key={entry.key} onClick={() => setSelectedKey(entry.key)} style={{ cursor: 'pointer' }}>
                            <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{entry.key}</td>
                            <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, maxWidth: 300 }} className="truncate">
                              {formatValue(entry.value)}
                            </td>
                            <td><span className="badge">{entry.type}</span></td>
                            <td>{entry.mutable ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : '-'}</td>
                            <td>
                              {entry.mutable ? (
                                <button className="btn btn-sm" onClick={(e) => { e.stopPropagation(); openEdit(entry); }}>Edit</button>
                              ) : (
                                <span className="text-muted" style={{ fontSize: 11 }}>restart</span>
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              ) : (
                Object.entries(sections).map(([section, sectionEntries]) => (
                  <div key={section} className="card mb-4">
                    <div className="flex justify-between items-center mb-2">
                      <div className="card-title" style={{ margin: 0 }}>{section} <span className="count">({sectionEntries.length})</span></div>
                      <span className="text-sm text-muted">{sectionEntries.filter(e => e.mutable).length} mutable</span>
                    </div>
                    <div className="data-table-wrapper">
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>Key</th>
                            <th>Value</th>
                            <th>Type</th>
                            <th>Mutable</th>
                            <th>Restart</th>
                            <th></th>
                          </tr>
                        </thead>
                        <tbody>
                          {sectionEntries.map((entry) => (
                            <tr key={entry.key} onClick={() => setSelectedKey(entry.key)} style={{ cursor: 'pointer' }}>
                              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                                {entry.key.replace(section + '.', '')}
                              </td>
                              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, maxWidth: 280 }} className="truncate">
                                {formatValue(entry.value)}
                              </td>
                              <td><span className="badge">{entry.type}</span></td>
                              <td>{entry.mutable ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : '-'}</td>
                              <td>{entry.requires_restart ? <AlertIcon size={14} style={{ color: 'var(--warning)' }} /> : '-'}</td>
                              <td>
                                {entry.mutable ? (
                                  <button className="btn btn-sm" onClick={(e) => { e.stopPropagation(); openEdit(entry); }}>Edit</button>
                                ) : (
                                  <span className="text-muted" style={{ fontSize: 11 }}>—</span>
                                )}
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>

        {selectedEntry && (
          <div className="detail-panel" style={{ width: 360, flexShrink: 0, position: 'sticky', top: 16, alignSelf: 'flex-start' }}>
            <div className="flex items-center justify-between mb-4">
              <div className="card-title" style={{ margin: 0 }}>Details</div>
              <div className="flex gap-2">
                {selectedEntry.mutable && <button className="btn btn-sm btn-primary" onClick={() => openEdit(selectedEntry)}>Edit</button>}
                <button className="btn btn-sm" onClick={() => setSelectedKey(null)}>Close</button>
              </div>
            </div>
            <div className="detail-row">
              <span className="detail-label">Key</span>
              <span className="detail-value" style={{ fontSize: 11, wordBreak: 'break-all' }}>{selectedEntry.key}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Value</span>
              <span className="detail-value" style={{ wordBreak: 'break-all' }}>{formatValue(selectedEntry.value)}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Type</span>
              <span className="detail-value"><span className="badge">{selectedEntry.type}</span></span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Mutable</span>
              <span className="detail-value">{selectedEntry.mutable ? 'Yes — hot reload' : 'No — restart required'}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Requires Restart</span>
              <span className="detail-value">{selectedEntry.requires_restart ? 'Yes' : 'No'}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Default</span>
              <span className="detail-value">{formatValue(selectedEntry.default_value)}</span>
            </div>
            {selectedEntry.description && (
              <div style={{ marginTop: 12, padding: 10, background: 'var(--bg-primary)', borderRadius: 6, fontSize: 12, color: 'var(--text-secondary)', lineHeight: 1.5 }}>
                {selectedEntry.description}
              </div>
            )}
            {selectedEntry.mutable && (
              <div className="callout info" style={{ marginTop: 12, fontSize: 11 }}>
                Click Edit to change. Non-mutable keys need `kill -SIGHUP` or restart.
              </div>
            )}
          </div>
        )}
      </div>

      <Modal isOpen={!!editingEntry} onClose={() => setEditingEntry(null)} title={`Edit ${editingEntry?.key}`} size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setEditingEntry(null)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleSave} disabled={saving}>{saving ? 'Saving...' : 'Save'}</button>
          </div>
        }>
        {editingEntry && (
          <div>
            <div className="form-group">
              <label>Key</label>
              <input className="form-input" value={editingEntry.key} disabled style={{ background: 'var(--bg-tertiary)' }} />
            </div>
            <div className="form-group">
              <label>Value ({editingEntry.type})</label>
              {editingEntry.type === 'boolean' ? (
                <select className="form-select" value={editValue} onChange={e => setEditValue(e.target.value)}>
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              ) : (
                <textarea className="form-input" value={editValue} onChange={e => setEditValue(e.target.value)} rows={editingEntry.type === 'object' || editingEntry.type === 'array' ? 4 : 2} style={{ fontFamily: editingEntry.type === 'string' ? 'inherit' : 'var(--font-mono)', fontSize: 12 }} />
              )}
              <div className="form-hint">
                {editingEntry.type === 'number' && 'Enter a number'}
                {editingEntry.type === 'object' && 'Valid JSON object'}
                {editingEntry.type === 'duration' && 'e.g. 30s, 5m, 1h'}
                {editingEntry.type === 'size' && 'e.g. 1GB, 512MB'}
                {!['number', 'object', 'duration', 'size', 'boolean'].includes(editingEntry.type) && 'String value'}
              </div>
            </div>
            <div className="text-sm text-muted">
              Default: <span style={{ fontFamily: 'var(--font-mono)' }}>{formatValue(editingEntry.default_value)}</span>
              {editingEntry.requires_restart && <span style={{ color: 'var(--warning)', marginLeft: 8 }}>· Requires restart</span>}
            </div>
            {editError && <div className="callout error" style={{ marginTop: 12 }}>{editError}</div>}
          </div>
        )}
      </Modal>
    </div>
  );
}
