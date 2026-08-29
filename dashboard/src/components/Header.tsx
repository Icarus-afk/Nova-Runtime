import { useState, useEffect } from 'react';
import { useAuth } from './AuthContext';

export default function Header() {
  const { user, logout } = useAuth();
  const [connectionStatus, setConnectionStatus] = useState<'connected' | 'disconnected' | 'reconnecting'>('disconnected');
  const [showUserMenu, setShowUserMenu] = useState(false);

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

  return (
    <header className="header">
      <div className="header-left">
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: 'var(--text-muted)' }}>
          <span style={{ width: 6, height: 6, borderRadius: 999, background: connectionStatus === 'connected' ? 'var(--success)' : connectionStatus === 'reconnecting' ? 'var(--warning)' : 'var(--danger)' }} />
          <span style={{ textTransform: 'capitalize', fontWeight: 500 }}>{connectionStatus}</span>
          <span style={{ opacity: 0.3 }}>•</span>
          <span>Press <span style={{ background: 'var(--bg-tertiary)', border: '1px solid var(--border)', borderBottomWidth: 2, padding: '1px 4px', borderRadius: 4, fontSize: 10, fontFamily: 'var(--font-mono)' }}>⌘K</span> for commands</span>
        </div>
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
    </header>
  );
}
