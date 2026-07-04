import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { QueueInfo, QueueMessage } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';

export default function QueuePage() {
  const [selectedQueue, setSelectedQueue] = useState<string | null>(null);
  const [messagePage, setMessagePage] = useState(1);
  const [showPublish, setShowPublish] = useState(false);
  const [publishBody, setPublishBody] = useState('{"test": true}');
  const [publishStatus, setPublishStatus] = useState<string | null>(null);

  const { data: queues, loading: queuesLoading } = useApi<QueueInfo[]>(() => api.getQueues(), []);

  const { data: messagesData, loading: messagesLoading, refetch: refetchMessages } = useApi(
    () => selectedQueue ? api.getQueueMessages(selectedQueue, messagePage) : Promise.resolve(null),
    [selectedQueue, messagePage]
  );

  const selectedInfo = queues?.find(q => q.name === selectedQueue);

  const handlePublish = async () => {
    if (!selectedQueue) return;
    setPublishStatus(null);
    try {
      await api.publishMessage(selectedQueue, publishBody);
      setPublishStatus('Message published successfully');
      setShowPublish(false);
      refetchMessages();
    } catch (err: unknown) {
      setPublishStatus(err instanceof Error ? err.message : 'Publish failed');
    }
  };

  const handlePurge = async () => {
    if (!selectedQueue) return;
    try {
      const result = await api.purgeQueue(selectedQueue);
      setPublishStatus(`Purged ${result.purged_count} messages`);
      refetchMessages();
    } catch (err: unknown) {
      setPublishStatus(err instanceof Error ? err.message : 'Purge failed');
    }
  };

  const queueColumns = [
    { key: 'name', header: 'Name' },
    { key: 'message_count', header: 'Depth', width: '80px' },
    { key: 'ready_count', header: 'Ready', width: '70px' },
    { key: 'reserved_count', header: 'Reserved', width: '80px' },
    { key: 'delayed_count', header: 'Delayed', width: '70px' },
    { key: 'buried_count', header: 'Buried', width: '70px' },
    { key: 'enqueue_rate_per_sec', header: 'In Rate', width: '70px', render: (v: unknown) => `${(v as number).toFixed(1)}/s` },
    { key: 'dequeue_rate_per_sec', header: 'Out Rate', width: '70px', render: (v: unknown) => `${(v as number).toFixed(1)}/s` },
  ];

  const messageColumns = [
    { key: 'id', header: 'ID', width: '160px' },
    { key: 'state', header: 'State', width: '90px' },
    { key: 'priority', header: 'Priority', width: '70px' },
    { key: 'attempts', header: 'Attempts', width: '70px' },
    { key: 'enqueued_at', header: 'Enqueued', width: '140px', render: (v: unknown) => new Date(v as number).toLocaleString() },
    { key: 'ttr_seconds', header: 'TTR', width: '60px', render: (v: unknown) => `${v}s` },
    { key: 'body', header: 'Body', render: (v: unknown) => {
      const s = String(v);
      return s.length > 60 ? s.slice(0, 60) + '...' : s;
    }},
  ];

  return (
    <div>
      <div className="page-header">
        <h1>Queue</h1>
        <p>Monitor and manage message queues</p>
      </div>

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Queues" value={queues?.length ?? '-'} color="accent" loading={queuesLoading} />
        <MetricCard
          title="Total Messages"
          value={queues?.reduce((s, q) => s + q.message_count, 0).toLocaleString() ?? '-'}
          color="info" loading={queuesLoading}
        />
        <MetricCard
          title="Total Buried"
          value={queues?.reduce((s, q) => s + q.buried_count, 0).toLocaleString() ?? '-'}
          color="warning" loading={queuesLoading}
        />
        <MetricCard
          title="DLQ Messages"
          value={queues?.reduce((s, q) => s + q.dead_letter_count, 0).toLocaleString() ?? '-'}
          color="danger" loading={queuesLoading}
        />
      </div>

      <div className="card mb-4">
        <div className="card-title">Queues</div>
        <DataTable
          columns={queueColumns}
          data={(queues || []) as unknown as Record<string, unknown>[]}
          loading={queuesLoading}
          onRowClick={(row) => { setSelectedQueue(row.name as string); setMessagePage(1); setShowPublish(false); }}
          emptyMessage="No queues defined"
        />
      </div>

      {selectedQueue && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Messages: {selectedQueue}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  Visibility timeout: {selectedInfo.visibility_timeout_seconds}s · Retention: {selectedInfo.retention_seconds}s · Max length: {selectedInfo.max_length || '∞'}
                  {selectedInfo.dead_letter_queue && ` · DLQ: ${selectedInfo.dead_letter_queue}`}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm" onClick={() => setShowPublish(!showPublish)}>
                {showPublish ? 'Cancel' : 'Publish Test'}
              </button>
              <button className="btn btn-sm btn-danger" onClick={handlePurge}>Purge</button>
            </div>
          </div>

          {publishStatus && (
            <div className={`callout ${publishStatus.includes('failed') || publishStatus.includes('Error') ? 'error' : 'info'}`}>
              {publishStatus}
              <button className="btn btn-sm" style={{ marginLeft: 12 }} onClick={() => setPublishStatus(null)}>Dismiss</button>
            </div>
          )}

          {showPublish && (
            <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
              <div className="form-group">
                <label>Message Body (JSON)</label>
                <textarea className="form-input" value={publishBody} onChange={(e) => setPublishBody(e.target.value)} rows={4} />
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
            emptyMessage="No messages in this queue"
          />

          {selectedInfo && selectedInfo.dead_letter_count > 0 && (
            <div className="dlq-section mt-4">
              <div className="card-title" style={{ color: 'var(--danger)' }}>Dead Letter Queue</div>
              <div className="text-sm mt-2">
                {selectedInfo.dead_letter_count} messages in dead letter queue
                {selectedInfo.dead_letter_queue && (
                  <span> → forwarded to <strong>{selectedInfo.dead_letter_queue}</strong></span>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
