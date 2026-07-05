import { useState } from 'react';
import { useAuth } from '../components/AuthContext';
import { useNavigate } from 'react-router-dom';

export default function LoginPage() {
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [localError, setLocalError] = useState<string | null>(null);
    const { login, loading, error } = useAuth();
    const navigate = useNavigate();

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLocalError(null);
        try {
            await login(username, password);
            navigate('/');
        } catch (err: unknown) {
            setLocalError(err instanceof Error ? err.message : 'Login failed');
        }
    };

    return (
        <div style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            minHeight: '100vh',
            background: 'var(--bg-primary)',
        }}>
            <div className="card" style={{ width: 360, padding: 32 }}>
                <div style={{ textAlign: 'center', marginBottom: 24 }}>
                    <h1 style={{ fontSize: 20, margin: 0 }}>Nova Runtime</h1>
                    <p style={{ color: 'var(--text-secondary)', fontSize: 13, marginTop: 8 }}>
                        Sign in to your dashboard
                    </p>
                </div>
                <form onSubmit={handleSubmit}>
                    <div className="form-group" style={{ marginBottom: 16 }}>
                        <label className="form-label">Username</label>
                        <input
                            className="form-input"
                            type="text"
                            value={username}
                            onChange={e => setUsername(e.target.value)}
                            placeholder="Enter your username"
                            autoFocus
                            required
                        />
                    </div>
                    <div className="form-group" style={{ marginBottom: 24 }}>
                        <label className="form-label">Password</label>
                        <input
                            className="form-input"
                            type="password"
                            value={password}
                            onChange={e => setPassword(e.target.value)}
                            placeholder="Enter your password"
                            required
                        />
                    </div>
                    {(localError || error) && (
                        <div className="callout error" style={{ marginBottom: 16 }}>
                            {localError || error}
                        </div>
                    )}
                    <button
                        type="submit"
                        className="btn btn-primary"
                        style={{ width: '100%' }}
                        disabled={loading}
                    >
                        {loading ? 'Signing in...' : 'Sign In'}
                    </button>
                </form>
                <div style={{ marginTop: 16, textAlign: 'center', fontSize: 12, color: 'var(--text-secondary)' }}>
                    Default: admin / admin123
                </div>
            </div>
        </div>
    );
}
