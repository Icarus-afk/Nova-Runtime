import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import ErrorBoundary from './components/ErrorBoundary';
import { AuthProvider, useAuth } from './components/AuthContext';
import LoginPage from './pages/LoginPage';
import DashboardPage from './pages/Dashboard';
import DatabasePage from './pages/Database';
import CachePage from './pages/Cache';
import QueuePage from './pages/Queue';
import SchedulerPage from './pages/Scheduler';
import SearchPage from './pages/Search';
import BlobPage from './pages/Blob';
import AuthPage from './pages/Auth';
import ConfigPage from './pages/Config';
import LogsPage from './pages/Logs';
import type { ReactNode } from 'react';

function ProtectedRoute({ children }: { children: ReactNode }) {
    const { isAuthenticated } = useAuth();
    if (!isAuthenticated) {
        return <Navigate to="/login" replace />;
    }
    return <>{children}</>;
}

export default function App() {
    return (
        <BrowserRouter>
            <AuthProvider>
                <Routes>
                    <Route path="/login" element={<LoginPage />} />
                    <Route element={<ProtectedRoute><Layout /></ProtectedRoute>}>
                        <Route path="/" element={<ErrorBoundary pageName="Dashboard"><DashboardPage /></ErrorBoundary>} />
                        <Route path="/database" element={<ErrorBoundary pageName="Database"><DatabasePage /></ErrorBoundary>} />
                        <Route path="/cache" element={<ErrorBoundary pageName="Cache"><CachePage /></ErrorBoundary>} />
                        <Route path="/queue" element={<ErrorBoundary pageName="Queue"><QueuePage /></ErrorBoundary>} />
                        <Route path="/scheduler" element={<ErrorBoundary pageName="Scheduler"><SchedulerPage /></ErrorBoundary>} />
                        <Route path="/search" element={<ErrorBoundary pageName="Search"><SearchPage /></ErrorBoundary>} />
                        <Route path="/blob" element={<ErrorBoundary pageName="Blob Storage"><BlobPage /></ErrorBoundary>} />
                        <Route path="/auth" element={<ErrorBoundary pageName="Users & API Keys"><AuthPage /></ErrorBoundary>} />
                        <Route path="/config" element={<ErrorBoundary pageName="Configuration"><ConfigPage /></ErrorBoundary>} />
                        <Route path="/logs" element={<ErrorBoundary pageName="Logs"><LogsPage /></ErrorBoundary>} />
                    </Route>
                </Routes>
            </AuthProvider>
        </BrowserRouter>
    );
}
