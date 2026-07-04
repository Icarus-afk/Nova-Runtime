import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { BucketInfo, BlobObject } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

export default function BlobPage() {
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [objectPage, setObjectPage] = useState(1);
  const [uploadStatus, setUploadStatus] = useState<string | null>(null);

  const { data: buckets, loading: bucketsLoading, refetch: refetchBuckets } = useApi<BucketInfo[]>(
    () => api.getBuckets(), []
  );

  const { data: objectsData, loading: objectsLoading, refetch: refetchObjects } = useApi(
    () => selectedBucket ? api.getBucketObjects(selectedBucket, objectPage) : Promise.resolve(null),
    [selectedBucket, objectPage]
  );

  const selectedInfo = buckets?.find(b => b.name === selectedBucket);

  const bucketColumns = [
    { key: 'name', header: 'Bucket Name' },
    { key: 'file_count', header: 'Files', width: '80px', render: (v: unknown) => (v as number).toLocaleString() },
    { key: 'total_size_bytes', header: 'Total Size', width: '100px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'public', header: 'Public', width: '60px', render: (v: unknown) => v ? 'Yes' : 'No' },
    { key: 'versioning_enabled', header: 'Versioning', width: '80px', render: (v: unknown) => v ? 'Enabled' : 'Disabled' },
    { key: 'created_at', header: 'Created', width: '140px', render: (v: unknown) => new Date(v as number).toLocaleString() },
  ];

  const objectColumns = [
    { key: 'key', header: 'Key' },
    { key: 'size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'mime_type', header: 'MIME Type', width: '100px' },
    { key: 'etag', header: 'ETag', width: '80px', render: (v: unknown) => String(v).slice(0, 8) + '...' },
    { key: 'last_modified_at', header: 'Modified', width: '140px', render: (v: unknown) => new Date(v as number).toLocaleString() },
  ];

  const handleDownload = async (key: string) => {
    if (!selectedBucket) return;
    window.open(`/api/v1/dashboard/blob/buckets/${selectedBucket}/objects/${encodeURIComponent(key)}/download`, '_blank');
  };

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!selectedBucket || !e.target.files?.length) return;
    setUploadStatus('Uploading...');
    try {
      const file = e.target.files[0];
      const formData = new FormData();
      formData.append('file', file);
      const token = localStorage.getItem('nova_token');
      const res = await fetch(`/api/v1/dashboard/blob/buckets/${selectedBucket}/objects`, {
        method: 'POST',
        headers: token ? { Authorization: `Bearer ${token}` } : {},
        body: formData,
      });
      if (res.ok) {
        setUploadStatus(`Uploaded ${file.name} successfully`);
        refetchObjects();
      } else {
        setUploadStatus(`Upload failed: ${res.statusText}`);
      }
    } catch (err: unknown) {
      setUploadStatus(err instanceof Error ? err.message : 'Upload failed');
    }
  };

  return (
    <div>
      <div className="page-header">
        <h1>Blob Storage</h1>
        <p>Browse buckets and manage stored files</p>
      </div>

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Buckets" value={buckets?.length ?? '-'} color="accent" loading={bucketsLoading} />
        <MetricCard
          title="Total Files"
          value={buckets?.reduce((s, b) => s + b.file_count, 0).toLocaleString() ?? '-'}
          color="info" loading={bucketsLoading}
        />
        <MetricCard
          title="Total Size"
          value={buckets ? formatBytes(buckets.reduce((s, b) => s + b.total_size_bytes, 0)) : '-'}
          color="success" loading={bucketsLoading}
        />
      </div>

      <div className="card mb-4">
        <div className="card-title">Buckets</div>
        <DataTable
          columns={bucketColumns}
          data={(buckets || []) as unknown as Record<string, unknown>[]}
          loading={bucketsLoading}
          onRowClick={(row) => { setSelectedBucket(row.name as string); setObjectPage(1); }}
          emptyMessage="No buckets created"
        />
      </div>

      {selectedBucket && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Objects: {selectedBucket}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  Max file size: {formatBytes(selectedInfo.max_file_size_bytes)} · Allowed types: {selectedInfo.allowed_mime_types.join(', ') || 'any'}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <label className="btn btn-sm btn-primary" style={{ cursor: 'pointer' }}>
                Upload File
                <input type="file" style={{ display: 'none' }} onChange={handleUpload} />
              </label>
              <button className="btn btn-sm" onClick={() => refetchObjects()}>Refresh</button>
            </div>
          </div>

          {uploadStatus && (
            <div className={`callout ${uploadStatus.includes('failed') || uploadStatus.includes('Error') ? 'error' : 'info'}`}>
              {uploadStatus}
              <button className="btn btn-sm" style={{ marginLeft: 12 }} onClick={() => setUploadStatus(null)}>Dismiss</button>
            </div>
          )}

          <DataTable
            columns={objectColumns}
            data={(objectsData?.data || []) as unknown as Record<string, unknown>[]}
            loading={objectsLoading}
            pagination={objectsData?.pagination}
            onPageChange={setObjectPage}
            onRowClick={(row) => handleDownload(row.key as string)}
            emptyMessage="No objects in this bucket"
          />
        </div>
      )}
    </div>
  );
}
