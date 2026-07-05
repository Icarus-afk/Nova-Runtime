import { useState, useEffect } from 'react';
import { useAuth } from './AuthContext';

export default function Header() {
  const { user, logout } = useAuth();
  const [connectionStatus, setConnectionStatus] = useState<'connected' | 'disconnected' | 'reconnecting'>('disconnected');

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
        <span style={{ fontSize: 13, color: 'var(--text-secondary)' }}>Nova Runtime Dashboard</span>
      </div>
      <div className="header-right" style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        {user && (
          <span style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
            {user.username}
          </span>
        )}
        {user && (
          <button className="btn btn-sm" onClick={logout} style={{ fontSize: 12 }}>
            Logout
          </button>
        )}
        <div className="connection-status">
          <span className={`connection-dot ${connectionStatus}`} />
          <span>{connectionStatus}</span>
        </div>
      </div>
    </header>
  );
}
