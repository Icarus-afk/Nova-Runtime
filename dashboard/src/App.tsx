import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
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

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/database" element={<DatabasePage />} />
          <Route path="/cache" element={<CachePage />} />
          <Route path="/queue" element={<QueuePage />} />
          <Route path="/scheduler" element={<SchedulerPage />} />
          <Route path="/search" element={<SearchPage />} />
          <Route path="/blob" element={<BlobPage />} />
          <Route path="/auth" element={<AuthPage />} />
          <Route path="/config" element={<ConfigPage />} />
          <Route path="/logs" element={<LogsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
