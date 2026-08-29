import { useState } from 'react';
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

  const [deleteIndexName, setDeleteIndexName] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: indexes, loading: indexesLoading, refetch: refetchIndexes } = useApi<IndexInfo[]>(
    () => api.getIndexes(), []
  );

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
      setCreateError('Name required');
      return;
    }
    setCreateLoading(true);
    setCreateError(null);
    try {
      await api.createIndex(newIndexName.trim(), newFields.filter(f => f.name.trim()));
      showToast(`Index ${newIndexName} created`);
      setShowCreate(false);
      setNewIndexName('');
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
      }
      setDeleteIndexName(null);
      refetchIndexes();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleAddDocs = async () => {
    if (!selectedIndex) return;
    setAddDocsError(null);
    try {
      const docs = JSON.parse(docsJson);
      const arr = Array.isArray(docs) ? docs : [docs];
      await api.addDocuments(selectedIndex, arr);
      showToast(`Added ${arr.length} documents to ${selectedIndex}`);
      setShowAddDocs(false);
      refetchIndexes();
    } catch (err: unknown) {
      setAddDocsError(err instanceof Error ? err.message : 'Add failed - check JSON');
    }
  };

  const indexColumns: any[] = [
    { key: 'name', header: 'Index Name' },
    { key: 'document_count', header: 'Docs', width: '80px', render: (v: unknown) => (v as number).toLocaleString() },
    { key: 'index_size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'field_count', header: 'Fields', width: '70px' },
    {
      key: 'actions',
      header: '',
      width: '140px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => setSelectedIndex(row.name as string)}>Open</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(row.name as string)}>Delete</button>
        </div>
      ),
    },
  ];

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Search</h1>
            <p>Full-text search with BM25, fields, and highlighting</p>
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Index</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Indexes" value={indexes?.length ?? '-'} color="accent" loading={indexesLoading} />
        <MetricCard title="Total Docs" value={indexes?.reduce((s, i) => s + i.document_count, 0).toLocaleString() ?? '-'} color="info" loading={indexesLoading} />
        <MetricCard title="Total Size" value={indexes ? formatBytes(indexes.reduce((s, i) => s + i.index_size_bytes, 0)) : '-'} color="success" loading={indexesLoading} />
      </div>

      <div className="card mb-4">
        <div className="flex justify-between items-center mb-4">
          <div className="card-title" style={{ margin: 0 }}>Indexes</div>
          <button className="btn btn-sm" onClick={() => refetchIndexes()}>Refresh</button>
        </div>
        <DataTable
          columns={indexColumns}
          data={(indexes || []) as unknown as Record<string, unknown>[]}
          loading={indexesLoading}
          onRowClick={(row) => setSelectedIndex(row.name as string)}
          emptyMessage="No indexes — create one to start"
        />
      </div>

      {selectedIndex && (
        <div className="grid grid-cols-2 gap-4">
          <div className="card">
            <div className="card-title">Index: {selectedIndex}</div>
            <div className="flex gap-2 mb-4" style={{ marginTop: 8 }}>
              <button className="btn btn-sm btn-primary" onClick={() => setShowAddDocs(true)}>+ Add Documents</button>
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(selectedIndex)}>Delete Index</button>
            </div>
            <div className="form-group">
              <label>Test Query</label>
              <div className="flex gap-2">
                <input className="form-input" style={{ flex: 1 }} placeholder="hello world" value={query} onChange={e => setQuery(e.target.value)} onKeyDown={e => e.key === 'Enter' && handleSearch()} />
                <button className="btn btn-primary" onClick={handleSearch} disabled={queryLoading || !query.trim()}>{queryLoading ? 'Searching...' : 'Search'}</button>
              </div>
            </div>
            {queryError && <div className="callout error">{queryError}</div>}
            {selectedIndex && indexes?.find(i => i.name === selectedIndex) && (
              <div className="text-sm text-muted">
                {(() => {
                  const idx = indexes!.find(i => i.name === selectedIndex)!;
                  return `${idx.document_count.toLocaleString()} docs · ${formatBytes(idx.index_size_bytes)} · ${idx.field_count} fields`;
                })()}
              </div>
            )}
          </div>

          <div className="card">
            <div className="card-title">Results {queryResult && `(${queryResult.total} in ${queryResult.execution_time_ms.toFixed(1)}ms)`}</div>
            {queryResult ? (
              queryResult.hits.length > 0 ? (
                <div className="data-table-wrapper">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th style={{ width: 60 }}>Score</th>
                        <th>ID</th>
                        {queryResult.hits[0].fields && Object.keys(queryResult.hits[0].fields).map(k => <th key={k}>{k}</th>)}
                      </tr>
                    </thead>
                    <tbody>
                      {queryResult.hits.map(hit => (
                        <tr key={hit.id}>
                          <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{hit.score.toFixed(3)}</td>
                          <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{hit.id}</td>
                          {hit.fields && Object.values(hit.fields).map((v, i) => <td key={i}>{String(v ?? '')}</td>)}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>No results for "{query}"</div>
              )
            ) : (
              <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>Enter a query and click Search</div>
            )}
          </div>
        </div>
      )}

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title="Create Index" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateIndex} disabled={createLoading}>{createLoading ? 'Creating...' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Name *</label>
          <input className="form-input" value={newIndexName} onChange={e => setNewIndexName(e.target.value)} placeholder="my_index" />
        </div>
        <div className="form-group">
          <label>Fields</label>
          {newFields.map((f, idx) => (
            <div key={idx} className="flex gap-2" style={{ marginBottom: 8 }}>
              <input className="form-input" value={f.name} onChange={e => { const c=[...newFields]; c[idx].name=e.target.value; setNewFields(c); }} placeholder="field name" style={{ flex: 1 }} />
              <select className="form-select" value={f.type} onChange={e => { const c=[...newFields]; c[idx].type=e.target.value; setNewFields(c); }} style={{ flex: 1 }}>
                <option value="text">text</option>
                <option value="keyword">keyword</option>
                <option value="integer">integer</option>
                <option value="float">float</option>
                <option value="boolean">boolean</option>
              </select>
              <button className="btn btn-sm btn-danger" onClick={() => setNewFields(newFields.filter((_, i) => i !== idx))}>×</button>
            </div>
          ))}
          <button className="btn btn-sm" onClick={() => setNewFields([...newFields, { name: '', type: 'text' }])}>+ Add Field</button>
        </div>
        {createError && <div className="callout error">{createError}</div>}
      </Modal>

      <Modal isOpen={showAddDocs} onClose={() => setShowAddDocs(false)} title={`Add Documents to ${selectedIndex}`} size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowAddDocs(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleAddDocs}>Add</button>
          </div>
        }>
        <div className="form-group">
          <label>JSON Array</label>
          <textarea className="form-input json-editor" value={docsJson} onChange={e => setDocsJson(e.target.value)} rows={8} />
          <div className="form-hint">Each object needs an `id` field; other fields are indexed per index fields</div>
        </div>
        {addDocsError && <div className="callout error">{addDocsError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteIndexName} onClose={() => setDeleteIndexName(null)} onConfirm={handleDeleteIndex} title="Delete Index" message={`Delete index "${deleteIndexName}" and all its documents?`} confirmText="Delete" variant="danger" />
    </div>
  );
}
