import { useState, useRef, useCallback } from 'react';
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

function mimeBadge(mime: string): { label: string; tone: string } {
  const m = (mime || '').toLowerCase();
  if (m.startsWith('image/')) return { label: m.split('/')[1] || 'image', tone: 'badge-success' };
  if (m.startsWith('video/')) return { label: 'video', tone: 'badge-warning' };
  if (m.startsWith('text/') || m.includes('json') || m.includes('javascript')) return { label: m.split('/').pop() || 'text', tone: '' };
  if (m.includes('pdf')) return { label: 'pdf', tone: 'badge-danger' };
  return { label: m ? m.split('/').pop() || 'binary' : 'binary', tone: '' };
}

export default function BlobPage() {
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [objectPage, setObjectPage] = useState(1);
  const [uploadStatus, setUploadStatus] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const headerInputRef = useRef<HTMLInputElement>(null);

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
      showToast(`Namespace ${deleteBucketName} cleared`);
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
      showToast(`Object deleted`);
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

  const doUploadFile = useCallback(async (file: File) => {
    if (!selectedBucket) {
      setUploadStatus('Select a namespace first — click a row in Buckets.');
      return;
    }
    if (file.size > 100 * 1024 * 1024) {
      setUploadStatus('File too large — max 100 MB in demo');
      return;
    }
    setUploading(true);
    setUploadStatus(`Uploading ${file.name}…`);
    try {
      const result = await api.uploadBlob(selectedBucket, file);
      setUploadStatus(`Uploaded ${file.name} (${formatBytes(result.size_bytes)})`);
      showToast(`Uploaded ${file.name}`);
      refetchObjects();
      refetchBuckets();
    } catch (err: unknown) {
      setUploadStatus(err instanceof Error ? err.message : 'Upload failed');
    } finally {
      setUploading(false);
    }
  }, [selectedBucket, refetchObjects, refetchBuckets]);

  const handleUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    await doUploadFile(file);
    e.target.value = '';
  };

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const file = e.dataTransfer.files?.[0];
    if (file) await doUploadFile(file);
  }, [doUploadFile]);

  const handleView = async (obj: BlobObject) => {
    setViewObject(obj);
    try {
      const info: any = await api.getBlobInfo(obj.key);
      setViewObject({ ...obj, ...info, mime_type: info.content_type || obj.mime_type, etag: info.checksum_sha256 || obj.etag });
    } catch {}
  };

  const bucketColumns: any[] = [
    {
      key: 'name',
      header: 'Bucket / Namespace',
      render: (v: unknown) => (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{String(v)}</span>
      ),
    },
    { key: 'file_count', header: 'Files', width: '76px', render: (v: unknown) => <span style={{ fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}>{(v as number).toLocaleString()}</span> },
    { key: 'total_size_bytes', header: 'Size', width: '92px', render: (v: unknown) => <span style={{ color: 'var(--text-secondary)', fontSize: 12 }}>{formatBytes(v as number)}</span> },
    { key: 'created_at', header: 'Created', width: '138px', render: (v: unknown) => v ? <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{new Date(v as number).toLocaleDateString()} {new Date(v as number).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span> : <span style={{ color: 'var(--text-muted)' }}>-</span> },
    {
      key: 'actions',
      header: '',
      width: '150px',
      render: (_: unknown, row: any) => {
        const active = selectedBucket === row.name;
        return (
          <div className="actions" onClick={e => e.stopPropagation()}>
            <button className={`btn btn-sm ${active ? '' : 'btn-primary'}`} onClick={() => { setSelectedBucket(row.name as string); setObjectPage(1); setUploadStatus(null); }}>
              {active ? 'Selected' : 'View'}
            </button>
            <button className="btn btn-sm btn-danger" onClick={() => setDeleteBucketName(row.name as string)}>Delete</button>
          </div>
        );
      },
    },
  ];

  const objectColumns: any[] = [
    {
      key: 'key',
      header: 'Key',
      render: (v: unknown) => (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-primary)', wordBreak: 'break-all' }} title={String(v)}>
          {String(v).length > 42 ? String(v).slice(0, 42) + '…' : String(v)}
        </span>
      ),
    },
    { key: 'size_bytes', header: 'Size', width: '84px', render: (v: unknown) => <span style={{ fontVariantNumeric: 'tabular-nums', fontSize: 12 }}>{formatBytes(v as number)}</span> },
    {
      key: 'mime_type',
      header: 'Type',
      width: '110px',
      render: (v: unknown) => {
        const { label, tone } = mimeBadge(String(v || ''));
        return <span className={`badge ${tone}`} style={{ maxWidth: 100, overflow: 'hidden', textOverflow: 'ellipsis' }} title={String(v)}>{label}</span>;
      },
    },
    {
      key: 'etag',
      header: 'ETag',
      width: '108px',
      render: (v: unknown) => v ? <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10, color: 'var(--text-muted)' }} title={String(v)}>{String(v).slice(0, 12)}…</span> : <span style={{ color: 'var(--text-muted)' }}>-</span>,
    },
    { key: 'last_modified_at', header: 'Modified', width: '136px', render: (v: unknown) => v ? <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>{new Date(v as number).toLocaleString()}</span> : <span style={{ color: 'var(--text-muted)' }}>-</span> },
    {
      key: 'actions',
      header: '',
      width: '176px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => handleView(row as BlobObject)}>View</button>
          <button className="btn btn-sm" onClick={() => handleDownload(row.key as string)}>Download</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteObjectKey(row.key as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const totalFiles = buckets?.reduce((s, b) => s + b.file_count, 0) ?? 0;
  const totalBytes = buckets ? buckets.reduce((s, b) => s + b.total_size_bytes, 0) : 0;

  return (
    <div>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 12 }}>
        <div className="flex justify-between items-center" style={{ gap: 12 }}>
          <div>
            <h1 style={{ margin: 0 }}>Blob Storage</h1>
            <p style={{ margin: '2px 0 0', color: 'var(--text-muted)', fontSize: 12.5 }}>Namespaces with deduplicated, chunked storage — SHA-256, streaming</p>
          </div>
          <div className="flex gap-2" style={{ alignItems: 'center' }}>
            {!selectedBucket && <span style={{ fontSize: 11, color: 'var(--text-muted)', maxWidth: 180, textAlign: 'right', lineHeight: 1.3 }}>Select a namespace to upload</span>}
            <label className={`btn btn-primary ${!selectedBucket ? 'btn' : ''}`} style={{ cursor: selectedBucket ? 'pointer' : 'not-allowed', opacity: selectedBucket ? 1 : 0.85, display: 'inline-flex', alignItems: 'center', gap: 6 }} title={selectedBucket ? `Upload to ${selectedBucket}` : 'Select a namespace first'}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden><path d="M12 16V4M12 4l-5 5M12 4l5 5M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></svg>
              Upload File
              <input ref={headerInputRef} type="file" style={{ display: 'none' }} onChange={(e) => { if (selectedBucket) handleUpload(e); else setUploadStatus('Select a namespace first — click a row in Buckets.'); if (headerInputRef.current) headerInputRef.current.value = ''; }} />
            </label>
          </div>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      {/* Stats */}
      <div className="grid grid-cols-3" style={{ gap: 12, marginBottom: 12 }}>
        <MetricCard title="Namespaces" value={buckets?.length ?? '-'} color="accent" loading={bucketsLoading} />
        <MetricCard title="Total Files" value={totalFiles.toLocaleString() ?? '-'} color="info" loading={bucketsLoading} />
        <MetricCard title="Total Size" value={buckets ? formatBytes(totalBytes) : '-'} color="success" loading={bucketsLoading} />
      </div>

      {/* Buckets */}
      <div className="card" style={{ marginBottom: 12, padding: 12 }}>
        <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
          <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>Buckets (Namespaces)</div>
          <div className="flex gap-2" style={{ alignItems: 'center' }}>
            {selectedBucket && <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Selected <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-primary)', fontWeight: 600 }}>{selectedBucket}</span></span>}
            <button className="btn btn-sm" onClick={() => refetchBuckets()}>Refresh</button>
          </div>
        </div>
        <DataTable
          columns={bucketColumns}
          data={(buckets || []) as unknown as Record<string, unknown>[]}
          loading={bucketsLoading}
          onRowClick={(row) => { setSelectedBucket(row.name as string); setObjectPage(1); setUploadStatus(null); }}
          emptyMessage="No namespaces yet — upload a file to create one"
        />
        {!bucketsLoading && (!buckets || buckets.length === 0) && (
          <div style={{ display: 'flex', gap: 8, justifyContent: 'center', alignItems: 'center', marginTop: 10, flexWrap: 'wrap' }}>
            <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Namespaces are created on first upload. Select <code style={{ fontSize: 11 }}>default</code> to start, or upload with <code style={{ fontSize: 11 }}>?namespace=my_app</code>.</span>
          </div>
        )}
      </div>

      {/* Objects in bucket */}
      {selectedBucket && (
        <div className="card" style={{ padding: 12 }}>
          <div className="flex items-center justify-between" style={{ marginBottom: 10, gap: 12, flexWrap: 'wrap' }}>
            <div style={{ minWidth: 0 }}>
              <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5, display: 'flex', alignItems: 'center', gap: 6 }}>
                Objects
                <span style={{ fontFamily: 'var(--font-mono)', textTransform: 'none', letterSpacing: 0, fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>{selectedBucket}</span>
              </div>
              {selectedInfo && (
                <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>
                  {selectedInfo.file_count.toLocaleString()} files · {formatBytes(selectedInfo.total_size_bytes)} · <span style={{ color: 'var(--text-secondary)' }}>{new Date(selectedInfo.created_at).toLocaleDateString()}</span>
                </div>
              )}
            </div>
            <div className="flex gap-2" style={{ alignItems: 'center', flexWrap: 'wrap' }}>
              <label className="btn btn-sm btn-primary" style={{ cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
                <svg width="12" height="12" viewBox="0 0 14 14" fill="none" aria-hidden><path d="M7 3v8M3 7h8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></svg>
                {uploading ? 'Uploading…' : 'Upload'}
                <input ref={fileInputRef} type="file" style={{ display: 'none' }} onChange={handleUpload} disabled={uploading} />
              </label>
              <button className="btn btn-sm" onClick={() => refetchObjects()} disabled={objectsLoading}>{objectsLoading ? 'Loading…' : 'Refresh'}</button>
              <button className="btn btn-sm" onClick={() => { setSelectedBucket(null); setUploadStatus(null); }}>Close</button>
            </div>
          </div>

          {/* Drag-drop zone */}
          <div
            onDragOver={(e) => { e.preventDefault(); setIsDragging(true); }}
            onDragLeave={() => setIsDragging(false)}
            onDrop={handleDrop}
            style={{
              border: `1px dashed ${isDragging ? 'var(--text-primary)' : 'var(--border-strong)'}`,
              borderRadius: 8,
              background: isDragging ? 'var(--bg-hover)' : 'var(--bg-tertiary)',
              padding: '10px 12px',
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              marginBottom: 10,
              transition: 'all 0.12s',
            }}
          >
            <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--bg-secondary)', border: '1px solid var(--border)', display: 'grid', placeItems: 'center', color: 'var(--text-muted)', flexShrink: 0 }}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden><path d="M12 16V4M12 4l-5 5M12 4l5 5M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></svg>
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-secondary)' }}>
                Drop file here or <button className="btn btn-sm" style={{ marginLeft: 4, verticalAlign: 'middle', padding: '2px 8px' }} onClick={() => fileInputRef.current?.click()} disabled={uploading}>Browse…</button>
              </div>
              <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>
                Namespace: <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-primary)', fontWeight: 600 }}>{selectedBucket}</span> · Max 100 MB · Stored deduplicated & chunked
              </div>
            </div>
            <span style={{ fontSize: 10, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: 0.4, fontWeight: 600, flexShrink: 0 }}>{uploading ? 'Uploading…' : 'Drag & Drop'}</span>
          </div>

          {uploadStatus && (
            <div className={`callout ${uploadStatus.includes('failed') || uploadStatus.includes('Error') || uploadStatus.includes('too large') ? 'error' : 'info'}`} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, padding: '8px 10px' }}>
              <span style={{ fontSize: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
                {uploading && <span className="loading-spinner" style={{ padding: 0, width: 14, height: 14, borderWidth: 2 } as any} />}
                {uploadStatus}
              </span>
              <button className="btn btn-sm" style={{ flexShrink: 0 }} onClick={() => setUploadStatus(null)}>Dismiss</button>
            </div>
          )}

          <DataTable
            columns={objectColumns}
            data={(objectsData?.data || []) as unknown as Record<string, unknown>[]}
            loading={objectsLoading}
            pagination={objectsData?.pagination}
            onPageChange={setObjectPage}
            onRowClick={(row) => handleView(row as unknown as BlobObject)}
            emptyMessage="No objects in this namespace — drop a file above or click Upload"
          />
          {!objectsLoading && (objectsData?.data?.length ?? 0) === 0 && (
            <div style={{ display: 'flex', justifyContent: 'center', marginTop: 10 }}>
              <button className="btn btn-sm btn-primary" onClick={() => fileInputRef.current?.click()}>Upload first file</button>
            </div>
          )}
        </div>
      )}

      {!selectedBucket && (
        <div className="card" style={{ padding: '12px 14px', borderStyle: 'dashed', display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--bg-tertiary)', display: 'grid', placeItems: 'center', flexShrink: 0, color: 'var(--text-muted)', border: '1px solid var(--border)' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden><path d="M3 8l6-4 6 4v8l-6 4-6-4V8z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" /><path d="M3 8l6 4 6-4M9 12l6-4M9 12v8" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" /></svg>
          </div>
          <div style={{ fontSize: 12.5, color: 'var(--text-muted)', lineHeight: 1.45 }}>
            Select a namespace above to browse objects, or upload to <code style={{ fontSize: 11 }}>default</code>. Keys are content-addressed (SHA-256) and deduplicated across namespaces.
          </div>
        </div>
      )}

      <Modal isOpen={!!viewObject} onClose={() => setViewObject(null)} title="Object Details" size="md"
        footer={<button className="btn" onClick={() => setViewObject(null)}>Close</button>}>
        {viewObject && (
          <div>
            <div className="detail-row"><span className="detail-label">Key</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)', fontSize: 11, wordBreak: 'break-all' }}>{viewObject.key}</span></div>
            <div className="detail-row"><span className="detail-label">Size</span><span className="detail-value">{formatBytes(viewObject.size_bytes)}</span></div>
            <div className="detail-row"><span className="detail-label">MIME</span><span className="detail-value"><span className={`badge ${mimeBadge(viewObject.mime_type).tone}`}>{viewObject.mime_type || 'binary'}</span></span></div>
            <div className="detail-row"><span className="detail-label">ETag / SHA-256</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)', fontSize: 10 }}>{viewObject.etag ? `${viewObject.etag.slice(0, 32)}…` : '-'}</span></div>
            <div className="detail-row"><span className="detail-label">Created</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{new Date(viewObject.created_at).toLocaleString()}</span></div>
            <div className="detail-row"><span className="detail-label">Modified</span><span className="detail-value" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{new Date(viewObject.last_modified_at).toLocaleString()}</span></div>
            {viewObject.etag && <div className="form-hint" style={{ marginTop: 8 }}>Full SHA-256: <code style={{ fontSize: 11, wordBreak: 'break-all' }}>{viewObject.etag}</code></div>}
            <div className="flex gap-2" style={{ marginTop: 14 }}>
              <button className="btn btn-primary" onClick={() => handleDownload(viewObject.key)}>Download</button>
              <button className="btn btn-danger" onClick={() => { setDeleteObjectKey(viewObject.key); }}>Delete</button>
            </div>
          </div>
        )}
      </Modal>

      <ConfirmDialog isOpen={!!deleteBucketName} onClose={() => setDeleteBucketName(null)} onConfirm={handleDeleteBucket} title="Clear Namespace" message={`Clear namespace "${deleteBucketName}"? This deletes every object in that namespace. This cannot be undone.`} confirmText="Clear Namespace" variant="danger" />
      <ConfirmDialog isOpen={!!deleteObjectKey} onClose={() => setDeleteObjectKey(null)} onConfirm={handleDeleteObject} title="Delete Object" message={`Delete object ${deleteObjectKey?.slice(0, 24)}…? This cannot be undone.`} confirmText="Delete" variant="danger" />
    </div>
  );
}
