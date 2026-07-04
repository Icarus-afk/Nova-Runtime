import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { CollectionInfo, Document, QueryResult } from '../types';
import DataTable from '../components/DataTable';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export default function DatabasePage() {
  const [activeTab, setActiveTab] = useState<'browse' | 'query'>('browse');
  const [selectedCollection, setSelectedCollection] = useState<string | null>(null);
  const [queryInput, setQueryInput] = useState('SELECT * FROM users LIMIT 10');
  const [queryResult, setQueryResult] = useState<QueryResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [docPage, setDocPage] = useState(1);

  const { data: collections, loading: collectionsLoading } = useApi<CollectionInfo[]>(
    () => api.getCollections(), []
  );

  const { data: docsData, loading: docsLoading, refetch: refetchDocs } = useApi(
    () => selectedCollection ? api.getDocuments(selectedCollection, docPage) : Promise.resolve(null),
    [selectedCollection, docPage]
  );

  const handleRunQuery = async () => {
    setQueryLoading(true);
    setQueryError(null);
    try {
      const result = await api.queryDatabase({
        collection: selectedCollection || 'default',
        filter: {},
        limit: 100,
      });
      setQueryResult(result);
    } catch (err: unknown) {
      setQueryError(err instanceof Error ? err.message : 'Query failed');
    } finally {
      setQueryLoading(false);
    }
  };

  const docColumns = [
    { key: 'id', header: 'ID', width: '200px' },
    { key: 'collection', header: 'Collection' },
    { key: 'version', header: 'Version', width: '80px' },
    { key: 'size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'updated_at', header: 'Updated', width: '140px', render: (v: unknown) => new Date(v as number).toLocaleString() },
  ];

  return (
    <div>
      <div className="page-header">
        <h1>Database</h1>
        <p>Browse collections, documents, and run SQL queries</p>
      </div>

      <div className="tabs">
        <button className={`tab ${activeTab === 'browse' ? 'active' : ''}`} onClick={() => setActiveTab('browse')}>Browse</button>
        <button className={`tab ${activeTab === 'query' ? 'active' : ''}`} onClick={() => setActiveTab('query')}>Query</button>
      </div>

      <div className="flex gap-4">
        <div className="schema-sidebar">
          <div className="section-title" style={{ marginBottom: 8 }}>Collections</div>
          {collectionsLoading ? (
            <div className="loading-spinner" style={{ padding: 16 }}>Loading</div>
          ) : (
            (collections || []).map((col) => (
              <div
                key={col.name}
                className={`schema-item ${selectedCollection === col.name ? 'active' : ''}`}
                onClick={() => setSelectedCollection(col.name)}
              >
                <span>{col.name}</span>
                <span className="schema-count">{col.document_count.toLocaleString()}</span>
              </div>
            ))
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
                      <button className="btn btn-sm" onClick={() => { setDocPage(1); refetchDocs(); }}>Refresh</button>
                    </div>
                  </div>
                  <DataTable
                    columns={docColumns}
                    data={(docsData?.data || []) as unknown as Record<string, unknown>[]}
                    loading={docsLoading}
                    pagination={docsData?.pagination}
                    onPageChange={setDocPage}
                    emptyMessage="No documents in this collection"
                  />
                </div>
              ) : (
                <div className="card">
                  <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>
                    Select a collection from the sidebar to browse documents
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
                  />
                </div>
                <div className="flex gap-2">
                  <button className="btn btn-primary" onClick={handleRunQuery} disabled={queryLoading}>
                    {queryLoading ? 'Running...' : 'Run Query'}
                  </button>
                  <button className="btn" onClick={() => setQueryResult(null)}>Clear</button>
                </div>
              </div>

              {queryError && <div className="page-error">{queryError}</div>}

              {queryResult && (
                <div className="card">
                  <div className="flex justify-between mb-2">
                    <span className="card-title">Results</span>
                    <span className="text-sm text-muted">
                      {queryResult.execution_time_ms.toFixed(1)}ms · {queryResult.documents.length} rows
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
    </div>
  );
}
