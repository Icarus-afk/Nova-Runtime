import { NavLink } from 'react-router-dom';
import type { NavItem } from '../types';

const navItems: NavItem[] = [
  { id: 'dashboard', label: 'Overview', path: '/', icon: '◉' },
  { id: 'database', label: 'Database', path: '/database', icon: '🗄' },
  { id: 'cache', label: 'Cache', path: '/cache', icon: '⚡' },
  { id: 'queue', label: 'Queue', path: '/queue', icon: '📨' },
  { id: 'scheduler', label: 'Scheduler', path: '/scheduler', icon: '⏰' },
  { id: 'search', label: 'Search', path: '/search', icon: '🔍' },
  { id: 'blob', label: 'Blob Storage', path: '/blob', icon: '📦' },
  { id: 'auth', label: 'Users & API Keys', path: '/auth', icon: '🔐' },
  { id: 'config', label: 'Configuration', path: '/config', icon: '⚙' },
  { id: 'logs', label: 'Logs', path: '/logs', icon: '📋' },
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
