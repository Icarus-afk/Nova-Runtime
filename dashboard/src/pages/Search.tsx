import { useState, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { IndexInfo, SearchResult } from '../types';
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

function scoreTone(score: number, max: number): { bg: string; color: string; label: string } {
  const normalized = max > 0 ? score / max : score;
  if (normalized >= 0.75 || score >= 1.5) return { bg: 'rgba(16,185,129,0.14)', color: 'var(--success)', label: 'high' };
  if (normalized >= 0.4 || score >= 0.6) return { bg: 'rgba(245,158,11,0.14)', color: 'var(--warning)', label: 'med' };
  return { bg: 'var(--bg-tertiary)', color: 'var(--text-muted)', label: 'low' };
}

export default function SearchPage() {
  const [selectedIndex, setSelectedIndex] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [queryResult, setQueryResult] = useState<SearchResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);

  // Create index
  const [showCreate, setShowCreate] = useState(false);
  const [newIndexName, setNewIndexName] = useState('');
  const [newFields, setNewFields] = useState<Array<{ name: string; type: string }>>([{ name: 'title', type: 'text' }, { name: 'body', type: 'text' }]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createLoading, setCreateLoading] = useState(false);

  // Add documents
  const [showAddDocs, setShowAddDocs] = useState(false);
  const [docsJson, setDocsJson] = useState('[\n  {"id": "1", "title": "Hello world", "body": "This is a test document"}\n]');
  const [addDocsError, setAddDocsError] = useState<string | null>(null);
  const [addDocsLoading, setAddDocsLoading] = useState(false);
  const docsJsonError = useMemo(() => {
    try {
      const v = JSON.parse(docsJson);
      const arr = Array.isArray(v) ? v : [v];
      if (arr.length === 0) return 'Array is empty — add at least one document.';
      const missing = arr.findIndex((d: unknown) => !d || typeof d !== 'object' || !('id' in (d as Record<string, unknown>)));
      if (missing >= 0) return `Document ${missing + 1} is missing required field "id".`;
      return null;
    } catch (e: unknown) {
      return e instanceof Error ? e.message : 'Invalid JSON';
    }
  }, [docsJson]);

  const [deleteIndexName, setDeleteIndexName] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: indexes, loading: indexesLoading, refetch: refetchIndexes } = useApi<IndexInfo[]>(
    () => api.getIndexes(), []
  );

  const selectedMeta = indexes?.find(i => i.name === selectedIndex) ?? null;
  const maxScore = useMemo(() => queryResult ? Math.max(0, ...queryResult.hits.map(h => h.score)) : 0, [queryResult]);

  const handleSearch = async () => {
    if (!selectedIndex || !query.trim()) return;
    setQueryLoading(true);
    setQueryError(null);
    try {
      const result = await api.searchQuery(selectedIndex, query);
      setQueryResult(result);
    } catch (err: unknown) {
      setQueryError(err instanceof Error ? err.message : 'Search failed');
    } finally {
      setQueryLoading(false);
    }
  };

  const handleCreateIndex = async () => {
    if (!newIndexName.trim()) {
      setCreateError('Name is required — use lowercase, e.g. my_index');
      return;
    }
    if (!/^[a-z0-9_\-]+$/.test(newIndexName.trim())) {
      setCreateError('Name may only contain a–z, 0–9, _ and -');
      return;
    }
    const cleaned = newFields.filter(f => f.name.trim());
    if (cleaned.length === 0) {
      setCreateError('Add at least one field');
      return;
    }
    setCreateLoading(true);
    setCreateError(null);
    try {
      await api.createIndex(newIndexName.trim(), cleaned);
      showToast(`Index ${newIndexName.trim()} created`);
      setShowCreate(false);
      setNewIndexName('');
      setNewFields([{ name: 'title', type: 'text' }, { name: 'body', type: 'text' }]);
      refetchIndexes();
    } catch (err: unknown) {
      setCreateError(err instanceof Error ? err.message : 'Create failed');
    } finally {
      setCreateLoading(false);
    }
  };

  const handleDeleteIndex = async () => {
    if (!deleteIndexName) return;
    try {
      await api.deleteIndex(deleteIndexName);
      showToast(`Index ${deleteIndexName} deleted`);
      if (selectedIndex === deleteIndexName) {
        setSelectedIndex(null);
        setQueryResult(null);
        setQuery('');
      }
      setDeleteIndexName(null);
      refetchIndexes();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleAddDocs = async () => {
    if (!selectedIndex) return;
    if (docsJsonError) {
      setAddDocsError(docsJsonError);
      return;
    }
    setAddDocsError(null);
    setAddDocsLoading(true);
    try {
      const docs = JSON.parse(docsJson);
      const arr = Array.isArray(docs) ? docs : [docs];
      await api.addDocuments(selectedIndex, arr);
      showToast(`Added ${arr.length} document${arr.length === 1 ? '' : 's'} to ${selectedIndex}`);
      setShowAddDocs(false);
      refetchIndexes();
    } catch (err: unknown) {
      setAddDocsError(err instanceof Error ? err.message : 'Add failed — check JSON');
    } finally {
      setAddDocsLoading(false);
    }
  };

  const indexColumns: any[] = [
    {
      key: 'name',
      header: 'Index',
      render: (v: unknown) => (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{String(v)}</span>
      ),
    },
    { key: 'document_count', header: 'Docs', width: '84px', render: (v: unknown) => <span style={{ fontVariantNumeric: 'tabular-nums' }}>{(v as number).toLocaleString()}</span> },
    { key: 'index_size_bytes', header: 'Size', width: '84px', render: (v: unknown) => <span style={{ color: 'var(--text-secondary)', fontSize: 12 }}>{formatBytes(v as number)}</span> },
    { key: 'field_count', header: 'Fields', width: '70px', render: (v: unknown) => <span className="badge" style={{ fontFamily: 'var(--font-mono)' }}>{String(v)}</span> },
    {
      key: 'actions',
      header: '',
      width: '150px',
      render: (_: unknown, row: any) => {
        const isActive = selectedIndex === row.name;
        return (
          <div className="actions" onClick={e => e.stopPropagation()}>
            <button
              className={`btn btn-sm ${isActive ? '' : 'btn-primary'}`}
              onClick={() => { setSelectedIndex(row.name as string); setQueryResult(null); setQueryError(null); }}
              title={isActive ? 'Selected' : 'Open index'}
            >
              {isActive ? 'Selected' : 'Open'}
            </button>
            <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(row.name as string)}>Delete</button>
          </div>
        );
      },
    },
  ];

  const totalDocs = indexes?.reduce((s, i) => s + i.document_count, 0) ?? 0;
  const totalSize = indexes ? indexes.reduce((s, i) => s + i.index_size_bytes, 0) : 0;

  return (
    <div>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 12 }}>
        <div className="flex justify-between items-center" style={{ gap: 12 }}>
          <div>
            <h1 style={{ margin: 0 }}>Search</h1>
            <p style={{ margin: '2px 0 0', color: 'var(--text-muted)', fontSize: 12.5 }}>Full-text search with BM25, field types, and highlighting — per-index</p>
          </div>
          <button className="btn btn-primary" onClick={() => { setCreateError(null); setShowCreate(true); }} style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden><path d="M7 2.5v9M2.5 7h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
            Create Index
          </button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      {/* Stats — gap 12 */}
      <div className="grid grid-cols-3" style={{ gap: 12, marginBottom: 12 }}>
        <MetricCard title="Indexes" value={indexes?.length ?? '-'} color="accent" loading={indexesLoading} />
        <MetricCard title="Total Docs" value={totalDocs.toLocaleString() ?? '-'} color="info" loading={indexesLoading} />
        <MetricCard title="Total Size" value={indexes ? formatBytes(totalSize) : '-'} color="success" loading={indexesLoading} />
      </div>

      {/* Indexes table */}
      <div className="card" style={{ marginBottom: 12, padding: 12 }}>
        <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
          <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>Indexes</div>
          <div className="flex gap-2">
            {indexes && indexes.length > 0 && selectedIndex && (
              <span style={{ fontSize: 11, color: 'var(--text-muted)', alignSelf: 'center' }}>
                Selected <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-primary)', fontWeight: 600 }}>{selectedIndex}</span>
              </span>
            )}
            <button className="btn btn-sm" onClick={() => refetchIndexes()}>Refresh</button>
          </div>
        </div>
        <DataTable
          columns={indexColumns}
          data={(indexes || []) as unknown as Record<string, unknown>[]}
          loading={indexesLoading}
          onRowClick={(row) => { setSelectedIndex(row.name as string); setQueryResult(null); setQueryError(null); }}
          emptyMessage="No indexes — create one to start"
        />
        {!indexesLoading && (!indexes || indexes.length === 0) && (
          <div style={{ display: 'flex', gap: 8, justifyContent: 'center', marginTop: 10 }}>
            <button className="btn btn-sm btn-primary" onClick={() => setShowCreate(true)}>+ Create your first index</button>
            <span style={{ fontSize: 11, color: 'var(--text-muted)', alignSelf: 'center' }}>Indexes hold documents and define searchable fields.</span>
          </div>
        )}
      </div>

      {/* Selected index: left info + right results */}
      {selectedIndex && (
        <div className="grid grid-cols-2" style={{ gap: 12 }}>
          {/* Left: Index info + query */}
          <div className="card" style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div className="flex justify-between items-center" style={{ gap: 8 }}>
              <div style={{ minWidth: 0 }}>
                <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>Index</div>
                <div style={{ fontFamily: 'var(--font-mono)', fontSize: 13, fontWeight: 700, color: 'var(--text-primary)', marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={selectedIndex}>
                  {selectedIndex}
                </div>
              </div>
              <span className="badge" title="Fields in this index" style={{ flexShrink: 0 }}>{selectedMeta ? `${selectedMeta.field_count} fields` : '—'}</span>
            </div>

            {selectedMeta && (
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', fontSize: 11, color: 'var(--text-muted)', background: 'var(--bg-tertiary)', border: '1px solid var(--border)', borderRadius: 6, padding: '6px 8px' }}>
                <span><strong style={{ color: 'var(--text-primary)', fontWeight: 600 }}>{selectedMeta.document_count.toLocaleString()}</strong> docs</span>
                <span style={{ opacity: 0.5 }}>·</span>
                <span>{formatBytes(selectedMeta.index_size_bytes)}</span>
                <span style={{ opacity: 0.5 }}>·</span>
                <span>{selectedMeta.field_count} fields</span>
              </div>
            )}

            <div className="flex gap-2" style={{ flexWrap: 'wrap' }}>
              <button className="btn btn-sm btn-primary" onClick={() => { setAddDocsError(null); setShowAddDocs(true); }}>+ Add Documents</button>
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(selectedIndex)}>Delete Index</button>
              <button className="btn btn-sm" onClick={() => { setSelectedIndex(null); setQueryResult(null); setQuery(''); setQueryError(null); }}>Close</button>
            </div>

            <div className="form-group" style={{ marginBottom: 0 }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                Query
                <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, fontSize: 11, color: 'var(--text-muted)' }}>BM25 — Enter to search</span>
              </label>
              <div className="flex gap-2">
                <input
                  className="form-input"
                  style={{ flex: 1, fontSize: 13 }}
                  placeholder="Search for documents..."
                  value={query}
                  onChange={e => setQuery(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') handleSearch(); }}
                  aria-label="Search query"
                />
                <button className="btn btn-primary" onClick={handleSearch} disabled={queryLoading || !query.trim()} style={{ minWidth: 78 }}>
                  {queryLoading ? 'Searching…' : 'Search'}
                </button>
              </div>
              <div className="form-hint">Tip: use field:value for keyword fields · e.g. <code style={{ fontSize: 11 }}>title:hello</code></div>
            </div>

            {queryError && <div className="callout error" style={{ margin: 0 }}>{queryError}</div>}

            <div style={{ fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5, borderTop: '1px dashed var(--border)', paddingTop: 8, marginTop: 2 }}>
              Documents are JSON with an <code style={{ fontSize: 11 }}>id</code> field. Add via <strong style={{ color: 'var(--text-secondary)' }}>Add Documents</strong> and search above. Scores are BM25 relevance.
            </div>
          </div>

          {/* Right: Results */}
          <div className="card" style={{ padding: 12, display: 'flex', flexDirection: 'column', minHeight: 220 }}>
            <div className="flex justify-between items-center" style={{ marginBottom: 8 }}>
              <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>
                Results {queryResult ? <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)', fontSize: 11 }}>· {queryResult.total} in {queryResult.execution_time_ms.toFixed(1)}ms</span> : null}
              </div>
              {queryResult && queryResult.hits.length > 0 && (
                <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{queryResult.hits.length} shown</span>
              )}
            </div>

            {!queryResult ? (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', padding: 24, textAlign: 'center', color: 'var(--text-muted)', border: '1px dashed var(--border)', borderRadius: 8, background: 'var(--bg-tertiary)' }}>
                <div>
                  <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--bg-hover)', display: 'grid', placeItems: 'center', margin: '0 auto 8px', color: 'var(--text-muted)' }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden><circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="1.6" /><path d="M16 16l4 4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
                  </div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 4 }}>Enter a query and click Search</div>
                  <div style={{ fontSize: 11, lineHeight: 1.5 }}>Try <code style={{ fontSize: 11 }}>hello</code> or <code style={{ fontSize: 11 }}>title:hello</code> · results show score and fields</div>
                </div>
              </div>
            ) : queryResult.hits.length === 0 ? (
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', padding: 24, textAlign: 'center', border: '1px solid var(--border)', borderRadius: 8, background: 'var(--bg-tertiary)' }}>
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 4 }}>No results for “{query}”</div>
                  <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 10 }}>Check spelling, try broader terms, or add documents to the index.</div>
                  <button className="btn btn-sm" onClick={() => { setQuery(''); setQueryResult(null); }}>Clear query</button>
                </div>
              </div>
            ) : (
              <div className="data-table-wrapper" style={{ flex: 1 }}>
                <table className="data-table">
                  <thead>
                    <tr>
                      <th style={{ width: 72 }}>Score</th>
                      <th style={{ width: 140 }}>ID</th>
                      {queryResult.hits[0].fields && Object.keys(queryResult.hits[0].fields).map(k => <th key={k} style={{ minWidth: 120 }}>{k}</th>)}
                    </tr>
                  </thead>
                  <tbody>
                    {queryResult.hits.map(hit => {
                      const tone = scoreTone(hit.score, maxScore);
                      const fieldEntries = hit.fields ? Object.entries(hit.fields) : [];
                      return (
                        <tr key={hit.id}>
                          <td>
                            <span
                              title={`Score ${hit.score.toFixed(4)} — ${tone.label}`}
                              style={{
                                display: 'inline-flex',
                                alignItems: 'center',
                                justifyContent: 'center',
                                minWidth: 52,
                                padding: '2px 7px',
                                borderRadius: 999,
                                fontFamily: 'var(--font-mono)',
                                fontSize: 11,
                                fontWeight: 700,
                                background: tone.bg,
                                color: tone.color,
                                border: `1px solid ${tone.color}22`,
                                fontVariantNumeric: 'tabular-nums',
                              }}
                            >
                              {hit.score.toFixed(2)}
                            </span>
                          </td>
                          <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-primary)', maxWidth: 160, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={hit.id}>
                            {hit.id}
                          </td>
                          {fieldEntries.map(([k, v]) => {
                            const s = String(v ?? '');
                            const snippet = s.length > 96 ? s.slice(0, 96) + '…' : s;
                            return (
                              <td key={k} title={s} style={{ fontSize: 12, lineHeight: 1.5, maxWidth: 260 }}>
                                <span style={{ display: '-webkit-box', WebkitLineClamp: 2 as unknown as number, WebkitBoxOrient: 'vertical', overflow: 'hidden', wordBreak: 'break-word' }}>
                                  {snippet}
                                </span>
                              </td>
                            );
                          })}
                          {fieldEntries.length === 0 && <td style={{ color: 'var(--text-muted)', fontSize: 11 }}>-</td>}
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Create Index */}
      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title="Create Index" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateIndex} disabled={createLoading}>{createLoading ? 'Creating…' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Name <span style={{ color: 'var(--danger)' }}>*</span></label>
          <input className="form-input" value={newIndexName} onChange={e => setNewIndexName(e.target.value)} placeholder="my_index" style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }} />
          <div className="form-hint">Lowercase letters, digits, <code>_</code> and <code>-</code> only. Example: <code>products</code>, <code>docs_v2</code></div>
        </div>
        <div className="form-group">
          <label>Fields <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)' }}>— at least one</span></label>
          {newFields.map((f, idx) => (
            <div key={idx} className="flex gap-2" style={{ marginBottom: 8, alignItems: 'center' }}>
              <input className="form-input" value={f.name} onChange={e => { const c=[...newFields]; c[idx].name=e.target.value; setNewFields(c); }} placeholder="field name" style={{ flex: 1, fontFamily: 'var(--font-mono)', fontSize: 12 }} />
              <select className="form-select" value={f.type} onChange={e => { const c=[...newFields]; c[idx].type=e.target.value; setNewFields(c); }} style={{ flex: 1, maxWidth: 160 }}>
                <option value="text">text — full-text (BM25)</option>
                <option value="keyword">keyword — exact</option>
                <option value="integer">integer</option>
                <option value="float">float</option>
                <option value="boolean">boolean</option>
              </select>
              <button className="btn btn-sm btn-danger" onClick={() => setNewFields(newFields.filter((_, i) => i !== idx))} aria-label="Remove field" title="Remove field">×</button>
            </div>
          ))}
          <button className="btn btn-sm" onClick={() => setNewFields([...newFields, { name: '', type: 'text' }])}>+ Add Field</button>
          <div className="form-hint"><code>text</code> is analyzed for search; <code>keyword</code> is exact-match/filter. Add <code>id</code> automatically if you omit it.</div>
        </div>
        {createError && <div className="callout error">{createError}</div>}
      </Modal>

      {/* Add Documents */}
      <Modal isOpen={showAddDocs} onClose={() => setShowAddDocs(false)} title={`Add Documents — ${selectedIndex ?? ''}`} size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end', alignItems: 'center' }}>
            <span style={{ fontSize: 11, color: docsJsonError ? 'var(--danger)' : 'var(--text-muted)', marginRight: 'auto' }}>
              {docsJsonError ? `⚠ ${docsJsonError}` : `✓ Valid JSON — ${(() => { try { const v = JSON.parse(docsJson); return Array.isArray(v) ? v.length : 1; } catch { return '?'; } })()} doc(s)`}
            </span>
            <button className="btn" onClick={() => setShowAddDocs(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleAddDocs} disabled={!!docsJsonError || addDocsLoading}>{addDocsLoading ? 'Adding…' : 'Add'}</button>
          </div>
        }>
        <div className="form-group">
          <label>JSON Array <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)' }}>— each object needs an `id`</span></label>
          <textarea className="form-input json-editor" value={docsJson} onChange={e => setDocsJson(e.target.value)} rows={10} spellCheck={false} style={{ fontFamily: 'var(--font-mono)', fontSize: 12, lineHeight: 1.6 }} />
          <div className="form-hint">Single object or array. Other keys must match fields defined for the index. Example: <code style={{ fontSize: 11 }}>[{`{"id":"1","title":"Hello","body":"..."}`}]</code></div>
        </div>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 8 }}>
          <button className="btn btn-sm" onClick={() => setDocsJson('[\n  {"id": "1", "title": "Hello world", "body": "This is a test document"}\n]')}>Example</button>
          <button className="btn btn-sm" onClick={() => { try { setDocsJson(JSON.stringify(JSON.parse(docsJson), null, 2)); } catch {} }}>Format JSON</button>
          <span style={{ fontSize: 11, color: 'var(--text-muted)', alignSelf: 'center' }}>Bulk add — send 1 to 1000 docs per request.</span>
        </div>
        {addDocsError && <div className="callout error" style={{ marginTop: 10 }}>{addDocsError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteIndexName} onClose={() => setDeleteIndexName(null)} onConfirm={handleDeleteIndex} title="Delete Index" message={`Delete index "${deleteIndexName}" and all its documents? This cannot be undone.`} confirmText="Delete" variant="danger" />
    </div>
  );
}
