import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { CacheStats, CacheEntry } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';

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

  const { data: stats, loading: statsLoading } = useApi<CacheStats>(() => api.getCacheStats(), []);
  const { data: keysData, loading: keysLoading, refetch: refetchKeys } = useApi(
    () => api.getCacheKeys(search || undefined, page),
    [search, page]
  );

  const handleViewKey = async (key: string) => {
    setSelectedKey(key);
    setValueLoading(true);
    try {
      const res = await fetch(`/api/v1/dashboard/cache/keys/${encodeURIComponent(key)}`, {
        headers: { Authorization: `Bearer ${localStorage.getItem('nova_token') || ''}` },
      });
      const data = await res.json();
      setKeyValue(JSON.stringify(data.value ?? data, null, 2));
    } catch {
      setKeyValue('Error loading value');
    } finally {
      setValueLoading(false);
    }
  };

  const handleClearCache = async () => {
    try {
      await api.clearCache();
      refetchKeys();
    } catch {}
  };

  const keyColumns = [
    { key: 'key', header: 'Key', width: '250px' },
    { key: 'value_size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'ttl_seconds', header: 'TTL', width: '80px', render: (v: unknown) => v ? `${v}s` : '∞' },
    { key: 'access_count', header: 'Access Count', width: '100px' },
    { key: 'last_access_at', header: 'Last Accessed', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
  ];

  const hitRate = stats ? (stats.hit_ratio * 100) : 0;

  return (
    <div>
      <div className="page-header">
        <h1>Cache</h1>
        <p>Monitor cache performance and browse cached entries</p>
      </div>

      <div className="grid grid-cols-4 mb-4">
        <MetricCard
          title="Hit Rate"
          value={hitRate.toFixed(1)} unit="%"
          color={hitRate > 90 ? 'success' : hitRate > 70 ? 'warning' : 'danger'}
          loading={statsLoading}
        />
        <MetricCard
          title="Entries"
          value={stats?.total_entries?.toLocaleString() ?? '-'}
          color="accent" loading={statsLoading}
        />
        <MetricCard
          title="Memory"
          value={stats ? formatBytes(stats.current_size_bytes) : '-'}
          unit={`/ ${stats ? formatBytes(stats.max_size_bytes) : ''}`}
          color="info" loading={statsLoading}
        />
        <MetricCard
          title="Evictions"
          value={stats?.eviction_count?.toLocaleString() ?? '-'}
          color="warning" loading={statsLoading}
        />
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="card">
          <div className="card-title">Cache Statistics</div>
          <div style={{ marginTop: 8 }}>
            <div className="detail-row"><span className="detail-label">Hit Count</span><span className="detail-value">{stats?.hit_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Miss Count</span><span className="detail-value">{stats?.miss_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">TTL Expired</span><span className="detail-value">{stats?.ttl_expired_count?.toLocaleString() ?? '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Oldest Entry</span><span className="detail-value">{stats ? `${Math.round(stats.oldest_entry_age_seconds / 60)}m` : '-'}</span></div>
          </div>
        </div>
        {selectedKey && (
          <div className="card">
            <div className="flex justify-between items-center mb-2">
              <div className="card-title" style={{ margin: 0 }}>Key: {selectedKey}</div>
              <button className="btn btn-sm" onClick={() => { setSelectedKey(null); setKeyValue(null); }}>Close</button>
            </div>
            {valueLoading ? (
              <div className="loading-spinner">Loading</div>
            ) : (
              <div className="value-viewer">{keyValue}</div>
            )}
          </div>
        )}
      </div>

      <div className="card">
        <div className="flex items-center justify-between mb-4">
          <div className="card-title" style={{ margin: 0 }}>Key Browser</div>
          <div className="flex gap-2">
            <input
              className="form-input"
              style={{ width: 200 }}
              placeholder="Search keys..."
              value={search}
              onChange={(e) => { setSearch(e.target.value); setPage(1); }}
            />
            <button className="btn btn-sm btn-danger" onClick={handleClearCache}>Clear All</button>
          </div>
        </div>
        <DataTable
          columns={keyColumns}
          data={(keysData?.data || []) as unknown as Record<string, unknown>[]}
          loading={keysLoading}
          pagination={keysData?.pagination}
          onPageChange={setPage}
          onRowClick={(row) => handleViewKey(row.key as string)}
          emptyMessage="No cached entries"
        />
      </div>
    </div>
  );
}
