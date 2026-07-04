import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { IndexInfo, SearchResult } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';

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

  const handleDeleteIndex = async () => {
    if (!selectedIndex) return;
    try {
      await api.deleteIndex(selectedIndex);
      setSelectedIndex(null);
      refetchIndexes();
    } catch {}
  };

  const indexColumns = [
    { key: 'name', header: 'Index Name' },
    { key: 'document_count', header: 'Documents', width: '100px', render: (v: unknown) => (v as number).toLocaleString() },
    { key: 'index_size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'field_count', header: 'Fields', width: '70px' },
    { key: 'average_query_time_ms', header: 'Avg Query', width: '80px', render: (v: unknown) => `${(v as number).toFixed(1)}ms` },
    { key: 'query_count', header: 'Queries', width: '80px', render: (v: unknown) => (v as number).toLocaleString() },
  ];

  const resultColumns = [
    { key: 'score', header: 'Score', width: '70px', render: (v: unknown) => (v as number).toFixed(3) },
    { key: 'id', header: 'ID', width: '180px' },
  ];

  return (
    <div>
      <div className="page-header">
        <h1>Search</h1>
        <p>Manage search indexes and test queries</p>
      </div>

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Indexes" value={indexes?.length ?? '-'} color="accent" loading={indexesLoading} />
        <MetricCard
          title="Total Documents"
          value={indexes?.reduce((s, i) => s + i.document_count, 0).toLocaleString() ?? '-'}
          color="info" loading={indexesLoading}
        />
        <MetricCard
          title="Total Size"
          value={indexes ? formatBytes(indexes.reduce((s, i) => s + i.index_size_bytes, 0)) : '-'}
          color="success" loading={indexesLoading}
        />
      </div>

      <div className="card mb-4">
        <div className="card-title">Indexes</div>
        <DataTable
          columns={indexColumns}
          data={(indexes || []) as unknown as Record<string, unknown>[]}
          loading={indexesLoading}
          onRowClick={(row) => setSelectedIndex(row.name as string)}
          emptyMessage="No search indexes"
        />
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="card">
          <div className="card-title">Test Query</div>
          <div className="flex gap-2 mb-4" style={{ marginTop: 8 }}>
            <select
              className="form-select"
              style={{ width: 160 }}
              value={selectedIndex || ''}
              onChange={(e) => { setSelectedIndex(e.target.value); setQueryResult(null); }}
            >
              <option value="">Select index</option>
              {indexes?.map((idx) => (
                <option key={idx.name} value={idx.name}>{idx.name}</option>
              ))}
            </select>
            <input
              className="form-input"
              style={{ flex: 1 }}
              placeholder="Search query..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
            />
            <button className="btn btn-primary" onClick={handleSearch} disabled={queryLoading || !selectedIndex || !query.trim()}>
              {queryLoading ? 'Searching...' : 'Search'}
            </button>
          </div>

          {selectedIndex && indexes?.find(i => i.name === selectedIndex) && (
            <div className="text-sm text-muted mb-4">
              {(() => {
                const idx = indexes!.find(i => i.name === selectedIndex)!;
                return `${idx.document_count.toLocaleString()} docs · ${formatBytes(idx.index_size_bytes)} · ${idx.average_query_time_ms.toFixed(1)}ms avg latency`;
              })()}
            </div>
          )}

          {selectedIndex && (
            <button className="btn btn-sm btn-danger" onClick={handleDeleteIndex}>Delete Index</button>
          )}
        </div>

        <div className="card">
          <div className="card-title">Results</div>
          {queryError && <div className="page-error">{queryError}</div>}
          {queryResult ? (
            <div>
              <div className="text-sm text-muted mb-2">
                {queryResult.total} results found in {queryResult.execution_time_ms.toFixed(1)}ms
                {queryResult.max_score > 0 && ` · Max score: ${queryResult.max_score.toFixed(3)}`}
              </div>
              {queryResult.hits.length > 0 ? (
                <div className="data-table-wrapper">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th style={{ width: 60 }}>Score</th>
                        <th>ID</th>
                        {queryResult.hits[0].fields && Object.keys(queryResult.hits[0].fields).map((k) => (
                          <th key={k}>{k}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {queryResult.hits.map((hit) => (
                        <tr key={hit.id}>
                          <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{hit.score.toFixed(3)}</td>
                          <td style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{hit.id}</td>
                          {hit.fields && Object.values(hit.fields).map((v, i) => (
                            <td key={i}>{String(v ?? '')}</td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>No results</div>
              )}
            </div>
          ) : (
            <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>
              Select an index and enter a query to search
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
