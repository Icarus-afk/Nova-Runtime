import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { SystemHealth } from '../types';
import MetricCard from '../components/MetricCard';
import StatusBadge from '../components/StatusBadge';
import { CheckCircleIcon, XCircleIcon, AlertTriangleIcon, InfoIcon } from '../components/Icons';
import { Link } from 'react-router-dom';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function ProgressBar({ percent, color }: { percent: number; color: string }) {
  const p = Math.max(0, Math.min(100, percent));
  return (
    <div style={{ height: 6, background: 'var(--bg-tertiary)', borderRadius: 999, overflow: 'hidden', marginTop: 10 }}>
      <div style={{ width: `${p}%`, height: '100%', background: color, borderRadius: 999, transition: 'width 0.4s ease' }} />
    </div>
  );
}

function buildRecentActivity(health: SystemHealth | null) {
  if (!health) return [];
  const items: { type: 'success' | 'info' | 'warning' | 'error'; text: string; time: string }[] = [];
  items.push({ type: 'success', text: `Uptime ${formatUptime(health.uptime_seconds)} · v${health.version}`, time: 'now' });
  for (const sub of health.subsystems) {
    if (sub.status !== 'healthy') {
      items.push({ type: sub.status === 'degraded' ? 'warning' : 'error', text: `${sub.name} is ${sub.status}`, time: 'now' });
    }
  }
  if (health.status === 'healthy' && items.length === 1) {
    items.push({ type: 'success', text: 'All subsystems healthy — run a query or queue job to see activity', time: 'now' });
  }
  return items;
}

const subsystemLinks: Record<string, string> = {
  database: '/database', cache: '/cache', queue: '/queue', scheduler: '/scheduler',
  search: '/search', blob: '/blob', storage: '/database', sql: '/database',
};

export default function DashboardPage() {
  const { data: health, loading } = useApi<SystemHealth>(() => api.getSystemHealth(), []);
  const recentActivity = buildRecentActivity(health as SystemHealth | null);

  const cpuPercent = health?.cpu.usage_percent ?? 0;
  const memPercent = health?.memory.total_bytes ? (health.memory.used_bytes / health.memory.total_bytes) * 100 : 0;
  const diskPercent = health?.disk.total_bytes ? (health.disk.used_bytes / health.disk.total_bytes) * 100 : 0;

  return (
    <div>
      <div className="page-header">
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 16 }}>
          <div>
            <h1>Overview</h1>
            <p>Health, storage and compute — one backend for prototypes. Pick a subsystem to start.</p>
          </div>
          {health && (
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexShrink: 0, paddingTop: 2 }}>
              <StatusBadge status={health.status} label={health.status} />
              <span style={{ fontSize: 12, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>v{health.version} · {formatUptime(health.uptime_seconds)}</span>
            </div>
          )}
        </div>
      </div>

      {/* Hero metrics */}
      <div className="grid grid-cols-3 mb-4">
        <div className="card" style={{ padding: 18 }}>
          <div className="card-title" style={{ marginBottom: 8 }}>CPU</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
            <span style={{ fontSize: 28, fontWeight: 700, letterSpacing: '-0.03em' }}>{cpuPercent.toFixed(1)}%</span>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{health?.cpu.cores ?? '-'} cores</span>
          </div>
          <ProgressBar percent={cpuPercent} color={cpuPercent > 80 ? 'var(--danger)' : cpuPercent > 60 ? 'var(--warning)' : 'var(--success)'} />
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 8 }}>{loading ? 'Loading…' : `${health?.cpu.cores ?? 0} cores`}</div>
        </div>
        <div className="card" style={{ padding: 18 }}>
          <div className="card-title" style={{ marginBottom: 8 }}>Memory</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8, flexWrap: 'wrap' }}>
            <span style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>{health ? formatBytes(health.memory.used_bytes) : '-'}</span>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>/ {health ? formatBytes(health.memory.total_bytes) : '—'}</span>
          </div>
          <ProgressBar percent={memPercent} color={memPercent > 85 ? 'var(--danger)' : memPercent > 70 ? 'var(--warning)' : 'var(--info)'} />
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 8 }}>{memPercent.toFixed(0)}% used</div>
        </div>
        <div className="card" style={{ padding: 18 }}>
          <div className="card-title" style={{ marginBottom: 8 }}>Storage</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
            <span style={{ fontSize: 22, fontWeight: 700 }}>{diskPercent.toFixed(0)}%</span>
            <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>{health ? formatBytes(health.disk.used_bytes) : '-'} used</span>
          </div>
          <ProgressBar percent={diskPercent} color={diskPercent > 80 ? 'var(--danger)' : 'var(--success)'} />
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 8, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={health?.disk.data_path}>{health?.disk.data_path || '—'}</div>
        </div>
      </div>

      {/* Compact network strip */}
      <div className="card" style={{ padding: '12px 16px', display: 'flex', gap: 24, flexWrap: 'wrap', alignItems: 'center', marginBottom: 16 }}>
        <span style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.05em', textTransform: 'uppercase', color: 'var(--text-muted)' }}>Network</span>
        <span style={{ fontSize: 12 }}><span style={{ color: 'var(--text-muted)' }}>in</span> <b>{health ? formatBytes(health.network.rx_bytes_per_sec) + '/s' : '-'}</b></span>
        <span style={{ fontSize: 12 }}><span style={{ color: 'var(--text-muted)' }}>out</span> <b>{health ? formatBytes(health.network.tx_bytes_per_sec) + '/s' : '-'}</b></span>
        <span style={{ fontSize: 12 }}><span style={{ color: 'var(--text-muted)' }}>active</span> <b>{health ? (health.network.connections_active || 0) : '-'}</b></span>
        <span style={{ fontSize: 12 }}><span style={{ color: 'var(--text-muted)' }}>retrans</span> <b>{health ? `${health.network.tcp_retransmit_percent}%` : '-'}</b></span>
        <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--text-muted)' }}>{health ? `${(health.network.rx_packets_per_sec + health.network.tx_packets_per_sec).toLocaleString()} pkt/s` : ''}</span>
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="card">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
            <div className="card-title" style={{ margin: 0 }}>Subsystems</div>
            <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{health ? `${health.subsystems.filter(s=>s.status==='healthy').length}/${health.subsystems.length} healthy` : ''}</span>
          </div>
          {health ? (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
              {health.subsystems.map((sub) => {
                const href = subsystemLinks[sub.name.toLowerCase()] || '/database';
                return (
                  <Link key={sub.name} to={href} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 12px', borderRadius: 8, border: '1px solid var(--border)', background: sub.status === 'healthy' ? 'var(--bg-primary)' : 'rgba(245,158,11,0.06)', textDecoration: 'none', transition: 'border-color 0.12s' }}>
                    <span style={{ fontSize: 13, fontWeight: 550, color: 'var(--text-primary)', textTransform: 'capitalize' }}>{sub.name}</span>
                    <StatusBadge status={sub.status} />
                  </Link>
                );
              })}
            </div>
          ) : (
            <div className="loading-spinner">Loading subsystems</div>
          )}
        </div>

        <div className="card">
          <div className="card-title">Quick start</div>
          <div style={{ display: 'grid', gap: 8, marginTop: 8 }}>
            <Link to="/database" className="btn" style={{ justifyContent: 'flex-start', textDecoration: 'none' }}>→ Create a table & run SQL</Link>
            <Link to="/queue" className="btn" style={{ justifyContent: 'flex-start', textDecoration: 'none' }}>→ Publish a queue message</Link>
            <Link to="/cache" className="btn" style={{ justifyContent: 'flex-start', textDecoration: 'none' }}>→ Try cache set / get</Link>
            <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 4 }}>SQL: <code style={{ background: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 4, fontFamily: 'var(--font-mono)' }}>SELECT/INSERT/UPDATE/DELETE</code> + <code style={{ background: 'var(--bg-tertiary)', padding: '2px 6px', borderRadius: 4 }}>JOIN/GROUP BY</code> — see Database → Query.</div>
          </div>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }} className="dashboard-bottom">
        <div className="card">
          <div className="card-title">Recent Activity</div>
          <div className="activity-feed" style={{ marginTop: 8 }}>
            {recentActivity.length === 0 ? (
              <div className="text-muted" style={{ textAlign: 'center', padding: 16 }}>
                {loading ? 'Loading...' : 'No activity yet — run a query or publish a message to see events here.'}
              </div>
            ) : (
              recentActivity.map((item, i) => (
                <div key={i} className="activity-item">
                  <div className={`activity-icon ${item.type}`}>
                    {item.type === 'success' ? <CheckCircleIcon size={14} /> : item.type === 'error' ? <XCircleIcon size={14} /> : item.type === 'warning' ? <AlertTriangleIcon size={14} /> : <InfoIcon size={14} />}
                  </div>
                  <div className="activity-text">{item.text}</div>
                  <div className="activity-time">{item.time}</div>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="card">
          <div className="card-title">Prototype tips</div>
          <div style={{ display: 'grid', gap: 10, marginTop: 8, fontSize: 13, lineHeight: 1.6, color: 'var(--text-secondary)' }}>
            <div><b style={{ color: 'var(--text-primary)' }}>One backend:</b> SQL, cache, queue, search, blobs & auth are all <code>novad</code> — no external Postgres/Redis.</div>
            <div><b style={{ color: 'var(--text-primary)' }}>Auth:</b> Bearer token or <code>X-Api-Key</code> (RBAC: admin/editor/viewer). Create keys in Auth.</div>
            <div><b style={{ color: 'var(--text-primary)' }}>Logs:</b> no REST history — use <Link to="/logs">Live Logs → Start Streaming</Link> (`/ws`).</div>
            <div><b style={{ color: 'var(--text-primary)' }}>Config:</b> editable keys hot-reload; others need <code>SIGHUP</code>/restart. See Config.</div>
          </div>
        </div>
      </div>
    </div>
  );
}
