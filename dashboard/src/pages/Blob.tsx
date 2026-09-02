import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { BucketInfo, BlobObject } from '../types';
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

export default function BlobPage() {
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [objectPage, setObjectPage] = useState(1);
  const [uploadStatus, setUploadStatus] = useState<string | null>(null);

  // Delete bucket/object
  const [deleteBucketName, setDeleteBucketName] = useState<string | null>(null);
  const [deleteObjectKey, setDeleteObjectKey] = useState<string | null>(null);
  const [viewObject, setViewObject] = useState<BlobObject | null>(null);

  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: buckets, loading: bucketsLoading, refetch: refetchBuckets } = useApi<BucketInfo[]>(
    () => api.getBuckets(), []
  );

  const { data: objectsData, loading: objectsLoading, refetch: refetchObjects } = useApi(
    () => selectedBucket ? api.getBucketObjects(selectedBucket, objectPage) : Promise.resolve(null),
    [selectedBucket, objectPage]
  );

  const selectedInfo = buckets?.find(b => b.name === selectedBucket);

  const handleDeleteBucket = async () => {
    if (!deleteBucketName) return;
    try {
      await api.deleteBucket(deleteBucketName);
      showToast(`Bucket ${deleteBucketName} deleted`);
      if (selectedBucket === deleteBucketName) setSelectedBucket(null);
      setDeleteBucketName(null);
      refetchBuckets();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleDeleteObject = async () => {
    if (!deleteObjectKey) return;
    try {
      await api.deleteBlob(deleteObjectKey);
      showToast(`Object ${deleteObjectKey.slice(0, 8)} deleted`);
      setDeleteObjectKey(null);
      setViewObject(null);
      refetchObjects();
      refetchBuckets();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleDownload = async (key: string) => {
    window.open(`/api/v1/blobs/${encodeURIComponent(key)}`, '_blank');
  };

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!selectedBucket || !e.target.files?.length) return;
    setUploadStatus('Uploading...');
    try {
      const file = e.target.files[0];
      if (file.size > 100 * 1024 * 1024) {
        setUploadStatus('File too large (max 100MB in demo)');
        return;
      }
      const result = await api.uploadBlob(selectedBucket, file);
      setUploadStatus(`Uploaded ${file.name} (${formatBytes(result.size_bytes)})`);
      showToast(`Uploaded ${file.name}`);
      refetchObjects();
      refetchBuckets();
    } catch (err: unknown) {
      setUploadStatus(err instanceof Error ? err.message : 'Upload failed');
    }
  };

  const handleView = async (obj: BlobObject) => {
    setViewObject(obj);
    try {
      const info: any = await api.getBlobInfo(obj.key);
      setViewObject({ ...obj, ...info, mime_type: info.content_type || obj.mime_type });
    } catch {}
  };

  const bucketColumns: any[] = [
    { key: 'name', header: 'Bucket / Namespace' },
    { key: 'file_count', header: 'Files', width: '70px', render: (v: unknown) => (v as number).toLocaleString() },
    { key: 'total_size_bytes', header: 'Size', width: '90px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'created_at', header: 'Created', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'actions',
      header: '',
      width: '140px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => { setSelectedBucket(row.name as string); setObjectPage(1); }}>Open</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteBucketName(row.name as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const objectColumns: any[] = [
    { key: 'key', header: 'Key', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{String(v).slice(0, 32)}</span> },
    { key: 'size_bytes', header: 'Size', width: '80px', render: (v: unknown) => formatBytes(v as number) },
    { key: 'mime_type', header: 'Type', width: '110px', render: (v: unknown) => <span className="badge">{String(v) || 'binary'}</span> },
    { key: 'last_modified_at', header: 'Modified', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'actions',
      header: '',
      width: '160px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => handleView(row as BlobObject)}>View</button>
          <button className="btn btn-sm" onClick={() => handleDownload(row.key as string)}>Download</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteObjectKey(row.key as string)}>Delete</button>
        </div>
      ),
    },
  ];

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Blob Storage</h1>
            <p>Namespaces with deduplicated, chunked storage and SHA-256</p>
          </div>
          <label className="btn btn-primary" style={{ cursor: 'pointer' }}>
            Upload File
            <input type="file" style={{ display: 'none' }} onChange={(e) => { if (selectedBucket) handleUpload(e); else setUploadStatus('Select a namespace first'); }} />
          </label>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="grid grid-cols-3 mb-4">
        <MetricCard title="Buckets" value={buckets?.length ?? '-'} color="accent" loading={bucketsLoading} />
        <MetricCard title="Total Files" value={buckets?.reduce((s, b) => s + b.file_count, 0).toLocaleString() ?? '-'} color="info" loading={bucketsLoading} />
        <MetricCard title="Total Size" value={buckets ? formatBytes(buckets.reduce((s, b) => s + b.total_size_bytes, 0)) : '-'} color="success" loading={bucketsLoading} />
      </div>

      <div className="card mb-4">
        <div className="flex justify-between items-center mb-4">
          <div className="card-title" style={{ margin: 0 }}>Buckets (Namespaces)</div>
          <button className="btn btn-sm" onClick={() => refetchBuckets()}>Refresh</button>
        </div>
        <DataTable
          columns={bucketColumns}
          data={(buckets || []) as unknown as Record<string, unknown>[]}
          loading={bucketsLoading}
          onRowClick={(row) => { setSelectedBucket(row.name as string); setObjectPage(1); }}
          emptyMessage="No namespaces yet — upload a file to one to create it"
        />
      </div>

      {selectedBucket && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Objects: {selectedBucket}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  {selectedInfo.file_count} files · {formatBytes(selectedInfo.total_size_bytes)} · Created {new Date(selectedInfo.created_at).toLocaleDateString()}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <label className="btn btn-sm btn-primary" style={{ cursor: 'pointer' }}>
                Upload
                <input type="file" style={{ display: 'none' }} onChange={handleUpload} />
              </label>
              <button className="btn btn-sm" onClick={() => refetchObjects()}>Refresh</button>
              <button className="btn btn-sm" onClick={() => setSelectedBucket(null)}>Close</button>
            </div>
          </div>

          {uploadStatus && <div className={`callout ${uploadStatus.includes('failed') || uploadStatus.includes('Error') ? 'error' : 'info'}`}>{uploadStatus} <button className="btn btn-sm" style={{ marginLeft: 12 }} onClick={() => setUploadStatus(null)}>Dismiss</button></div>}

          <DataTable
            columns={objectColumns}
            data={(objectsData?.data || []) as unknown as Record<string, unknown>[]}
            loading={objectsLoading}
            pagination={objectsData?.pagination}
            onPageChange={setObjectPage}
            onRowClick={(row) => handleView(row as unknown as BlobObject)}
            emptyMessage="No objects — upload a file"
          />
        </div>
      )}

      <Modal isOpen={!!viewObject} onClose={() => setViewObject(null)} title="Object Details" size="md"
        footer={<button className="btn" onClick={() => setViewObject(null)}>Close</button>}>
        {viewObject && (
          <div>
            <div className="detail-row"><span className="detail-label">Key</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)' }}>{viewObject.key}</span></div>
            <div className="detail-row"><span className="detail-label">Size</span><span className="detail-value">{formatBytes(viewObject.size_bytes)}</span></div>
            <div className="detail-row"><span className="detail-label">MIME</span><span className="detail-value">{viewObject.mime_type}</span></div>
            <div className="detail-row"><span className="detail-label">ETag</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)' }}>{viewObject.etag || '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Created</span><span className="detail-value">{new Date(viewObject.created_at).toLocaleString()}</span></div>
            <div className="detail-row"><span className="detail-label">Modified</span><span className="detail-value">{new Date(viewObject.last_modified_at).toLocaleString()}</span></div>
            <div className="flex gap-2" style={{ marginTop: 16 }}>
              <button className="btn btn-primary" onClick={() => handleDownload(viewObject.key)}>Download</button>
              <button className="btn btn-danger" onClick={() => { setDeleteObjectKey(viewObject.key); setViewObject(null); }}>Delete</button>
            </div>
          </div>
        )}
      </Modal>

      <ConfirmDialog isOpen={!!deleteBucketName} onClose={() => setDeleteBucketName(null)} onConfirm={handleDeleteBucket} title="Delete Bucket" message={`Delete bucket "${deleteBucketName}"? Objects remain under namespace but bucket entry will be removed.`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={!!deleteObjectKey} onClose={() => setDeleteObjectKey(null)} onConfirm={handleDeleteObject} title="Delete Object" message={`Delete object ${deleteObjectKey?.slice(0, 16)}...?`} confirmText="Delete" variant="danger" />
    </div>
  );
}
