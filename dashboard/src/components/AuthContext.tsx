import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { api, setToken, getToken } from '../api/client';

interface AuthUser {
    id: string;
    username: string;
    email: string;
    role: string;
}

interface AuthContextType {
    user: AuthUser | null;
    token: string | null;
    loading: boolean;
    error: string | null;
    login: (username: string, password: string) => Promise<void>;
    logout: () => void;
    isAuthenticated: boolean;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
    const [user, setUser] = useState<AuthUser | null>(null);
    const [token, setTokenState] = useState<string | null>(() => localStorage.getItem('nova_token'));
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    // On mount, restore token
    useEffect(() => {
        const savedToken = localStorage.getItem('nova_token');
        if (savedToken) {
            setToken(savedToken);
            setTokenState(savedToken);
        }
    }, []);

    const login = useCallback(async (username: string, password: string) => {
        setLoading(true);
        setError(null);
        try {
            const result = await api.login(username, password);
            setUser(result.user);
            setTokenState(result.token);
        } catch (err: unknown) {
            const message = err instanceof Error ? err.message : 'Login failed';
            setError(message);
            throw err;
        } finally {
            setLoading(false);
        }
    }, []);

    const logout = useCallback(() => {
        setToken(null);
        setTokenState(null);
        setUser(null);
        localStorage.removeItem('nova_token');
    }, []);

    return (
        <AuthContext.Provider value={{
            user,
            token,
            loading,
            error,
            login,
            logout,
            isAuthenticated: !!token,
        }}>
            {children}
        </AuthContext.Provider>
    );
}

export function useAuth(): AuthContextType {
    const ctx = useContext(AuthContext);
    if (!ctx) {
        throw new Error('useAuth must be used within AuthProvider');
    }
    return ctx;
}
