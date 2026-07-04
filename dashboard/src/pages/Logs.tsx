import { useState, useEffect, useRef, useCallback } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { LogEntry as LogEntryType } from '../types';

const LEVELS = ['trace', 'debug', 'info', 'warn', 'error', 'fatal'] as const;

export default function LogsPage() {
  const [streaming, setStreaming] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [streamEntries, setStreamEntries] = useState<LogEntryType[]>([]);
  const [levelFilter, setLevelFilter] = useState<string>('');
  const [subsystemFilter, setSubsystemFilter] = useState('');
  const [searchFilter, setSearchFilter] = useState('');
  const [historyPage, setHistoryPage] = useState(1);
  const wsRef = useRef<WebSocket | null>(null);
  const logBodyRef = useRef<HTMLDivElement>(null);

  const { data: historyData, loading: historyLoading } = useApi(
    () => api.getLogs({
      levels: levelFilter || undefined,
      subsystems: subsystemFilter || undefined,
      search: searchFilter || undefined,
      limit: 100,
      offset: (historyPage - 1) * 100,
      order: 'desc',
    }),
    [levelFilter, subsystemFilter, searchFilter, historyPage]
  );

  const handleWsMessage = useCallback((event: MessageEvent) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.type === 'log_entry' && msg.data) {
        setStreamEntries((prev) => {
          const next = [...prev, msg.data];
          if (next.length > 500) return next.slice(-500);
          return next;
        });
      }
    } catch {}
  }, []);

  const toggleStream = useCallback(() => {
    if (streaming) {
      wsRef.current?.close();
      wsRef.current = null;
      setStreaming(false);
    } else {
      setStreamEntries([]);
      const wsUrl = api.getWsUrl();
      const params = new URLSearchParams();
      if (levelFilter) params.set('levels', levelFilter);
      if (subsystemFilter) params.set('subsystems', subsystemFilter);
      const url = `${wsUrl}?${params.toString()}`;
      const ws = new WebSocket(url);
      ws.onmessage = handleWsMessage;
      ws.onopen = () => setStreaming(true);
      ws.onclose = () => setStreaming(false);
      wsRef.current = ws;
    }
  }, [streaming, levelFilter, subsystemFilter, handleWsMessage]);

  useEffect(() => {
    return () => {
      wsRef.current?.close();
    };
  }, []);

  useEffect(() => {
    if (autoScroll && logBodyRef.current) {
      logBodyRef.current.scrollTop = logBodyRef.current.scrollHeight;
    }
  }, [streamEntries, autoScroll]);

  const displayEntries = streaming ? streamEntries : (historyData?.entries || []);

  const formatTime = (ts: number) => {
    const d = new Date(ts);
    return d.toLocaleTimeString('en-US', { hour12: false });
  };

  return (
    <div>
      <div className="page-header">
        <h1>Logs</h1>
        <p>View and stream runtime logs</p>
      </div>

      <div className="filter-bar">
        <select className="form-select" value={levelFilter} onChange={(e) => setLevelFilter(e.target.value)}>
          <option value="">All Levels</option>
          {LEVELS.map((l) => (
            <option key={l} value={l}>{l.toUpperCase()}</option>
          ))}
        </select>

        <input
          className="form-input"
          style={{ minWidth: 140 }}
          placeholder="Subsystem..."
          value={subsystemFilter}
          onChange={(e) => setSubsystemFilter(e.target.value)}
        />

        <input
          className="form-input"
          style={{ flex: 1, minWidth: 200 }}
          placeholder="Search logs..."
          value={searchFilter}
          onChange={(e) => setSearchFilter(e.target.value)}
        />

        <button
          className={`btn btn-sm ${streaming ? 'btn-danger' : 'btn-primary'}`}
          onClick={toggleStream}
        >
          {streaming ? 'Stop Streaming' : 'Live Stream'}
        </button>

        {streaming && (
          <button className="btn btn-sm" onClick={() => setAutoScroll(!autoScroll)}>
            Auto-scroll: {autoScroll ? 'ON' : 'OFF'}
          </button>
        )}

        {!streaming && (
          <span className="text-sm text-muted">
            {historyData?.total_count ?? 0} entries
          </span>
        )}

        {streaming && (
          <span className="text-sm text-muted">
            {streamEntries.length} entries
          </span>
        )}
      </div>

      <div className="log-viewer">
        <div className="log-viewer-header">
          <span style={{ width: 36, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)' }}>LEVEL</span>
          <span style={{ width: 80, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)' }}>SUBSYSTEM</span>
          <span style={{ flex: 1, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)' }}>MESSAGE</span>
          <span style={{ fontSize: 10, fontWeight: 600, color: 'var(--text-muted)' }}>TIME</span>
        </div>

        <div className="log-viewer-body" ref={logBodyRef}>
          {historyLoading && !streaming ? (
            <div className="loading-spinner">Loading logs</div>
          ) : displayEntries.length === 0 ? (
            <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>
              {streaming ? 'Waiting for log entries...' : 'No log entries found'}
            </div>
          ) : (
            displayEntries.map((entry, i) => (
              <div key={streaming ? i : `${entry.timestamp}-${i}`} className="log-entry">
                <span className={`log-level ${entry.level}`}>{entry.level}</span>
                <span className="log-subsystem">{entry.subsystem}</span>
                <span className="log-message">
                  {entry.message}
                  {entry.trace_id && (
                    <span style={{ color: 'var(--text-muted)', marginLeft: 8, fontSize: 10 }}>
                      [{entry.trace_id.slice(0, 8)}]
                    </span>
                  )}
                </span>
                <span className="log-timestamp">{formatTime(entry.timestamp)}</span>
              </div>
            ))
          )}
        </div>
      </div>

      {!streaming && historyData?.has_more && (
        <div className="flex justify-center mt-4">
          <button className="btn btn-sm" onClick={() => setHistoryPage(historyPage + 1)}>
            Load More
          </button>
        </div>
      )}

      {streaming && (
        <div className="flex items-center justify-between mt-2">
          <div className="callout info" style={{ margin: 0, padding: '6px 12px' }}>
            Streaming live logs{levelFilter && ` · Level: ${levelFilter}`}
            {subsystemFilter && ` · Subsystem: ${subsystemFilter}`}
          </div>
          <button className="btn btn-sm" onClick={() => setStreamEntries([])}>Clear Buffer</button>
        </div>
      )}
    </div>
  );
}
