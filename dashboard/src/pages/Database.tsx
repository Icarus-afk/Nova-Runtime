import { useState } from 'react';
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

  const handleRunQuery = async () => {
    setQueryLoading(true);
    setQueryError(null);
    try {
      let params: unknown[] | undefined;
      if (queryParams.trim()) {
        const parsed = JSON.parse(queryParams.trim());
        params = Array.isArray(parsed) ? parsed : [parsed];
      }
      const result = await api.queryDatabase({
        collection: queryInput,
        filter: {},
        limit: queryLimit ? parseInt(queryLimit, 10) : undefined,
        params,
      });
      setQueryResult(result);
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
      const data = JSON.parse(editJson);
      // Use UPDATE where id = original id (assume first column is id or use data.id)
      const where = `id = '${editingDoc.id}'`;
      const setClause = Object.entries(data).map(([k, v]) => `${k} = ${typeof v === 'string' ? `'${String(v).replace(/'/g, "''")}'` : String(v)}`).join(', ');
      await api.executeSql(`UPDATE ${selectedCollection} SET ${setClause} WHERE ${where}`);
      showToast('Document updated', 'success');
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

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Database</h1>
            <p>Browse collections, manage tables and documents, run SQL</p>
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreateTable(true)}>+ Create Table</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="tabs">
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
                  <div className="flex items-center justify-between mb-4">
                    <div>
                      <div className="section-title">{selectedCollection}</div>
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
            <div>
              <div className="card" style={{ marginBottom: 16 }}>
                <div className="form-group">
                  <label>SQL Query</label>
                  <textarea
                    className="query-editor"
                    value={queryInput}
                    onChange={(e) => setQueryInput(e.target.value)}
                    rows={6}
                    placeholder="SELECT * FROM users WHERE age > $1 LIMIT 10"
                  />
                </div>
                <div className="grid grid-cols-2 gap-3" style={{ marginBottom: 12 }}>
                  <div className="form-group" style={{ marginBottom: 0 }}>
                    <label>Params (JSON array for $1, $2, ...)</label>
                    <input className="form-input" value={queryParams} onChange={(e) => setQueryParams(e.target.value)} placeholder='[21]' style={{ fontFamily: 'var(--font-mono)' }} />
                  </div>
                  <div className="form-group" style={{ marginBottom: 0 }}>
                    <label>Max rows (optional)</label>
                    <input className="form-input" value={queryLimit} onChange={(e) => setQueryLimit(e.target.value)} placeholder="100" type="number" min={1} max={1000} />
                  </div>
                </div>
                <div className="flex gap-2">
                  <button className="btn btn-primary" onClick={handleRunQuery} disabled={queryLoading}>
                    {queryLoading ? 'Running...' : 'Run Query'}
                  </button>
                  <button className="btn" onClick={() => setQueryResult(null)}>Clear</button>
                  <button className="btn" onClick={() => setQueryInput('SELECT * FROM users LIMIT 10')}>Example</button>
                </div>
                <div className="text-sm text-muted" style={{ marginTop: 8 }}>
                  Supports: SELECT, INSERT, UPDATE, DELETE, CREATE/DROP TABLE. Parameter placeholders ($1, $2...) are interpolated by the server.
                </div>
              </div>

              {queryError && <div className="page-error">{queryError}</div>}

              {queryResult && (
                <div className="card">
                  <div className="flex justify-between mb-2">
                    <span className="card-title">Results</span>
                    <span className="text-sm text-muted">
                      {queryResult.execution_time_ms.toFixed(1)}ms · {queryResult.documents.length} rows {queryResult.warning && `· ${queryResult.warning}`}
                    </span>
                  </div>
                  {queryResult.documents.length > 0 ? (
                    <div className="data-table-wrapper">
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>#</th>
                            <th>ID</th>
                            {Object.keys(queryResult.documents[0].data).map((k) => (
                              <th key={k}>{k}</th>
                            ))}
                            <th>Actions</th>
                          </tr>
                        </thead>
                        <tbody>
                          {queryResult.documents.map((doc, i) => (
                            <tr key={doc.id}>
                              <td>{i + 1}</td>
                              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{doc.id}</td>
                              {Object.keys(queryResult!.documents[0].data).map((k) => (
                                <td key={k}>{String(doc.data[k] ?? '')}</td>
                              ))}
                              <td>
                                <div className="actions">
                                  <button className="btn btn-sm" onClick={() => openEdit(doc)}>Edit</button>
                                  <button className="btn btn-sm btn-danger" onClick={() => setDeleteDoc(doc)}>Delete</button>
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <div className="text-muted" style={{ textAlign: 'center', padding: 20 }}>No results</div>
                  )}
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
