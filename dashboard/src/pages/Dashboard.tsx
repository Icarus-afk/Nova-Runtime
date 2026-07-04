import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { SystemHealth } from '../types';
import MetricCard from '../components/MetricCard';
import StatusBadge from '../components/StatusBadge';

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
  return `${d}d ${h}h ${m}m`;
}

function GaugeRing({ percent, label, color }: { percent: number; label: string; color: string }) {
  const angle = (percent / 100) * 360;
  return (
    <div className="gauge">
      <div className="gauge-ring">
        <div className="gauge-ring-bg" />
        <svg className="gauge-ring-fill" viewBox="0 0 100 100" style={{ transform: 'rotate(-90deg)' }}>
          <circle cx="50" cy="50" r="45" fill="none" stroke="var(--bg-tertiary)" strokeWidth="8" />
          <circle
            cx="50" cy="50" r="45" fill="none"
            stroke={color} strokeWidth="8" strokeLinecap="round"
            strokeDasharray={`${(angle / 360) * 2 * Math.PI * 45} ${2 * Math.PI * 45}`}
          />
        </svg>
        <span className="gauge-value">{Math.round(percent)}%</span>
      </div>
      <span className="gauge-label">{label}</span>
    </div>
  );
}

const recentActivity = [
  { type: 'success', text: 'Database query completed (12 rows in 3.2ms)', time: '2s ago' },
  { type: 'info', text: 'Cache eviction policy ran: 47 entries removed', time: '15s ago' },
  { type: 'warning', text: 'Queue "emails" depth at 85% capacity', time: '1m ago' },
  { type: 'success', text: 'Scheduled job "cleanup" completed successfully', time: '3m ago' },
  { type: 'error', text: 'Search query timeout on index "posts"', time: '5m ago' },
];

export default function DashboardPage() {
  const { data: health, loading } = useApi<SystemHealth>(() => api.getSystemHealth(), []);

  const cpuPercent = health?.cpu.usage_percent ?? 0;
  const memPercent = health ? (health.memory.used_bytes / health.memory.total_bytes) * 100 : 0;
  const diskPercent = health ? (health.disk.used_bytes / health.disk.total_bytes) * 100 : 0;

  return (
    <div>
      <div className="page-header">
        <h1>System Overview</h1>
        <p>Real-time health and performance metrics for the Nova Runtime</p>
      </div>

      {health && (
        <div className="flex items-center gap-3 mb-4">
          <StatusBadge status={health.status} label={health.status} />
          <span className="text-sm text-muted">
            v{health.version} · Uptime {formatUptime(health.uptime_seconds)}
          </span>
        </div>
      )}

      <div className="grid grid-cols-4 mb-4">
        <MetricCard
          title="CPU Usage"
          value={cpuPercent.toFixed(1)} unit="%"
          color="accent" loading={loading}
        />
        <MetricCard
          title="Memory"
          value={health ? formatBytes(health.memory.used_bytes) : '-'}
          unit={`/ ${health ? formatBytes(health.memory.total_bytes) : ''}`}
          color="info" loading={loading}
        />
        <MetricCard
          title="Disk Usage"
          value={diskPercent.toFixed(1)} unit="%"
          color={diskPercent > 80 ? 'danger' : 'success'}
          loading={loading}
        />
        <MetricCard
          title="Active Connections"
          value={health?.network.connections_active ?? '-'}
          color="success" loading={loading}
        />
      </div>

      <div className="grid grid-cols-4 mb-4">
        <div className="card">
          <div className="card-title">Network In</div>
          <div className="card-value text-sm">
            {health ? formatBytes(health.network.rx_bytes_per_sec) + '/s' : '-'}
          </div>
        </div>
        <div className="card">
          <div className="card-title">Network Out</div>
          <div className="card-value text-sm">
            {health ? formatBytes(health.network.tx_bytes_per_sec) + '/s' : '-'}
          </div>
        </div>
        <div className="card">
          <div className="card-title">Request Rate</div>
          <div className="card-value text-sm">
            {health ? `${(health.network.rx_packets_per_sec + health.network.tx_packets_per_sec).toLocaleString()} pkt/s` : '-'}
          </div>
        </div>
        <div className="card">
          <div className="card-title">TCP Retransmit</div>
          <div className="card-value text-sm">{health ? `${health.network.tcp_retransmit_percent}%` : '-'}</div>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="card">
          <div className="card-title">Resource Usage</div>
          <div className="grid grid-cols-3" style={{ marginTop: 8 }}>
            <GaugeRing percent={cpuPercent} label="CPU" color="var(--accent)" />
            <GaugeRing percent={memPercent} label="Memory" color="var(--info)" />
            <GaugeRing percent={diskPercent} label="Disk" color="var(--success)" />
          </div>
        </div>

        <div className="card">
          <div className="card-title">Subsystem Status</div>
          <div style={{ marginTop: 8 }}>
            {health ? (
              <table className="data-table">
                <tbody>
                  {health.subsystems.map((sub) => (
                    <tr key={sub.name}>
                      <td style={{ padding: '6px 0', borderBottom: '1px solid var(--border)' }}>{sub.name}</td>
                      <td style={{ padding: '6px 0', borderBottom: '1px solid var(--border)', textAlign: 'right' }}>
                        <StatusBadge status={sub.status} />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div className="loading-spinner">Loading subsystems</div>
            )}
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-title">Recent Activity</div>
        <div className="activity-feed" style={{ marginTop: 8 }}>
          {recentActivity.map((item, i) => (
            <div key={i} className="activity-item">
              <div className={`activity-icon ${item.type}`}>
                {item.type === 'success' ? '✓' : item.type === 'error' ? '✗' : item.type === 'warning' ? '!' : 'i'}
              </div>
              <div className="activity-text">{item.text}</div>
              <div className="activity-time">{item.time}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
