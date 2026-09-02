import { useState, useEffect, useRef, useCallback } from 'react';
import { useApi } from '../hooks/useApi';
import { api, getToken } from '../api/client';
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
      const entry: LogEntryType = {
        timestamp: msg.timestamp ?? Date.now(),
        level: 'info',
        subsystem: msg.event?.split('.')[0] ?? 'system',
        message: msg.event ?? JSON.stringify(msg),
        fields: { event_id: msg.event_id },
        file: '',
        line: 0,
        trace_id: null,
        span_id: null,
      };
      setStreamEntries((prev) => {
        const next = [...prev, entry];
        if (next.length > 500) return next.slice(-500);
        return next;
      });
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
      const token = getToken();
      const params = new URLSearchParams();
      if (token) params.set('token', token);
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
        <div className="flex justify-between items-center">
          <div>
            <h1>Logs</h1>
            <p>View and stream runtime logs — filter by level, subsystem, or search</p>
          </div>
          <div className="flex items-center gap-2">
            {streaming && (
              <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, fontSize: 11, color: 'var(--success)', fontWeight: 600, background: 'rgba(16,185,129,0.12)', padding: '4px 8px', borderRadius: 999 }}>
                <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--success)', animation: 'pulse 1s infinite' }} /> LIVE
              </span>
            )}
            <span className="text-sm text-muted" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>
              {streaming ? `${streamEntries.length} buffered` : `${historyData?.total_count ?? 0} entries`}
            </span>
          </div>
        </div>
      </div>

      <div className="filter-bar" style={{ flexWrap: 'nowrap', gap: 8 }}>
        <select className="form-select" value={levelFilter} onChange={(e) => setLevelFilter(e.target.value)} style={{ width: 130, minWidth: 130 }}>
          <option value="">All Levels</option>
          {LEVELS.map((l) => (
            <option key={l} value={l}>{l.toUpperCase()}</option>
          ))}
        </select>

        <input
          className="form-input"
          style={{ width: 160, minWidth: 140 }}
          placeholder="Subsystem…"
          value={subsystemFilter}
          onChange={(e) => setSubsystemFilter(e.target.value)}
        />

        <input
          className="form-input"
          style={{ flex: 1, minWidth: 160 }}
          placeholder="Search messages…"
          value={searchFilter}
          onChange={(e) => setSearchFilter(e.target.value)}
        />

        {searchFilter && <button className="btn btn-sm" onClick={() => setSearchFilter('')}>Clear</button>}

        <button
          className={`btn btn-sm ${streaming ? 'btn-danger' : 'btn-primary'}`}
          onClick={toggleStream}
          style={{ flexShrink: 0 }}
        >
          {streaming ? 'Stop Streaming' : 'Live Stream'}
        </button>

        {streaming && (
          <button className="btn btn-sm" style={{ flexShrink: 0 }} onClick={() => setAutoScroll(!autoScroll)}>
            Auto-scroll: {autoScroll ? 'ON' : 'OFF'}
          </button>
        )}
      </div>

      <div className="log-viewer">
        <div className="log-viewer-header">
          <span style={{ width: 48, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: '0.04em' }}>LEVEL</span>
          <span style={{ width: 90, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: '0.04em' }}>SUBSYSTEM</span>
          <span style={{ flex: 1, fontSize: 10, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: '0.04em' }}>MESSAGE</span>
          <span style={{ width: 72, textAlign: 'right', fontSize: 10, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: '0.04em' }}>TIME</span>
        </div>

        <div className="log-viewer-body" ref={logBodyRef} style={{ height: 500 }}>
          {historyLoading && !streaming ? (
            <div className="loading-spinner">Loading logs</div>
          ) : displayEntries.length === 0 ? (
            <div style={{ textAlign: 'center', padding: 48 }}>
              {streaming ? (
                <div>
                  <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 6 }}>Waiting for log entries…</div>
                  <div className="text-sm text-muted">Streaming from <code style={{ background: 'var(--bg-tertiary)', padding: '1px 4px', borderRadius: 3 }}>/ws</code> — logs will appear here in real time.</div>
                </div>
              ) : (
                <div>
                  <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 6 }}>No log history</div>
                  <div className="text-sm text-muted" style={{ marginBottom: 14 }}>No entries match the current filters. Try clearing filters or start a live stream.</div>
                  <button className="btn btn-primary btn-sm" onClick={toggleStream}>Live Stream</button>
                </div>
              )}
            </div>
          ) : (
            displayEntries.map((entry: LogEntryType, i: number) => (
              <div key={streaming ? i : `${entry.timestamp}-${i}`} className="log-entry">
                <span className={`log-level ${entry.level}`} style={{ width: 48 }}>{entry.level}</span>
                <span className="log-subsystem" style={{ width: 90, fontFamily: 'var(--font-mono)', fontSize: 11 }}>{entry.subsystem}</span>
                <span className="log-message">
                  {entry.message}
                  {entry.trace_id && (
                    <span style={{ color: 'var(--text-muted)', marginLeft: 8, fontSize: 10, fontFamily: 'var(--font-mono)' }}>
                      [{entry.trace_id.slice(0, 8)}]
                    </span>
                  )}
                </span>
                <span className="log-timestamp" style={{ width: 72, textAlign: 'right' }}>{formatTime(entry.timestamp)}</span>
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
        <div className="flex items-center justify-between mt-2" style={{ gap: 12 }}>
          <div className="callout info" style={{ margin: 0, padding: '6px 12px', fontSize: 11, flex: 1 }}>
            Streaming live logs{levelFilter && ` · Level: ${levelFilter}`}
            {subsystemFilter && ` · Subsystem: ${subsystemFilter}`}
            {searchFilter && ` · Search: ${searchFilter}`}
            <span style={{ color: 'var(--text-muted)', marginLeft: 8 }}>via WebSocket /ws</span>
          </div>
          <button className="btn btn-sm" onClick={() => setStreamEntries([])} style={{ flexShrink: 0 }}>Clear Buffer</button>
        </div>
      )}
    </div>
  );
}
