import { NavLink } from 'react-router-dom';
import type { NavItem } from '../types';
import { OverviewIcon, DatabaseIcon, CacheIcon, QueueIcon, SchedulerIcon, SearchIcon, BlobIcon, AuthIcon, ConfigIcon, LogsIcon } from './Icons';

const navItems: NavItem[] = [
  { id: 'dashboard', label: 'Overview', path: '/', icon: <OverviewIcon /> },
  { id: 'database', label: 'Database', path: '/database', icon: <DatabaseIcon /> },
  { id: 'cache', label: 'Cache', path: '/cache', icon: <CacheIcon /> },
  { id: 'queue', label: 'Queue', path: '/queue', icon: <QueueIcon /> },
  { id: 'scheduler', label: 'Scheduler', path: '/scheduler', icon: <SchedulerIcon /> },
  { id: 'search', label: 'Search', path: '/search', icon: <SearchIcon /> },
  { id: 'blob', label: 'Blob Storage', path: '/blob', icon: <BlobIcon /> },
  { id: 'auth', label: 'Users & API Keys', path: '/auth', icon: <AuthIcon /> },
  { id: 'config', label: 'Configuration', path: '/config', icon: <ConfigIcon /> },
  { id: 'logs', label: 'Logs', path: '/logs', icon: <LogsIcon /> },
];

export default function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="logo">N</div>
        <span>Nova Runtime</span>
      </div>
      <nav className="sidebar-nav">
        {navItems.map((item) => (
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
      </nav>
      <div className="sidebar-footer">v0.1.0 · Dashboard</div>
    </aside>
  );
}
