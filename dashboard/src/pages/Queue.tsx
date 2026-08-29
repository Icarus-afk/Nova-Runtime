import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { QueueInfo, QueueMessage } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

export default function QueuePage() {
  const [selectedQueue, setSelectedQueue] = useState<string | null>(null);
  const [messagePage, setMessagePage] = useState(1);
  const [showPublish, setShowPublish] = useState(false);
  const [publishBody, setPublishBody] = useState('{"hello": "world"}');
  const [publishDelay, setPublishDelay] = useState('');
  const [publishPriority, setPublishPriority] = useState('0');
  const [publishStatus, setPublishStatus] = useState<string | null>(null);

  // Create queue
  const [showCreate, setShowCreate] = useState(false);
  const [newQueueName, setNewQueueName] = useState('');
  const [newQueueMaxLen, setNewQueueMaxLen] = useState('');
  const [newQueueMaxSize, setNewQueueMaxSize] = useState('');
  const [newQueueDurable, setNewQueueDurable] = useState(true);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createLoading, setCreateLoading] = useState(false);

  // Delete/purge
  const [deleteQueueName, setDeleteQueueName] = useState<string | null>(null);
  const [showPurgeConfirm, setShowPurgeConfirm] = useState(false);

  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: queues, loading: queuesLoading, refetch: refetchQueues } = useApi<QueueInfo[]>(() => api.getQueues(), []);

  const { data: messagesData, loading: messagesLoading, refetch: refetchMessages } = useApi(
    () => selectedQueue ? api.getQueueMessages(selectedQueue, messagePage) : Promise.resolve(null),
    [selectedQueue, messagePage]
  );

  const selectedInfo = queues?.find(q => q.name === selectedQueue);

  const handleCreateQueue = async () => {
    if (!newQueueName.trim()) {
      setCreateError('Queue name required');
      return;
    }
    setCreateLoading(true);
    setCreateError(null);
    try {
      await api.createQueue(newQueueName.trim(), {
        durable: newQueueDurable,
        max_length: newQueueMaxLen ? parseInt(newQueueMaxLen, 10) : undefined,
        max_message_size: newQueueMaxSize ? parseInt(newQueueMaxSize, 10) : undefined,
      });
      showToast(`Queue ${newQueueName} created`);
      setShowCreate(false);
      setNewQueueName('');
      setNewQueueMaxLen('');
      setNewQueueMaxSize('');
      refetchQueues();
    } catch (err: unknown) {
      setCreateError(err instanceof Error ? err.message : 'Create failed');
    } finally {
      setCreateLoading(false);
    }
  };

  const handleDeleteQueue = async () => {
    if (!deleteQueueName) return;
    try {
      await api.deleteQueue(deleteQueueName);
      showToast(`Queue ${deleteQueueName} deleted`);
      if (selectedQueue === deleteQueueName) setSelectedQueue(null);
      setDeleteQueueName(null);
      refetchQueues();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handlePublish = async () => {
    if (!selectedQueue) return;
    setPublishStatus(null);
    try {
      let body: unknown = publishBody;
      try { body = JSON.parse(publishBody); } catch { /* keep as string */ }
      const delayMs = publishDelay ? parseInt(publishDelay, 10) * 1000 : undefined;
      await api.publishMessage(selectedQueue, typeof body === 'string' ? body : JSON.stringify(body), parseInt(publishPriority, 10) || 0, delayMs ? delayMs / 1000 : undefined);
      showToast('Message published');
      setShowPublish(false);
      setPublishBody('{"hello": "world"}');
      refetchMessages();
    } catch (err: unknown) {
      setPublishStatus(err instanceof Error ? err.message : 'Publish failed');
    }
  };

  const handlePurge = async () => {
    if (!selectedQueue) return;
    try {
      const result = await api.purgeQueue(selectedQueue);
      showToast(`Purged ${result.purged_count} messages`);
      setShowPurgeConfirm(false);
      refetchMessages();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Purge failed', 'error');
    }
  };

  const handleAck = async (msg: QueueMessage) => {
    if (!selectedQueue) return;
    try {
      await api.ackMessage(selectedQueue, msg.id);
      showToast(`Acked ${msg.id.slice(0, 8)}`);
      refetchMessages();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Ack failed', 'error');
    }
  };

  const queueColumns: any[] = [
    { key: 'name', header: 'Name' },
    { key: 'message_count', header: 'Depth', width: '70px' },
    { key: 'ready_count', header: 'Ready', width: '70px' },
    { key: 'reserved_count', header: 'Reserved', width: '80px' },
    { key: 'delayed_count', header: 'Delayed', width: '70px' },
    { key: 'dead_letter_count', header: 'DLQ', width: '60px', render: (v: unknown) => (v as number) > 0 ? <span className="badge badge-warning">{String(v)}</span> : '0' },
    {
      key: 'actions',
      header: '',
      width: '120px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => { setSelectedQueue(row.name as string); setMessagePage(1); }}>View</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteQueueName(row.name as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const messageColumns: any[] = [
    { key: 'id', header: 'ID', width: '140px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{String(v).slice(0, 8)}...</span> },
    { key: 'state', header: 'State', width: '80px', render: (v: unknown) => <span className="badge">{String(v)}</span> },
    { key: 'priority', header: 'Prio', width: '50px' },
    { key: 'attempts', header: 'Attempts', width: '70px' },
    { key: 'enqueued_at', header: 'Enqueued', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'body', header: 'Body', render: (v: unknown) => {
        const s = typeof v === 'string' ? v : JSON.stringify(v);
        return <span title={s} style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{s.length > 60 ? s.slice(0, 60) + '...' : s}</span>;
      }
    },
    {
      key: 'ack',
      header: '',
      width: '80px',
      render: (_: unknown, row: any) => (
        <button className="btn btn-sm btn-primary" onClick={() => handleAck(row as QueueMessage)}>Ack</button>
      ),
    },
  ];

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Queue</h1>
            <p>Durable FIFO/priority queues with delayed delivery and DLQ</p>
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Queue</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>{toast.message}</div>}

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Queues" value={queues?.length ?? '-'} color="accent" loading={queuesLoading} />
        <MetricCard title="Total Messages" value={queues?.reduce((s, q) => s + q.message_count, 0).toLocaleString() ?? '-'} color="info" loading={queuesLoading} />
        <MetricCard title="Delayed" value={queues?.reduce((s, q) => s + q.delayed_count, 0).toLocaleString() ?? '-'} color="warning" loading={queuesLoading} />
        <MetricCard title="DLQ" value={queues?.reduce((s, q) => s + q.dead_letter_count, 0).toLocaleString() ?? '-'} color="danger" loading={queuesLoading} />
      </div>

      <div className="card mb-4">
        <div className="flex justify-between items-center mb-4">
          <div className="card-title" style={{ margin: 0 }}>Queues</div>
          <button className="btn btn-sm" onClick={() => refetchQueues()}>Refresh</button>
        </div>
        <DataTable
          columns={queueColumns}
          data={(queues || []) as unknown as Record<string, unknown>[]}
          loading={queuesLoading}
          onRowClick={(row) => { setSelectedQueue(row.name as string); setMessagePage(1); setShowPublish(false); }}
          emptyMessage="No queues — create one to get started"
        />
      </div>

      {selectedQueue && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Messages: {selectedQueue}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  Visibility: {selectedInfo.visibility_timeout_seconds}s · Retention: {selectedInfo.retention_seconds}s · Max: {selectedInfo.max_length || '∞'}
                  {selectedInfo.dead_letter_queue && ` · DLQ: ${selectedInfo.dead_letter_queue}`}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm btn-primary" onClick={() => setShowPublish(!showPublish)}>{showPublish ? 'Cancel' : '+ Publish'}</button>
              <button className="btn btn-sm" onClick={() => refetchMessages()}>Refresh</button>
              <button className="btn btn-sm btn-danger" onClick={() => setShowPurgeConfirm(true)}>Purge</button>
              <button className="btn btn-sm" onClick={() => setSelectedQueue(null)}>Close</button>
            </div>
          </div>

          {publishStatus && <div className={`callout ${publishStatus.includes('failed') ? 'error' : 'info'}`}>{publishStatus}</div>}

          {showPublish && (
            <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
              <div className="form-group">
                <label>Body (JSON)</label>
                <textarea className="form-input json-editor" value={publishBody} onChange={(e) => setPublishBody(e.target.value)} rows={4} placeholder='{"key": "value"}' />
              </div>
              <div className="form-row">
                <div className="form-group">
                  <label>Priority (0-3, 0=highest)</label>
                  <input className="form-input" value={publishPriority} onChange={e => setPublishPriority(e.target.value)} placeholder="0" type="number" />
                </div>
                <div className="form-group">
                  <label>Delay (seconds)</label>
                  <input className="form-input" value={publishDelay} onChange={e => setPublishDelay(e.target.value)} placeholder="0 (no delay)" type="number" />
                </div>
              </div>
              <button className="btn btn-primary" onClick={handlePublish}>Publish</button>
            </div>
          )}

          <DataTable
            columns={messageColumns}
            data={(messagesData?.data || []) as unknown as Record<string, unknown>[]}
            loading={messagesLoading}
            pagination={messagesData?.pagination}
            onPageChange={setMessagePage}
            emptyMessage="No messages — publish one"
          />
        </div>
      )}

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title="Create Queue" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateQueue} disabled={createLoading}>{createLoading ? 'Creating...' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Name *</label>
          <input className="form-input" value={newQueueName} onChange={e => setNewQueueName(e.target.value)} placeholder="my-queue" />
        </div>
        <div className="form-row">
          <div className="form-group">
            <label>Max Length</label>
            <input className="form-input" value={newQueueMaxLen} onChange={e => setNewQueueMaxLen(e.target.value)} placeholder="10000" type="number" />
          </div>
          <div className="form-group">
            <label>Max Message Size (bytes)</label>
            <input className="form-input" value={newQueueMaxSize} onChange={e => setNewQueueMaxSize(e.target.value)} placeholder="262144" type="number" />
          </div>
        </div>
        <div className="form-group">
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={newQueueDurable} onChange={e => setNewQueueDurable(e.target.checked)} /> Durable
          </label>
          <div className="form-hint">Durable queues persist to storage; non-durable are memory-only</div>
        </div>
        {createError && <div className="callout error">{createError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteQueueName} onClose={() => setDeleteQueueName(null)} onConfirm={handleDeleteQueue} title="Delete Queue" message={`Delete queue "${deleteQueueName}" and all its messages?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={showPurgeConfirm} onClose={() => setShowPurgeConfirm(false)} onConfirm={handlePurge} title="Purge Queue" message={`Purge all messages from "${selectedQueue}"?`} confirmText="Purge" variant="danger" />
    </div>
  );
}
