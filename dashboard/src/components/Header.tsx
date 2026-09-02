import { useState, useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from './AuthContext';

const COMMANDS = [
  { label: 'Overview', path: '/', hint: 'Health & subsystems' },
  { label: 'Database', path: '/database', hint: 'SQL & collections' },
  { label: 'Cache', path: '/cache', hint: 'Keys & stats' },
  { label: 'Objects', path: '/blob', hint: 'Blob storage' },
  { label: 'Search', path: '/search', hint: 'Indexes' },
  { label: 'Queues', path: '/queue', hint: 'Messages' },
  { label: 'Scheduler', path: '/scheduler', hint: 'Jobs' },
  { label: 'Users & Keys', path: '/auth', hint: 'Auth' },
  { label: 'Config', path: '/config', hint: 'Runtime config' },
  { label: 'Live Logs', path: '/logs', hint: 'WebSocket stream' },
];

export default function Header() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const [connectionStatus, setConnectionStatus] = useState<'connected' | 'disconnected' | 'reconnecting'>('disconnected');
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState('');

  useEffect(() => {
    const checkConnection = async () => {
      try {
        const res = await fetch('/health', { signal: AbortSignal.timeout(3000) });
        if (res.ok) {
          setConnectionStatus('connected');
        } else {
          setConnectionStatus('disconnected');
        }
      } catch {
        setConnectionStatus('disconnected');
      }
    };
    checkConnection();
    const interval = setInterval(checkConnection, 15000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
      if (e.key === 'Escape') setPaletteOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const filtered = useMemo(() => {
    const q = paletteQuery.trim().toLowerCase();
    if (!q) return COMMANDS;
    return COMMANDS.filter((c) => c.label.toLowerCase().includes(q) || c.hint.toLowerCase().includes(q));
  }, [paletteQuery]);

  return (
    <header className="header">
      <div className="header-left">
        <button
          onClick={() => setPaletteOpen(true)}
          style={{ display: 'flex', alignItems: 'center', gap: 10, fontSize: 12, color: 'var(--text-muted)', background: 'var(--bg-tertiary)', border: '1px solid var(--border)', borderRadius: 6, padding: '5px 10px', cursor: 'pointer' }}
          title="Open command palette (⌘K)"
        >
          <span title={connectionStatus} style={{ width: 8, height: 8, borderRadius: 999, background: connectionStatus === 'connected' ? 'var(--success)' : connectionStatus === 'reconnecting' ? 'var(--warning)' : 'var(--danger)', boxShadow: connectionStatus === 'connected' ? '0 0 8px rgba(16,185,129,0.5)' : undefined, flexShrink: 0 }} />
          <span style={{ textTransform: 'capitalize', fontWeight: 600, color: connectionStatus === 'connected' ? 'var(--text-secondary)' : 'var(--danger)' }}>{connectionStatus === 'connected' ? 'Connected' : connectionStatus === 'reconnecting' ? 'Reconnecting' : 'Offline'}</span>
          <span style={{ opacity: 0.2 }}>·</span>
          <span style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>8642</span>
          <span style={{ marginLeft: 6, background: 'var(--bg-primary)', border: '1px solid var(--border)', borderBottomWidth: 1.5, padding: '1px 5px', borderRadius: 4, fontSize: 10, fontFamily: 'var(--font-mono)' }}>⌘K</span>
        </button>
      </div>
      <div className="header-right" style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <div style={{ fontSize: 11, color: 'var(--text-muted)', display: 'flex', alignItems: 'center', gap: 6 }}>
          <span>{new Date().toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}</span>
          <span style={{ opacity: 0.3 }}>•</span>
          <span>v0.1.0</span>
        </div>
        {user && (
          <div style={{ position: 'relative' }}>
            <button
              onClick={() => setShowUserMenu(!showUserMenu)}
              style={{
                display: 'flex', alignItems: 'center', gap: 8,
                background: 'var(--bg-tertiary)', border: '1px solid var(--border)',
                borderRadius: 6, padding: '4px 8px', cursor: 'pointer',
                fontSize: 12, color: 'var(--text-primary)'
              }}
            >
              <span style={{ width: 20, height: 20, borderRadius: 999, background: 'var(--text-primary)', color: 'var(--bg-primary)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, fontWeight: 700 }}>
                {user.username[0].toUpperCase()}
              </span>
              <span>{user.username}</span>
              <span style={{ color: 'var(--text-muted)', fontSize: 10 }}>▾</span>
            </button>
            {showUserMenu && (
              <div style={{ position: 'absolute', right: 0, top: 'calc(100% + 8px)', background: 'var(--bg-secondary)', border: '1px solid var(--border)', borderRadius: 6, padding: 4, minWidth: 160, boxShadow: 'var(--shadow-lg)', zIndex: 50 }}>
                <div style={{ padding: '8px 10px', borderBottom: '1px solid var(--border)', marginBottom: 4 }}>
                  <div style={{ fontSize: 12, fontWeight: 600 }}>{user.username}</div>
                  <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>{(user as any).email || `${user.username}@nova.local`}</div>
                </div>
                <button
                  onClick={() => { setShowUserMenu(false); logout(); }}
                  style={{ width: '100%', textAlign: 'left', padding: '6px 10px', background: 'none', border: 'none', color: 'var(--danger)', fontSize: 12, cursor: 'pointer', borderRadius: 4 }}
                  onMouseEnter={e => (e.currentTarget.style.background = 'var(--bg-tertiary)')}
                  onMouseLeave={e => (e.currentTarget.style.background = 'none')}
                >
                  Sign out
                </button>
              </div>
            )}
          </div>
        )}
      </div>
      {paletteOpen && (
        <div className="modal-overlay" onClick={() => setPaletteOpen(false)} style={{ zIndex: 100 }}>
          <div className="modal modal-md" onClick={(e) => e.stopPropagation()} style={{ overflow: 'hidden' }}>
            <div style={{ padding: 12, borderBottom: '1px solid var(--border)' }}>
              <input
                autoFocus
                className="form-input"
                placeholder="Go to… (Database, Cache, Queues…)"
                value={paletteQuery}
                onChange={(e) => setPaletteQuery(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter' && filtered[0]) { navigate(filtered[0].path); setPaletteOpen(false); } }}
              />
            </div>
            <div style={{ maxHeight: 320, overflowY: 'auto', padding: 8 }}>
              {filtered.map((c) => (
                <button
                  key={c.path}
                  onClick={() => { navigate(c.path); setPaletteOpen(false); setPaletteQuery(''); }}
                  style={{ width: '100%', display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 12px', borderRadius: 6, border: 'none', background: 'transparent', color: 'var(--text-primary)', cursor: 'pointer', textAlign: 'left' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--bg-tertiary)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                >
                  <span style={{ fontWeight: 550 }}>{c.label}</span>
                  <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{c.hint}</span>
                </button>
              ))}
              {filtered.length === 0 && <div style={{ padding: 16, textAlign: 'center', color: 'var(--text-muted)', fontSize: 12 }}>No matches</div>}
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
