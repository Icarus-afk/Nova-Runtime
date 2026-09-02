import { useState, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { CollectionInfo, Document, QueryResult } from '../types';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

const SQL_TYPES = [
  'INTEGER', 'BIGINT', 'SMALLINT', 'TINYINT',
  'FLOAT', 'DOUBLE', 'REAL', 'DECIMAL', 'NUMERIC',
  'TEXT', 'VARCHAR', 'CHAR', 'STRING', 'TINYTEXT', 'LONGTEXT',
  'BOOLEAN', 'BOOL',
  'TIMESTAMP', 'DATETIME', 'DATE', 'TIME',
  'BLOB', 'BINARY', 'VARBINARY', 'NULL',
];

type HistoryEntry = { sql: string; time: number; rows: number; ms: number; kind: 'select' | 'write' };

const TEMPLATES: Array<{ label: string; hint: string; sql: string; params: string }> = [
  { label: 'Browse', hint: 'SELECT * LIMIT 10', sql: 'SELECT * FROM users LIMIT 10', params: '' },
  { label: 'Filter', hint: 'WHERE $1', sql: 'SELECT * FROM users WHERE age > $1 LIMIT 10', params: '[21]' },
  { label: 'Insert', hint: 'VALUES ($1,$2)', sql: 'INSERT INTO users (name, age) VALUES ($1, $2)', params: '["alice", 30]' },
  { label: 'Update', hint: 'SET $1 WHERE $2', sql: 'UPDATE users SET name = $1 WHERE id = $2', params: '["alice", 1]' },
  { label: 'Create Table', hint: 'DDL', sql: 'CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)', params: '' },
];

export default function DatabasePage() {
  const [activeTab, setActiveTab] = useState<'browse' | 'query'>('browse');
  const [selectedCollection, setSelectedCollection] = useState<string | null>(null);
  const [queryInput, setQueryInput] = useState('SELECT * FROM users LIMIT 10');
  const [queryParams, setQueryParams] = useState('');
  const [queryLimit, setQueryLimit] = useState('');
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [docPage, setDocPage] = useState(1);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [helpCollapsed, setHelpCollapsed] = useState(false);
  const [copied, setCopied] = useState(false);

  // Create table
  const [showCreateTable, setShowCreateTable] = useState(false);
  const [newTableName, setNewTableName] = useState('');
  const [newColumns, setNewColumns] = useState<Array<{ name: string; type: string; nullable?: boolean; primaryKey?: boolean; unique?: boolean; autoIncrement?: boolean; default?: string }>>([
    { name: 'id', type: 'INTEGER', primaryKey: true, autoIncrement: true },
    { name: 'name', type: 'TEXT', nullable: true },
    { name: 'age', type: 'INTEGER' },
  ]);
  const [createTableLoading, setCreateTableLoading] = useState(false);
  const [createTableError, setCreateTableError] = useState<string | null>(null);

  // Insert document
  const [showInsert, setShowInsert] = useState(false);
  const [insertJson, setInsertJson] = useState('{\n  "name": "alice",\n  "age": 30\n}');
  const [insertError, setInsertError] = useState<string | null>(null);
  const [insertLoading, setInsertLoading] = useState(false);

  // Edit document
  const [editingDoc, setEditingDoc] = useState<Document | null>(null);
  const [editJson, setEditJson] = useState('');
  const [editError, setEditError] = useState<string | null>(null);

  // Delete confirmations
  const [deleteTableName, setDeleteTableName] = useState<string | null>(null);
  const [deleteDoc, setDeleteDoc] = useState<Document | null>(null);

  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: collections, loading: collectionsLoading, refetch: refetchCollections } = useApi<CollectionInfo[]>(
    () => api.getCollections(), []
  );

  const { data: docsData, loading: docsLoading, refetch: refetchDocs } = useApi(
    () => selectedCollection ? api.getDocuments(selectedCollection, docPage) : Promise.resolve(null),
    [selectedCollection, docPage]
  );

  const { data: schema } = useApi(
    () => selectedCollection ? api.getTableSchema(selectedCollection).catch(() => null) : Promise.resolve(null),
    [selectedCollection]
  ) as any;

  const paramsValidation = useMemo(() => {
    const t = queryParams.trim();
    if (!t) return null;
    try {
      const parsed = JSON.parse(t);
      if (!Array.isArray(parsed)) return { ok: true, hint: 'Will be sent as single-element array — prefer ["a", 30] for $1,$2.' };
      return { ok: true, hint: `${parsed.length} param${parsed.length === 1 ? '' : 's'} → $1…$${parsed.length} · strings need quotes: ["alice", 30]` };
    } catch {
      return { ok: false, hint: 'Invalid JSON — must be an array like [21] or ["alice", 30]' };
    }
  }, [queryParams]);

  const handleRunQuery = async () => {
    if (!queryInput.trim()) { setQueryError('SQL is empty'); return; }
    if (paramsValidation && !paramsValidation.ok) { setQueryError(paramsValidation.hint); return; }
    setQueryLoading(true);
    setQueryError(null);
    try {
      let params: unknown[] | undefined;
      if (queryParams.trim()) {
        const parsed = JSON.parse(queryParams.trim());
        params = Array.isArray(parsed) ? parsed : [parsed];
      }
      const head = queryInput.trim().toLowerCase();
      const isSelect = head.startsWith('select') || head.startsWith('with');
      if (isSelect) {
        const result = await api.queryDatabase({
          collection: queryInput,
          filter: {},
          limit: queryLimit ? parseInt(queryLimit, 10) : undefined,
          params,
        });
        setQueryResult(result);
        setHistory(prev => [{ sql: queryInput.trim(), time: Date.now(), rows: result.documents.length, ms: result.execution_time_ms, kind: 'select' as const }, ...prev].slice(0, 3));
      } else {
        const res = await api.executeSql(queryInput, params);
        const affected = (res as any).affected_rows ?? (res as any).row_count ?? 0;
        const ms = (res as any).execution_time_ms ?? 0;
        setQueryResult({ documents: [], total_count: affected, execution_time_ms: ms, warning: null } as any);
        setHistory(prev => [{ sql: queryInput.trim(), time: Date.now(), rows: affected, ms, kind: 'write' as const }, ...prev].slice(0, 3));
        setQueryError(null);
        refetchCollections();
        if (selectedCollection) refetchDocs();
      }
    } catch (err: unknown) {
      setQueryError(err instanceof Error ? err.message : 'Query failed');
    } finally {
      setQueryLoading(false);
    }
  };

  const handleCreateTable = async () => {
    if (!newTableName.trim()) {
      setCreateTableError('Table name required');
      return;
    }
    setCreateTableLoading(true);
    setCreateTableError(null);
    try {
      await api.createTable(newTableName.trim(), newColumns.filter(c => c.name.trim()));
      showToast(`Table ${newTableName} created`, 'success');
      setShowCreateTable(false);
      setNewTableName('');
      refetchCollections();
    } catch (err: unknown) {
      setCreateTableError(err instanceof Error ? err.message : 'Create failed');
    } finally {
      setCreateTableLoading(false);
    }
  };

  const handleDeleteTable = async () => {
    if (!deleteTableName) return;
    try {
      await api.deleteTable(deleteTableName);
      showToast(`Table ${deleteTableName} deleted`, 'success');
      if (selectedCollection === deleteTableName) setSelectedCollection(null);
      setDeleteTableName(null);
      refetchCollections();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleInsert = async () => {
    if (!selectedCollection) return;
    setInsertLoading(true);
    setInsertError(null);
    try {
      const data = JSON.parse(insertJson);
      await api.insertDocument(selectedCollection, data);
      showToast('Document inserted', 'success');
      setShowInsert(false);
      refetchDocs();
    } catch (err: unknown) {
      setInsertError(err instanceof Error ? err.message : 'Insert failed - check JSON');
    } finally {
      setInsertLoading(false);
    }
  };

  const handleEdit = async () => {
    if (!editingDoc || !selectedCollection) return;
    setEditError(null);
    try {
      const next = JSON.parse(editJson) as Record<string, unknown>;
      const prev = editingDoc.data as Record<string, unknown>;
      const changed = Object.entries(next).filter(([k, v]) => JSON.stringify(v) !== JSON.stringify(prev[k]));
      for (const [k, v] of Object.entries(next)) if (!(k in prev)) if (!changed.find(([ck]) => ck === k)) changed.push([k, v]);
      if (changed.length === 0) {
        setEditError('No changes to save');
        return;
      }
      const setClause = changed.map(([k], i) => `${k} = $${i + 1}`).join(', ');
      const whereIdx = changed.length + 1;
      const params: unknown[] = changed.map(([, v]) => v);
      const idVal: unknown = (editingDoc as any).id;
      const where = `id = $${whereIdx}`;
      params.push(idVal);
      await api.executeSql(`UPDATE ${selectedCollection} SET ${setClause} WHERE ${where}`, params);
      showToast(`Updated ${changed.map(([k]) => k).join(', ')}`, 'success');
      setEditingDoc(null);
      refetchDocs();
    } catch (err: unknown) {
      setEditError(err instanceof Error ? err.message : 'Update failed');
    }
  };

  const handleDeleteDoc = async () => {
    if (!deleteDoc || !selectedCollection) return;
    try {
      await api.deleteDocument(selectedCollection, `id = '${deleteDoc.id}'`);
      showToast('Document deleted', 'success');
      setDeleteDoc(null);
      refetchDocs();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const openEdit = (doc: Document) => {
    setEditingDoc(doc);
    setEditJson(JSON.stringify(doc.data, null, 2));
    setEditError(null);
  };

  const lineCount = queryInput.split('\n').length;

  const docColumns: any[] = [
    { key: 'id', header: 'ID', width: '160px' },
    { key: 'collection', header: 'Collection', width: '120px' },
    {
      key: 'data',
      header: 'Data',
      render: (_: unknown, row: any) => {
        const d = row.data as Record<string, unknown>;
        const preview = JSON.stringify(d);
        return <span title={preview} style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{preview.length > 80 ? preview.slice(0, 80) + '...' : preview}</span>;
      },
    },
    { key: 'updated_at', header: 'Updated', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'actions',
      header: '',
      width: '120px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => openEdit(row as Document)}>Edit</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteDoc(row as Document)}>Delete</button>
        </div>
      ),
    },
  ];

  const isSelectResult = !!queryResult && queryResult.documents.length > 0;
  const isWriteResult = !!queryResult && queryResult.documents.length === 0 && !queryError;
  const resultCols: string[] = isSelectResult ? Object.keys(queryResult!.documents[0].data) : [];

  const copyResults = async () => {
    if (!queryResult) return;
    const payload = JSON.stringify(queryResult.documents.map(d => d.data), null, 2);
    try { await navigator.clipboard.writeText(payload); setCopied(true); setTimeout(() => setCopied(false), 1200); showToast('Copied results JSON'); } catch { showToast('Copy failed', 'error'); }
  };

  return (
    <div>
      <div className="page-header" style={{ marginBottom: 12 }}>
        <div className="flex justify-between items-center">
          <div>
            <h1>Database</h1>
            <p>Browse collections, manage tables and documents, run SQL</p>
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreateTable(true)}>+ Create Table</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="tabs" style={{ marginBottom: 12 }}>
        <button className={`tab ${activeTab === 'browse' ? 'active' : ''}`} onClick={() => setActiveTab('browse')}>Browse</button>
        <button className={`tab ${activeTab === 'query' ? 'active' : ''}`} onClick={() => setActiveTab('query')}>Query</button>
      </div>

      <div className="flex gap-4">
        <div className="schema-sidebar">
          <div className="section-title" style={{ marginBottom: 8, display: 'flex', justifyContent: 'space-between' }}>
            <span>Collections</span>
            <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{collections?.length ?? 0}</span>
          </div>
          {collectionsLoading ? (
            <div className="loading-spinner" style={{ padding: 16 }}>Loading</div>
          ) : (collections || []).length === 0 ? (
            <div className="empty-cta" style={{ padding: 16 }}>
              <p>No tables yet</p>
              <button className="btn btn-sm btn-primary" onClick={() => setShowCreateTable(true)}>Create Table</button>
            </div>
          ) : (
            (collections || []).map((col) => (
              <div
                key={col.name}
                className={`schema-item ${selectedCollection === col.name ? 'active' : ''}`}
                onClick={() => setSelectedCollection(col.name)}
              >
                <span style={{ flex: 1 }}>{col.name}</span>
                <span className="schema-count">{col.document_count.toLocaleString()}</span>
                <button
                  className="btn btn-sm"
                  style={{ marginLeft: 6, padding: '2px 6px', fontSize: 10 }}
                  onClick={(e) => { e.stopPropagation(); setDeleteTableName(col.name); }}
                  title="Delete table"
                >
                  ×
                </button>
              </div>
            ))
          )}
          {schema && (
            <div className="card" style={{ marginTop: 12, padding: 12 }}>
              <div className="text-sm text-muted" style={{ fontWeight: 600, marginBottom: 6 }}>Schema: {selectedCollection}</div>
              <div style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>
                {(schema as any).columns?.map((c: any) => {
                  const flags = [c.is_primary_key && 'PK', !c.nullable && 'NOT NULL', c.unique && 'UNIQUE'].filter(Boolean).join(' · ');
                  return (
                    <div key={c.name} style={{ padding: '4px 0', borderBottom: '1px solid var(--border)' }}>
                      <div className="detail-row" style={{ padding: 0 }}>
                        <span>{c.name}</span>
                        <span style={{ color: 'var(--accent)' }}>{c.type}</span>
                      </div>
                      <div className="text-muted" style={{ fontSize: 9.5, color: flags ? undefined : 'var(--text-muted)' }}>
                        {flags || 'nullable'}
                      </div>
                    </div>
                  );
                }) || <span className="text-muted">No schema</span>}
              </div>
            </div>
          )}
        </div>

        <div style={{ flex: 1, minWidth: 0 }}>
          {activeTab === 'browse' ? (
            <div>
              {selectedCollection ? (
                <div>
                  <div className="flex items-center justify-between mb-4" style={{ marginBottom: 12 }}>
                    <div>
                      <div className="section-title" style={{ marginBottom: 2 }}>{selectedCollection}</div>
                      {collections?.find(c => c.name === selectedCollection) && (
                        <div className="text-sm text-muted">
                          {formatBytes(collections!.find(c => c.name === selectedCollection)!.total_size_bytes)} total · {collections!.find(c => c.name === selectedCollection)!.index_count} indexes
                        </div>
                      )}
                    </div>
                    <div className="flex gap-2">
                      <button className="btn btn-sm btn-primary" onClick={() => setShowInsert(true)}>+ Insert</button>
                      <button className="btn btn-sm" onClick={() => { setDocPage(1); refetchDocs(); }}>Refresh</button>
                    </div>
                  </div>
                  <DataTable
                    columns={docColumns}
                    data={(docsData?.data || []) as unknown as Record<string, unknown>[]}
                    loading={docsLoading}
                    pagination={docsData?.pagination}
                    onPageChange={setDocPage}
                    emptyMessage="No documents — click Insert to add one"
                  />
                </div>
              ) : (
                <div className="card">
                  <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>
                    Select a collection from the sidebar or create a table to browse documents
                  </div>
                </div>
              )}
            </div>
          ) : (
            /* ===================== QUERY TAB REDESIGN ===================== */
            <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
              {/* Templates */}
              <div className="card" style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
                <div className="flex items-center justify-between">
                  <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.4, color: 'var(--text-muted)', textTransform: 'uppercase' }}>Quick templates</span>
                  <span style={{ fontSize: 10, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', background: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 4, border: '1px solid var(--border)' }}>novad SQL · $1 params</span>
                </div>
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                  {TEMPLATES.map(t => (
                    <button
                      key={t.label}
                      className="btn btn-sm"
                      style={{ fontSize: 11, padding: '5px 8px', borderColor: queryInput.trim() === t.sql ? 'var(--text-primary)' : undefined }}
                      onClick={() => { setQueryInput(t.sql); setQueryParams(t.params); setQueryError(null); }}
                      title={t.sql}
                    >
                      <span style={{ fontWeight: 600 }}>{t.label}</span>
                      <span style={{ color: 'var(--text-muted)', fontWeight: 400 }}>· {t.hint}</span>
                    </button>
                  ))}
                  {selectedCollection && (
                    <button className="btn btn-sm" style={{ fontSize: 11 }} onClick={() => { setQueryInput(`SELECT * FROM ${selectedCollection} LIMIT 20`); setQueryParams(''); }}>
                      This table: {selectedCollection}
                    </button>
                  )}
                </div>
                <div style={{ fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5 }}>
                  Single-field <code style={{ background: 'var(--bg-tertiary)', padding: '1px 4px', borderRadius: 3, fontFamily: 'var(--font-mono)', fontSize: 10 }}>UPDATE users SET name=$1 WHERE id=$2</code> is the param-safe pattern — avoids string quoting bugs.
                </div>
              </div>

              {/* Editor + Help (two-column) */}
              <div style={{ display: 'grid', gridTemplateColumns: helpCollapsed ? '1fr 36px' : '1fr 300px', gap: 12, alignItems: 'start' }}>
                {/* Editor card */}
                <div className="card" style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 10 }}>
                  <div className="flex items-center justify-between">
                    <label style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.4, color: 'var(--text-secondary)', textTransform: 'uppercase' }}>SQL Editor</label>
                    <div className="flex gap-2 items-center">
                      <span style={{ fontSize: 10, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>{lineCount} line{lineCount > 1 ? 's' : ''} · ⌘+Enter to run</span>
                    </div>
                  </div>

                  {/* Monospace editor with line-numbers gutter */}
                  <div style={{ display: 'flex', border: '1px solid var(--border)', borderRadius: 6, overflow: 'hidden', background: 'var(--bg-primary)' }}>
                    <div
                      aria-hidden
                      style={{
                        width: 36, flexShrink: 0, background: 'var(--bg-tertiary)', borderRight: '1px solid var(--border)',
                        color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', fontSize: 11, lineHeight: '1.6',
                        padding: '10px 6px', textAlign: 'right', userSelect: 'none', whiteSpace: 'pre'
                      }}
                    >
                      {Array.from({ length: Math.max(lineCount, 6) }, (_, i) => i + 1).join('\n')}
                    </div>
                    <textarea
                      value={queryInput}
                      onChange={e => setQueryInput(e.target.value)}
                      onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') { e.preventDefault(); handleRunQuery(); } }}
                      placeholder={"SELECT * FROM users WHERE age > $1 LIMIT 10\n-- JOIN / GROUP BY / HAVING / ORDER BY / LIKE / IN / BETWEEN supported"}
                      style={{
                        flex: 1, minHeight: 140, border: 'none', outline: 'none', resize: 'vertical',
                        fontFamily: 'var(--font-mono)', fontSize: 12.5, lineHeight: '1.6', padding: '10px 10px',
                        background: 'transparent', color: 'var(--text-primary)'
                      }}
                      rows={6}
                      spellCheck={false}
                    />
                  </div>

                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 140px', gap: 10 }}>
                    <div className="form-group" style={{ marginBottom: 0 }}>
                      <label style={{ fontSize: 11, display: 'flex', justifyContent: 'space-between' }}>
                        <span>Params for $1, $2… — JSON array</span>
                        <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)', fontSize: 10 }}>safe, server-interpolated</span>
                      </label>
                      <input
                        className="form-input"
                        value={queryParams}
                        onChange={e => setQueryParams(e.target.value)}
                        placeholder='[21]  or  ["alice", 30]'
                        style={{
                          fontFamily: 'var(--font-mono)', fontSize: 12,
                          borderColor: paramsValidation && !paramsValidation.ok ? 'var(--danger)' : undefined
                        }}
                      />
                      <div style={{ minHeight: 14, marginTop: 4, fontSize: 11, lineHeight: 1.3, color: paramsValidation && !paramsValidation.ok ? 'var(--danger)' : 'var(--text-muted)' }}>
                        {paramsValidation ? paramsValidation.hint : 'Empty = no params · strings need quotes inside array: ["alice", 30]'}
                      </div>
                    </div>
                    <div className="form-group" style={{ marginBottom: 0 }}>
                      <label style={{ fontSize: 11 }}>Max rows</label>
                      <input className="form-input" value={queryLimit} onChange={e => setQueryLimit(e.target.value)} placeholder="100" type="number" min={1} max={1000} style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }} />
                      <div style={{ marginTop: 4, fontSize: 10, color: 'var(--text-muted)' }}>For SELECT limit · 1–1000</div>
                    </div>
                  </div>

                  <div className="flex gap-2" style={{ flexWrap: 'wrap', alignItems: 'center' }}>
                    <button className="btn btn-primary" onClick={handleRunQuery} disabled={queryLoading} style={{ minWidth: 118 }}>
                      {queryLoading ? (
                        <span className="flex items-center gap-2"><span className="loading-spinner" style={{ padding: 0, width: 14, height: 14 } as any} />Running…</span>
                      ) : 'Run  ⌘↵'}
                    </button>
                    <button className="btn" onClick={() => { setQueryResult(null); setQueryError(null); }} disabled={queryLoading}>Clear results</button>
                    <button className="btn" onClick={() => { setQueryInput('SELECT * FROM users LIMIT 10'); setQueryParams(''); setQueryError(null); }}>Reset</button>
                    {queryResult && (
                      <button className="btn" onClick={copyResults}>{copied ? 'Copied!' : 'Copy results JSON'}</button>
                    )}
                    {queryResult && (
                      <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                        {queryResult.execution_time_ms.toFixed(1)} ms
                        {queryResult.warning ? ` · ${queryResult.warning}` : ''}
                      </span>
                    )}
                  </div>

                  {/* History pills */}
                  {history.length > 0 && (
                    <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', alignItems: 'center', paddingTop: 8, borderTop: '1px solid var(--border)' }}>
                      <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 0.4, color: 'var(--text-muted)', textTransform: 'uppercase' }}>History</span>
                      {history.map((h, i) => (
                        <button
                          key={i}
                          className="btn btn-sm"
                          title={`${h.sql}\n${h.rows} rows · ${h.ms.toFixed(1)}ms — click to restore`}
                          onClick={() => { setQueryInput(h.sql); setQueryError(null); }}
                          style={{ fontFamily: 'var(--font-mono)', fontSize: 10, padding: '3px 7px', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                        >
                          <span style={{ width: 6, height: 6, borderRadius: 999, background: h.kind === 'select' ? 'var(--info)' : 'var(--success)', display: 'inline-block' }} />
                          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{h.sql.length > 34 ? h.sql.slice(0, 34) + '…' : h.sql}</span>
                          <span style={{ color: 'var(--text-muted)' }}>{h.rows}·{h.ms.toFixed(0)}ms</span>
                        </button>
                      ))}
                      <button className="btn btn-sm" style={{ fontSize: 10, padding: '3px 6px' }} onClick={() => setHistory([])} title="Clear history">×</button>
                    </div>
                  )}
                </div>

                {/* Help side card */}
                <div className="card" style={{ padding: helpCollapsed ? 8 : 14, display: 'flex', flexDirection: 'column', gap: 10, position: 'sticky', top: 0 }}>
                  <div className="flex items-center justify-between">
                    {!helpCollapsed && <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.4, color: 'var(--text-muted)', textTransform: 'uppercase' }}>Help</span>}
                    <button className="btn btn-sm" onClick={() => setHelpCollapsed(v => !v)} title={helpCollapsed ? 'Expand help' : 'Collapse help'} style={{ marginLeft: helpCollapsed ? 0 : 'auto', padding: '3px 7px', fontSize: 11 }}>
                      {helpCollapsed ? '›' : '‹'}
                    </button>
                  </div>
                  {!helpCollapsed && (
                    <>
                      <div>
                        <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 6 }}>Supported SQL</div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 11, lineHeight: 1.5 }}>
                          {[
                            { k: 'SELECT', ex: 'SELECT * FROM users WHERE age > $1 LIMIT 10' },
                            { k: 'INSERT', ex: 'INSERT INTO users (name, age) VALUES ($1, $2)' },
                            { k: 'UPDATE', ex: 'UPDATE users SET name=$1 WHERE id=$2' },
                            { k: 'DELETE', ex: 'DELETE FROM users WHERE id = $1' },
                            { k: 'CREATE', ex: 'CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)' },
                            { k: 'DROP', ex: 'DROP TABLE demo' },
                          ].map(r => (
                            <div key={r.k} style={{ display: 'flex', flexDirection: 'column', gap: 1, padding: '4px 6px', background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6 }}>
                              <span style={{ fontWeight: 700, fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--text-primary)' }}>{r.k}</span>
                              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--text-secondary)', wordBreak: 'break-all' }}>{r.ex}</span>
                            </div>
                          ))}
                          <div style={{ color: 'var(--text-muted)', fontSize: 10 }}>Also: <code>JOIN</code> · <code>GROUP BY</code> · <code>HAVING</code> · <code>ORDER BY</code> · <code>LIMIT/OFFSET</code> · <code>LIKE</code> · <code>IN</code> · <code>BETWEEN</code></div>
                        </div>
                      </div>

                      <div style={{ borderTop: '1px solid var(--border)', paddingTop: 10 }}>
                        <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 6 }}>Types</div>
                        <div style={{ display: 'flex', gap: 5, flexWrap: 'wrap' }}>
                          {['INTEGER', 'TEXT', 'FLOAT', 'BOOLEAN'].map(t => (
                            <span key={t} style={{ fontFamily: 'var(--font-mono)', fontSize: 10, padding: '2px 6px', borderRadius: 999, background: 'var(--bg-tertiary)', border: '1px solid var(--border)', color: 'var(--text-secondary)' }}>{t}</span>
                          ))}
                        </div>
                        <div style={{ fontSize: 10, color: 'var(--text-muted)', marginTop: 6 }}>Also <code>VARCHAR</code>/<code>TIMESTAMP</code>/<code>BLOB</code> etc.</div>
                      </div>

                      <div style={{ borderTop: '1px solid var(--border)', paddingTop: 10 }}>
                        <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 4 }}>Params $1, $2…</div>
                        <div style={{ fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.5 }}>
                          Placeholders <code style={{ fontFamily: 'var(--font-mono)', background: 'var(--bg-tertiary)', padding: '1px 4px', borderRadius: 3 }}>$1</code> are replaced <em>server-side</em> (safe, no string concat). Fill <strong>Params</strong> with a JSON array — position matches <code>$1, $2…</code>.
                          <div style={{ marginTop: 6, fontFamily: 'var(--font-mono)', fontSize: 10, background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '6px 8px', color: 'var(--text-secondary)' }}>
                            <div><span style={{ color: 'var(--text-muted)' }}>-- SQL</span> WHERE age {'>'} $1</div>
                            <div><span style={{ color: 'var(--text-muted)' }}>-- Params</span> [21]</div>
                            <div style={{ marginTop: 4 }}><span style={{ color: 'var(--text-muted)' }}>-- Strings need quotes in array</span></div>
                            <div>["alice", 30]</div>
                          </div>
                        </div>
                      </div>
                    </>
                  )}
                  {helpCollapsed && (
                    <div style={{ writingMode: 'vertical-rl' as any, fontSize: 10, letterSpacing: 0.6, color: 'var(--text-muted)', textAlign: 'center', padding: '6px 0' }}>HELP</div>
                  )}
                </div>
              </div>

              {/* Error callout */}
              {queryError && (
                <div className="callout error" style={{ marginBottom: 0, display: 'flex', gap: 10, alignItems: 'flex-start' }}>
                  <span style={{ fontWeight: 700, flexShrink: 0 }}>Query error</span>
                  <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, wordBreak: 'break-word', whiteSpace: 'pre-wrap' }}>{queryError}</span>
                  <button className="btn btn-sm" style={{ marginLeft: 'auto', flexShrink: 0 }} onClick={() => setQueryError(null)}>Dismiss</button>
                </div>
              )}

              {/* Results */}
              {queryResult && !queryError && isWriteResult && (
                <div className="callout info" style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
                  <span style={{ width: 8, height: 8, borderRadius: 999, background: 'var(--success)', display: 'inline-block', flexShrink: 0 }} />
                  <span style={{ fontWeight: 600, fontSize: 13 }}>Success</span>
                  <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, color: 'var(--text-secondary)' }}>
                    Affected: {queryResult.total_count ?? 0} row{(queryResult.total_count ?? 0) === 1 ? '' : 's'} · {queryResult.execution_time_ms.toFixed(1)} ms
                  </span>
                  <span style={{ marginLeft: 'auto', display: 'flex', gap: 6 }}>
                    <button className="btn btn-sm" onClick={() => setQueryResult(null)}>Clear</button>
                  </span>
                </div>
              )}

              {queryResult && isSelectResult && (
                <div className="card" style={{ padding: 0, overflow: 'hidden' }}>
                  <div className="flex items-center justify-between" style={{ padding: '10px 14px', borderBottom: '1px solid var(--border)' }}>
                    <div className="flex items-center gap-2">
                      <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.4, color: 'var(--text-muted)', textTransform: 'uppercase' }}>Results</span>
                      <span style={{ fontSize: 11, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', background: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 999, border: '1px solid var(--border)' }}>
                        {queryResult!.documents.length} row{queryResult!.documents.length === 1 ? '' : 's'} · {queryResult!.execution_time_ms.toFixed(1)} ms
                      </span>
                      {queryResult!.warning && <span style={{ fontSize: 11, color: 'var(--warning)' }}>{queryResult!.warning}</span>}
                    </div>
                    <div className="flex gap-2">
                      <button className="btn btn-sm" onClick={copyResults}>{copied ? 'Copied!' : 'Copy JSON'}</button>
                      <button className="btn btn-sm" onClick={() => setQueryResult(null)}>Clear</button>
                    </div>
                  </div>
                  <div className="data-table-wrapper" style={{ border: 'none', borderRadius: 0 }}>
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th style={{ width: 36 }}>#</th>
                          <th style={{ width: 140 }}>ID</th>
                          {resultCols.map(k => <th key={k}>{k}</th>)}
                          <th style={{ width: 96 }}>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {queryResult!.documents.map((doc, i) => (
                          <tr key={doc.id}>
                            <td style={{ color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', fontSize: 11 }}>{i + 1}</td>
                            <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11, maxWidth: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={doc.id}>{doc.id}</td>
                            {resultCols.map(k => {
                              const raw = doc.data[k];
                              const s = raw === null || raw === undefined ? '' : typeof raw === 'object' ? JSON.stringify(raw) : String(raw);
                              const short = s.length > 64 ? s.slice(0, 64) + '…' : s;
                              return <td key={k} title={s} style={{ fontSize: 12, maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{short}</td>;
                            })}
                            <td>
                              <div className="actions">
                                <button className="btn btn-sm" title="Copy row JSON" onClick={() => { navigator.clipboard.writeText(JSON.stringify(doc.data, null, 2)).then(() => showToast('Row copied')); }}>Copy</button>
                                <button className="btn btn-sm" onClick={() => openEdit(doc)}>Edit</button>
                              </div>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <div style={{ padding: '8px 14px', borderTop: '1px solid var(--border)', display: 'flex', justifyContent: 'space-between', fontSize: 11, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                    <span>{queryResult!.documents.length} rows in this page{queryResult!.total_count != null ? ` · total ${queryResult!.total_count}` : ''}</span>
                    <span>{queryResult!.execution_time_ms.toFixed(1)} ms</span>
                  </div>
                </div>
              )}

              {queryResult && !isSelectResult && !isWriteResult && !queryError && (
                <div className="card" style={{ textAlign: 'center', padding: 20, color: 'var(--text-muted)', fontSize: 12 }}>
                  No rows returned — try a SELECT, or check the success callout above for write ops.
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Create Table Modal */}
      <Modal isOpen={showCreateTable} onClose={() => setShowCreateTable(false)} title="Create Table" size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreateTable(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateTable} disabled={createTableLoading}>{createTableLoading ? 'Creating...' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Table Name</label>
          <input className="form-input" value={newTableName} onChange={e => setNewTableName(e.target.value)} placeholder="users" />
        </div>
        <div className="form-group">
          <label>Columns</label>
          {newColumns.map((col, idx) => (
            <div key={idx} style={{ border: '1px solid var(--border)', borderRadius: 8, padding: 10, marginBottom: 10 }}>
              <div className="flex gap-2" style={{ marginBottom: 8 }}>
                <input className="form-input" value={col.name} onChange={e => { const c=[...newColumns]; c[idx].name=e.target.value; setNewColumns(c); }} placeholder="column name" style={{ flex: 1 }} />
                <select className="form-select" value={col.type} onChange={e => { const c=[...newColumns]; c[idx].type=e.target.value; setNewColumns(c); }} style={{ flex: 1 }}>
                  {SQL_TYPES.map(t => <option key={t} value={t}>{t}</option>)}
                </select>
                <button className="btn btn-sm btn-danger" onClick={() => setNewColumns(newColumns.filter((_, i) => i !== idx))}>×</button>
              </div>
              <div className="flex gap-3" style={{ flexWrap: 'wrap', alignItems: 'center' }}>
                <label className="flex items-center gap-1" style={{ fontSize: 12 }}>
                  <input type="checkbox" checked={!!col.primaryKey} onChange={e => { const c=[...newColumns]; c[idx]={ ...c[idx], primaryKey: e.target.checked }; setNewColumns(c); }} /> PK
                </label>
                <label className="flex items-center gap-1" style={{ fontSize: 12 }}>
                  <input type="checkbox" checked={!!col.unique} onChange={e => { const c=[...newColumns]; c[idx]={ ...c[idx], unique: e.target.checked }; setNewColumns(c); }} /> UNIQUE
                </label>
                <label className="flex items-center gap-1" style={{ fontSize: 12 }}>
                  <input type="checkbox" checked={!!col.autoIncrement} onChange={e => { const c=[...newColumns]; c[idx]={ ...c[idx], autoIncrement: e.target.checked }; setNewColumns(c); }} /> AUTO_INCREMENT
                </label>
                <label className="flex items-center gap-1" style={{ fontSize: 12 }}>
                  <input type="checkbox" checked={col.nullable === false} onChange={e => { const c=[...newColumns]; c[idx]={ ...c[idx], nullable: !e.target.checked }; setNewColumns(c); }} /> NOT NULL
                </label>
                <label className="flex items-center gap-1" style={{ fontSize: 12 }}>
                  DEFAULT
                  <input className="form-input" style={{ width: 130, padding: '3px 6px', fontSize: 12 }} value={col.default ?? ''} placeholder="0 / 'x' / CURRENT_TIMESTAMP" onChange={e => { const c=[...newColumns]; c[idx]={ ...c[idx], default: e.target.value }; setNewColumns(c); }} />
                </label>
              </div>
            </div>
          ))}
          <button className="btn btn-sm" onClick={() => setNewColumns([...newColumns, { name: '', type: 'TEXT' }])}>+ Add Column</button>
        </div>
        {createTableError && <div className="callout error">{createTableError}</div>}
        <div className="text-sm text-muted">Types: INTEGER, FLOAT, TEXT, BOOLEAN, VARCHAR, TIMESTAMP, DATE, BLOB, ... · PK implies NOT NULL</div>
      </Modal>

      {/* Insert Modal */}
      <Modal isOpen={showInsert} onClose={() => setShowInsert(false)} title={`Insert into ${selectedCollection}`} size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowInsert(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleInsert} disabled={insertLoading}>{insertLoading ? 'Inserting...' : 'Insert'}</button>
          </div>
        }>
        <div className="form-group">
          <label>JSON Document</label>
          <textarea className="form-input json-editor" value={insertJson} onChange={e => setInsertJson(e.target.value)} rows={6} />
        </div>
        {insertError && <div className="callout error">{insertError}</div>}
      </Modal>

      {/* Edit Modal */}
      <Modal isOpen={!!editingDoc} onClose={() => setEditingDoc(null)} title="Edit Document" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setEditingDoc(null)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleEdit}>Save</button>
          </div>
        }>
        <div className="form-group">
          <label>JSON</label>
          <textarea className="form-input json-editor" value={editJson} onChange={e => setEditJson(e.target.value)} rows={8} />
        </div>
        {editError && <div className="callout error">{editError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteTableName} onClose={() => setDeleteTableName(null)} onConfirm={handleDeleteTable} title="Delete Table" message={`Delete table "${deleteTableName}" and all its data? This cannot be undone.`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={!!deleteDoc} onClose={() => setDeleteDoc(null)} onConfirm={handleDeleteDoc} title="Delete Document" message={`Delete document ${deleteDoc?.id}?`} confirmText="Delete" variant="danger" />
    </div>
  );
}
