import { useState, useEffect, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { QueueInfo, QueueMessage } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

function fmtDuration(sec: number | null | undefined): string {
  if (sec == null || sec === 0) return '—';
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) {
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return s ? `${m}m ${s}s` : `${m}m`;
  }
  if (sec < 86400) {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    return m ? `${h}h ${m}m` : `${h}h`;
  }
  const d = Math.floor(sec / 86400);
  const h = Math.floor((sec % 86400) / 3600);
  return h ? `${d}d ${h}h` : `${d}d`;
}

function fmtMaxLen(v: number | null | undefined): string {
  if (!v || v === 0) return '∞';
  return v.toLocaleString();
}

export default function QueuePage() {
  const [selectedQueue, setSelectedQueue] = useState<string | null>(null);
  const [messagePage, setMessagePage] = useState(1);
  const [showPublish, setShowPublish] = useState(false);
  const [publishBody, setPublishBody] = useState('{"hello": "world"}');
  const [publishDelay, setPublishDelay] = useState('');
  const [publishPriority, setPublishPriority] = useState('0');
  const [publishStatus, setPublishStatus] = useState<string | null>(null);
  const [publishBodyError, setPublishBodyError] = useState<string | null>(null);
  const [queueStats, setQueueStats] = useState<any>(null);

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

  useEffect(() => {
    if (!selectedQueue) { setQueueStats(null); return; }
    api.getQueueStats(selectedQueue).then(setQueueStats).catch(() => setQueueStats(null));
  }, [selectedQueue]);

  const selectedInfo = queues?.find(q => q.name === selectedQueue);

  const totals = useMemo(() => {
    if (!queues) return { total: 0, delayed: 0, dlq: 0 };
    return {
      total: queues.reduce((s, q) => s + q.message_count, 0),
      delayed: queues.reduce((s, q) => s + q.delayed_count, 0),
      dlq: queues.reduce((s, q) => s + q.dead_letter_count, 0),
    };
  }, [queues]);

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
    setPublishBodyError(null);
    if (!publishBody.trim()) {
      setPublishBodyError('Body cannot be empty');
      return;
    }
    // Validate JSON if it looks like JSON; allow plain strings but warn
    if (publishBody.trim().startsWith('{') || publishBody.trim().startsWith('[')) {
      try { JSON.parse(publishBody); } catch (e) {
        setPublishBodyError(e instanceof Error ? `Invalid JSON: ${e.message}` : 'Invalid JSON');
        return;
      }
    }
    const prio = parseInt(publishPriority, 10);
    if (Number.isNaN(prio) || prio < 0 || prio > 9) {
      setPublishBodyError('Priority must be 0–9 (0 = highest)');
      return;
    }
    if (publishDelay && (Number.isNaN(parseInt(publishDelay, 10)) || parseInt(publishDelay, 10) < 0)) {
      setPublishBodyError('Delay must be ≥ 0');
      return;
    }
    try {
      let body: unknown = publishBody;
      try { body = JSON.parse(publishBody); } catch { /* keep as string */ }
      const delayMs = publishDelay ? parseInt(publishDelay, 10) * 1000 : undefined;
      await api.publishMessage(selectedQueue, typeof body === 'string' ? body : JSON.stringify(body), prio || 0, delayMs ? delayMs / 1000 : undefined);
      showToast('Message published');
      setShowPublish(false);
      setPublishBody('{"hello": "world"}');
      setPublishDelay('');
      setPublishPriority('0');
      refetchMessages();
      api.getQueueStats(selectedQueue).then(setQueueStats).catch(() => {});
    } catch (err: unknown) {
      setPublishStatus(err instanceof Error ? err.message : 'Publish failed');
    }
  };

  const handlePurge = async () => {
    if (!selectedQueue) return;
    try {
      const result = await api.purgeQueue(selectedQueue);
      const n = (result as any)?.purged_count;
      showToast(n != null && n >= 0 ? `Purged ${n} messages` : 'Queue purged');
      setShowPurgeConfirm(false);
      refetchMessages();
      api.getQueueStats(selectedQueue).then(setQueueStats).catch(() => {});
      refetchQueues();
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
    {
      key: 'name',
      header: 'Queue',
      render: (_: unknown, row: any) => {
        const isSelected = row.name === selectedQueue;
        return (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
            <span style={{ width: 6, height: 6, borderRadius: 999, background: isSelected ? 'var(--success)' : 'var(--border-strong)', flexShrink: 0 }} />
            <span style={{ fontWeight: 500, color: isSelected ? 'var(--text-primary)' : 'var(--text-secondary)', fontFamily: 'var(--font-mono)', fontSize: 12.5, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={String(row.name)}>
              {String(row.name)}
            </span>
            {isSelected && <span className="badge" style={{ fontSize: 9, padding: '1px 5px' }}>selected</span>}
          </div>
        );
      },
    },
    {
      key: 'message_count',
      header: 'Depth',
      width: '160px',
      render: (v: unknown, row: any) => {
        const ready = row.ready_count as number;
        const reserved = row.reserved_count as number;
        const delayed = row.delayed_count as number;
        const total = v as number;
        return (
          <div title={`Ready ${ready} · Reserved ${reserved} · Delayed ${delayed}`} style={{ display: 'flex', flexDirection: 'column', lineHeight: 1.25 }}>
            <span style={{ fontWeight: 600, color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontSize: 13 }}>{Number(total).toLocaleString()}</span>
            <span style={{ fontSize: 11, color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>
              {ready} ready · {reserved} res{delayed > 0 ? ` · ${delayed} del` : ''}
            </span>
          </div>
        );
      },
    },
    {
      key: 'dead_letter_count',
      header: 'DLQ',
      width: '80px',
      render: (v: unknown) => (v as number) > 0 ? <span className="badge badge-warning">{String(v)}</span> : <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>0</span>,
    },
    {
      key: 'actions',
      header: '',
      width: '132px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => { setSelectedQueue(row.name as string); setMessagePage(1); setShowPublish(false); setPublishStatus(null); }}>View</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteQueueName(row.name as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const messageColumns: any[] = [
    { key: 'id', header: 'ID', width: '120px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-muted)' }}>{String(v).slice(0, 10)}…</span> },
    { key: 'state', header: 'State', width: '78px', render: (v: unknown) => {
      const s = String(v);
      const map: Record<string, string> = { ready: '', reserved: 'badge-warning', delayed: 'badge-success', buried: 'badge-danger', dead_letter: 'badge-danger' };
      const cls = map[s] ? `badge ${map[s]}` : 'badge';
      return <span className={cls} style={{ fontSize: 10 }}>{s}</span>;
    } },
    { key: 'priority', header: 'Prio', width: '54px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{String(v)}</span> },
    { key: 'attempts', header: 'Tries', width: '66px' },
    { key: 'enqueued_at', header: 'Enqueued', width: '146px', render: (v: unknown) => v ? <span style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{new Date(v as number).toLocaleString()}</span> : '-' },
    {
      key: 'body', header: 'Body', render: (v: unknown) => {
        const s = typeof v === 'string' ? v : JSON.stringify(v);
        return <span title={s} style={{ fontFamily: 'var(--font-mono)', fontSize: 11, color: 'var(--text-secondary)', wordBreak: 'break-all' }}>{s.length > 72 ? s.slice(0, 72) + '…' : s}</span>;
      }
    },
    {
      key: 'ack',
      header: '',
      width: '72px',
      render: (_: unknown, row: any) => (
        <button className="btn btn-sm btn-primary" onClick={() => handleAck(row as QueueMessage)}>Ack</button>
      ),
    },
  ];

  const isQueuesEmpty = !queuesLoading && (!queues || queues.length === 0);
  const isMessagesEmpty = !messagesLoading && selectedQueue && (!messagesData?.data || messagesData.data.length === 0);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      <div className="page-header" style={{ marginBottom: 0 }}>
        <div className="flex justify-between items-center">
          <div>
            <h1>Queue</h1>
            <p>Durable FIFO/priority queues with delayed delivery and DLQ</p>
          </div>
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Queue</button>
        </div>
      </div>

      {toast && <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 0 }}>{toast.message}</div>}

      <div className="grid grid-cols-4" style={{ gap: 12 }}>
        <MetricCard title="Queues" value={queues?.length ?? '-'} color="accent" loading={queuesLoading} />
        <MetricCard title="Total Messages" value={totals.total.toLocaleString()} color="info" loading={queuesLoading} />
        <MetricCard title="Delayed" value={totals.delayed.toLocaleString()} color="warning" loading={queuesLoading} />
        <MetricCard title="DLQ" value={totals.dlq.toLocaleString()} color="danger" loading={queuesLoading} />
      </div>

      <div className="card" style={{ padding: 12 }}>
        <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
          <div className="card-title" style={{ margin: 0 }}>Queues</div>
          <button className="btn btn-sm" onClick={() => refetchQueues()}>Refresh</button>
        </div>
        <DataTable
          columns={queueColumns}
          data={(queues || []) as unknown as Record<string, unknown>[]}
          loading={queuesLoading}
          onRowClick={(row) => { setSelectedQueue(row.name as string); setMessagePage(1); setShowPublish(false); setPublishStatus(null); }}
          emptyMessage={isQueuesEmpty ? 'No queues yet' : 'No queues'}
        />
        {isQueuesEmpty && (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, marginTop: 10, padding: '10px 12px', background: 'var(--bg-primary)', border: '1px dashed var(--border)', borderRadius: 6 }}>
            <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.4 }}>
              <span style={{ fontWeight: 600, color: 'var(--text-secondary)' }}>No queues — create one to get started.</span> Queues are durable FIFO by default; use priority 0–9 for ordering, delay for scheduled delivery.
            </div>
            <button className="btn btn-sm btn-primary" onClick={() => setShowCreate(true)}>Create Queue</button>
          </div>
        )}
        {!isQueuesEmpty && queues && queues.length > 0 && (
          <div style={{ marginTop: 8, fontSize: 11, color: 'var(--text-muted)', display: 'flex', gap: 12, flexWrap: 'wrap' }}>
            <span>Depth = total messages. Hover exact count for breakdown. DLQ flags failed messages after retries.</span>
            <span style={{ marginLeft: 'auto' }}>{queues.length} queue{queues.length !== 1 ? 's' : ''} · click a row to inspect messages</span>
          </div>
        )}
      </div>

      {!selectedQueue ? (
        <div className="card" style={{ padding: 14, display: 'flex', gap: 14, alignItems: 'flex-start', borderStyle: 'dashed' }}>
          <div style={{ width: 32, height: 32, borderRadius: 6, background: 'var(--bg-primary)', border: '1px solid var(--border)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, fontSize: 14 }}>→</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 4 }}>Select a queue to view messages</div>
            <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.5 }}>
              Pick a queue above to inspect messages, publish new ones, or purge. <span style={{ color: 'var(--text-secondary)' }}>Tip:</span> FIFO order is default — lower priority numbers (0 = highest) are delivered first; use delay for future delivery and DLQ for poison-message handling.
            </div>
          </div>
        </div>
      ) : (
        <div className="card" style={{ padding: 12, display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div className="flex items-center justify-between" style={{ gap: 12, flexWrap: 'wrap' }}>
            <div style={{ minWidth: 0 }}>
              <div className="card-title" style={{ margin: 0, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <span style={{ fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-muted)', fontWeight: 600 }}>Messages</span>
                <span style={{ fontSize: 13, color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', fontWeight: 600 }}>{selectedQueue}</span>
                {queueStats && (
                  <span style={{ display: 'inline-flex', gap: 6, alignItems: 'center', marginLeft: 2 }}>
                    <span className="badge" title="Ready messages">{queueStats.available_messages ?? 0} ready</span>
                    <span className="badge" title="In-flight / reserved">{queueStats.in_flight_messages ?? 0} in-flight</span>
                    {(queueStats.delayed_messages ?? 0) > 0 && <span className="badge badge-warning">{queueStats.delayed_messages} delayed</span>}
                    {(queueStats.dlq_messages ?? 0) > 0 && <span className="badge badge-danger">{queueStats.dlq_messages} dlq</span>}
                  </span>
                )}
              </div>
              {selectedInfo && (
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap', marginTop: 8 }}>
                  <span className="badge" title="Visibility timeout">visibility {fmtDuration(selectedInfo.visibility_timeout_seconds)}</span>
                  <span className="badge" title="Retention">retention {fmtDuration(selectedInfo.retention_seconds)}</span>
                  <span className="badge">max {fmtMaxLen(selectedInfo.max_length)}</span>
                  {selectedInfo.dead_letter_queue && <span className="badge badge-warning">DLQ: {selectedInfo.dead_letter_queue}</span>}
                </div>
              )}
            </div>
            <div className="flex gap-2" style={{ flexShrink: 0 }}>
              <button className="btn btn-sm btn-primary" onClick={() => { setShowPublish(v => !v); setPublishStatus(null); setPublishBodyError(null); }}>{showPublish ? 'Cancel' : '+ Publish'}</button>
              <button className="btn btn-sm" onClick={() => { refetchMessages(); if (selectedQueue) api.getQueueStats(selectedQueue).then(setQueueStats).catch(() => {}); refetchQueues(); }}>Refresh</button>
              <button className="btn btn-sm btn-danger" onClick={() => setShowPurgeConfirm(true)}>Purge</button>
              <button className="btn btn-sm" onClick={() => { setSelectedQueue(null); setShowPublish(false); }}>Close</button>
            </div>
          </div>

          {publishStatus && <div className={`callout ${publishStatus.toLowerCase().includes('fail') ? 'error' : 'info'}`} style={{ margin: 0 }}>{publishStatus}</div>}

          {showPublish && (
            <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: 12 }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
                <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)', letterSpacing: '-0.01em' }}>Publish message</div>
                <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>JSON · priority · delay</span>
              </div>
              <div className="form-group" style={{ marginBottom: 10 }}>
                <label>Body <span style={{ textTransform: 'none', fontWeight: 400, color: 'var(--text-muted)', letterSpacing: 0 }}>(JSON or text)</span></label>
                <textarea
                  className="form-input json-editor"
                  value={publishBody}
                  onChange={(e) => { setPublishBody(e.target.value); if (publishBodyError) setPublishBodyError(null); }}
                  rows={4}
                  placeholder='{"task":"send_email","to":"user@example.com"}'
                  style={{ minHeight: 96 }}
                />
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 6, flexWrap: 'wrap' }}>
                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Example:</span>
                  <button
                    type="button"
                    className="badge"
                    style={{ cursor: 'pointer' }}
                    onClick={() => setPublishBody('{"task":"send_email","to":"user@example.com","priority":0}')}
                    title="Fill example"
                  >{"{\"task\": \"send_email\"}"}</button>
                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>valid JSON recommended; plain text also accepted</span>
                </div>
                {publishBodyError && <div className="form-error" style={{ marginTop: 6 }}>{publishBodyError}</div>}
              </div>
              <div className="form-row" style={{ gap: 12 }}>
                <div className="form-group" style={{ marginBottom: 0 }}>
                  <label>Priority <span style={{ textTransform: 'none', fontWeight: 400, color: 'var(--text-muted)', letterSpacing: 0 }}>(0 highest)</span></label>
                  <input className="form-input" value={publishPriority} onChange={e => setPublishPriority(e.target.value)} placeholder="0" type="number" min={0} max={9} />
                  <div className="form-hint">0–9, lower is higher priority</div>
                </div>
                <div className="form-group" style={{ marginBottom: 0 }}>
                  <label>Delay <span style={{ textTransform: 'none', fontWeight: 400, color: 'var(--text-muted)', letterSpacing: 0 }}>(seconds)</span></label>
                  <input className="form-input" value={publishDelay} onChange={e => setPublishDelay(e.target.value)} placeholder="0 (no delay)" type="number" min={0} />
                  <div className="form-hint">Message becomes visible after delay</div>
                </div>
              </div>
              <div style={{ display: 'flex', gap: 8, marginTop: 12, justifyContent: 'flex-end' }}>
                <button className="btn" onClick={() => { setShowPublish(false); setPublishBodyError(null); }}>Cancel</button>
                <button className="btn btn-primary" onClick={handlePublish}>Publish</button>
              </div>
            </div>
          )}

          <DataTable
            columns={messageColumns}
            data={(messagesData?.data || []) as unknown as Record<string, unknown>[]}
            loading={messagesLoading}
            pagination={messagesData?.pagination}
            onPageChange={setMessagePage}
            emptyMessage={isMessagesEmpty ? 'No messages' : 'No messages'}
          />
          {isMessagesEmpty && (
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, padding: '10px 12px', background: 'var(--bg-primary)', border: '1px dashed var(--border)', borderRadius: 6, marginTop: -4 }}>
              <div style={{ fontSize: 12, color: 'var(--text-muted)' }}>
                <span style={{ fontWeight: 600, color: 'var(--text-secondary)' }}>No messages — publish one.</span> Messages are durable and ordered by priority, then FIFO.
              </div>
              {!showPublish && <button className="btn btn-sm btn-primary" onClick={() => setShowPublish(true)}>Publish message</button>}
            </div>
          )}
        </div>
      )}

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title="Create Queue" size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateQueue} disabled={createLoading}>{createLoading ? 'Creating…' : 'Create'}</button>
          </div>
        }>
        <div className="form-group">
          <label>Name *</label>
          <input className="form-input" value={newQueueName} onChange={e => setNewQueueName(e.target.value)} placeholder="my-queue" />
          <div className="form-hint">Lowercase, letters/digits/_-. Keep it descriptive (e.g. emails, jobs)</div>
        </div>
        <div className="form-row">
          <div className="form-group">
            <label>Max Length</label>
            <input className="form-input" value={newQueueMaxLen} onChange={e => setNewQueueMaxLen(e.target.value)} placeholder="∞ (unlimited)" type="number" min={0} />
            <div className="form-hint">Max queued messages</div>
          </div>
          <div className="form-group">
            <label>Max Message Size (bytes)</label>
            <input className="form-input" value={newQueueMaxSize} onChange={e => setNewQueueMaxSize(e.target.value)} placeholder="262144" type="number" min={0} />
            <div className="form-hint">Rejects oversized publishes</div>
          </div>
        </div>
        <div className="form-group" style={{ marginBottom: 0 }}>
          <label className="flex items-center gap-2" style={{ textTransform: 'none', letterSpacing: 0, fontWeight: 500 }}>
            <input type="checkbox" checked={newQueueDurable} onChange={e => setNewQueueDurable(e.target.checked)} /> Durable
          </label>
          <div className="form-hint">Durable queues persist to storage; non-durable are memory-only</div>
        </div>
        {createError && <div className="callout error" style={{ marginTop: 12 }}>{createError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteQueueName} onClose={() => setDeleteQueueName(null)} onConfirm={handleDeleteQueue} title="Delete Queue" message={`Delete queue "${deleteQueueName}" and all its messages?`} confirmText="Delete" variant="danger" />
      <ConfirmDialog isOpen={showPurgeConfirm} onClose={() => setShowPurgeConfirm(false)} onConfirm={handlePurge} title="Purge Queue" message={`Purge all messages from "${selectedQueue}"?`} confirmText="Purge" variant="danger" />
    </div>
  );
}
