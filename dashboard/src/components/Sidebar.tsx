import { NavLink } from 'react-router-dom';
import type { NavItem } from '../types';
import { OverviewIcon, DatabaseIcon, CacheIcon, QueueIcon, SchedulerIcon, SearchIcon, BlobIcon, AuthIcon, ConfigIcon, LogsIcon } from './Icons';

const navGroups: { title: string; items: NavItem[] }[] = [
  {
    title: 'Overview',
    items: [{ id: 'dashboard', label: 'Overview', path: '/', icon: <OverviewIcon /> }],
  },
  {
    title: 'Storage',
    items: [
      { id: 'database', label: 'Database', path: '/database', icon: <DatabaseIcon /> },
      { id: 'cache', label: 'Cache', path: '/cache', icon: <CacheIcon /> },
      { id: 'blob', label: 'Objects', path: '/blob', icon: <BlobIcon /> },
      { id: 'search', label: 'Search', path: '/search', icon: <SearchIcon /> },
    ],
  },
  {
    title: 'Compute',
    items: [
      { id: 'queue', label: 'Queues', path: '/queue', icon: <QueueIcon /> },
      { id: 'scheduler', label: 'Scheduler', path: '/scheduler', icon: <SchedulerIcon /> },
    ],
  },
  {
    title: 'Manage',
    items: [
      { id: 'auth', label: 'Users & Keys', path: '/auth', icon: <AuthIcon /> },
      { id: 'config', label: 'Config', path: '/config', icon: <ConfigIcon /> },
      { id: 'logs', label: 'Live Logs', path: '/logs', icon: <LogsIcon /> },
    ],
  },
];

export default function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="logo">◈</div>
        <div style={{ display: 'flex', flexDirection: 'column', lineHeight: 1.1 }}>
          <span style={{ letterSpacing: '-0.02em', fontSize: 13 }}>Nova <span style={{ color: 'var(--text-muted)', fontWeight: 400, fontSize: 11 }}>Runtime</span></span>
          <span style={{ fontSize: 10, color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>localhost:8642 · v0.1</span>
        </div>
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
        <a href="/graphql" target="_blank" rel="noreferrer" style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: 'var(--text-secondary)', textDecoration: 'none' }}>
          <span style={{ width: 6, height: 6, borderRadius: 999, background: 'var(--success)' }} />
          GraphQL Playground →
        </a>
        <div style={{ marginTop: 6, fontSize: 10, color: 'var(--text-muted)' }}>Docs: <a href="https://github.com/Icarus-afk/Nova-Runtime" target="_blank" rel="noreferrer" style={{ color: 'var(--text-muted)' }}>README</a> · <code style={{ background: 'var(--bg-tertiary)', padding: '1px 4px', borderRadius: 3 }}>make dev</code></div>
      </div>
    </aside>
  );
}
