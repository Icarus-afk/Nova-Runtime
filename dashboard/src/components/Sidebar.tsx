import { NavLink } from 'react-router-dom';
import type { NavItem } from '../types';
import { OverviewIcon, DatabaseIcon, CacheIcon, QueueIcon, SchedulerIcon, SearchIcon, BlobIcon, AuthIcon, ConfigIcon, LogsIcon } from './Icons';

const navGroups: { title: string; items: NavItem[] }[] = [
  {
    title: 'Platform',
    items: [{ id: 'dashboard', label: 'Overview', path: '/', icon: <OverviewIcon /> }],
  },
  {
    title: 'Data',
    items: [
      { id: 'database', label: 'Database', path: '/database', icon: <DatabaseIcon /> },
      { id: 'cache', label: 'Cache', path: '/cache', icon: <CacheIcon /> },
      { id: 'blob', label: 'Blob Storage', path: '/blob', icon: <BlobIcon /> },
      { id: 'search', label: 'Search', path: '/search', icon: <SearchIcon /> },
    ],
  },
  {
    title: 'Work',
    items: [
      { id: 'queue', label: 'Queue', path: '/queue', icon: <QueueIcon /> },
      { id: 'scheduler', label: 'Scheduler', path: '/scheduler', icon: <SchedulerIcon /> },
    ],
  },
  {
    title: 'Access',
    items: [
      { id: 'auth', label: 'Auth', path: '/auth', icon: <AuthIcon /> },
      { id: 'config', label: 'Config', path: '/config', icon: <ConfigIcon /> },
      { id: 'logs', label: 'Logs', path: '/logs', icon: <LogsIcon /> },
    ],
  },
];

export default function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="logo">◈</div>
        <span style={{ letterSpacing: '-0.02em' }}>Nova</span>
        <span style={{ color: 'var(--text-muted)', fontWeight: 400, marginLeft: 2, fontSize: 11 }}>Runtime</span>
        <span style={{ marginLeft: 'auto', fontSize: 9, color: 'var(--text-muted)', border: '1px solid var(--border)', padding: '1px 4px', borderRadius: 4, fontFamily: 'var(--font-mono)' }}>v0.1</span>
      </div>
      <nav className="sidebar-nav">
        {navGroups.map((group) => (
          <div key={group.title} style={{ marginBottom: 16 }}>
            <div style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.06em', textTransform: 'uppercase', color: 'var(--text-muted)', padding: '0 12px 6px' }}>
              {group.title}
            </div>
            {group.items.map((item) => (
              <NavLink
                key={item.id}
                to={item.path}
                end={item.path === '/'}
                className={({ isActive }) => isActive ? 'active' : ''}
              >
                <span className="nav-icon">{item.icon}</span>
                <span>{item.label}</span>
              </NavLink>
            ))}
          </div>
        ))}
      </nav>
      <div className="sidebar-footer">
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11 }}>
          <span style={{ width: 6, height: 6, borderRadius: 999, background: 'var(--success)' }} />
          <span>All systems operational</span>
        </div>
        <div style={{ marginTop: 4, fontSize: 10, color: 'var(--text-muted)' }}>Novad • 8642 • GraphQL /graphql</div>
      </div>
    </aside>
  );
}
