import { Outlet, useLocation, Link } from 'react-router-dom';
import Sidebar from './Sidebar';
import Header from './Header';

function Breadcrumbs() {
  const location = useLocation();
  const parts = location.pathname.split('/').filter(Boolean);
  if (parts.length === 0) return <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>Overview</span>;
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: 'var(--text-muted)' }}>
      <Link to="/" style={{ color: 'var(--text-muted)' }}>Home</Link>
      <span style={{ opacity: 0.4 }}>/</span>
      {parts.map((part, i) => {
        const path = '/' + parts.slice(0, i + 1).join('/');
        const isLast = i === parts.length - 1;
        const label = part.charAt(0).toUpperCase() + part.slice(1);
        return (
          <span key={path} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            {isLast ? (
              <span style={{ color: 'var(--text-primary)', fontWeight: 500 }}>{label}</span>
            ) : (
              <>
                <Link to={path} style={{ color: 'var(--text-muted)' }}>{label}</Link>
                <span style={{ opacity: 0.4 }}>/</span>
              </>
            )}
          </span>
        );
      })}
    </div>
  );
}

export default function Layout() {
  return (
    <div className="app-layout">
      <Sidebar />
      <div className="main-area">
        <Header />
        <div style={{ padding: '12px 24px 0', borderBottom: '1px solid var(--border)', background: 'var(--bg-primary)' }}>
          <Breadcrumbs />
        </div>
        <main className="content">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
