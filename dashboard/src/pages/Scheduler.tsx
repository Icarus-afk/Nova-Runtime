import { useState } from 'react';
import { useApi, useApiLazy } from '../hooks/useApi';
import { api } from '../api/client';
import type { JobInfo, JobExecution } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import StatusBadge from '../components/StatusBadge';

const statusMap: Record<string, 'healthy' | 'degraded' | 'critical'> = {
  active: 'healthy',
  paused: 'degraded',
  disabled: 'critical',
  completed: 'healthy',
  failed: 'critical',
};

export default function SchedulerPage() {
  const [selectedJob, setSelectedJob] = useState<string | null>(null);
  const [execPage, setExecPage] = useState(1);
  const [showCreate, setShowCreate] = useState(false);
  const [createForm, setCreateForm] = useState({ name: '', type: 'cron', schedule: '0 * * * *', handler: '', max_retries: 3 });
  const [triggerStatus, setTriggerStatus] = useState<string | null>(null);

  const { data: jobs, loading: jobsLoading, refetch: refetchJobs } = useApi<JobInfo[]>(() => api.getJobs(), []);
  const { data: execData, loading: execLoading, refetch: refetchExec } = useApi(
    () => selectedJob ? api.getJobExecutions(selectedJob, execPage) : Promise.resolve(null),
    [selectedJob, execPage]
  );

  const { execute: execCreate, loading: createLoading } = useApiLazy<JobInfo>();

  const selectedInfo = jobs?.find(j => j.id === selectedJob);

  const handleTrigger = async () => {
    if (!selectedJob) return;
    setTriggerStatus(null);
    try {
      await api.triggerJob(selectedJob);
      setTriggerStatus('Job triggered successfully');
      refetchExec();
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Trigger failed');
    }
  };

  const handleCreate = async () => {
    await execCreate(() => api.createJob(createForm));
    setShowCreate(false);
    refetchJobs();
  };

  const jobColumns = [
    { key: 'name', header: 'Job Name' },
    { key: 'type', header: 'Type', width: '80px' },
    { key: 'schedule', header: 'Schedule', width: '120px', render: (v: unknown) => (v as string) || '-' },
    { key: 'status', header: 'Status', width: '90px', render: (v: unknown) => <StatusBadge status={statusMap[v as string] || 'degraded'} label={v as string} /> },
    { key: 'last_run_at', header: 'Last Run', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    { key: 'next_run_at', header: 'Next Run', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
  ];

  const execColumns = [
    { key: 'id', header: 'Run ID', width: '160px' },
    { key: 'status', header: 'Status', width: '100px', render: (v: unknown) => {
      const s = v as string;
      return <StatusBadge status={s === 'success' ? 'healthy' : s === 'failed' || s === 'timeout' ? 'critical' : 'degraded'} label={s} />;
    }},
    { key: 'started_at', header: 'Started', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    { key: 'duration_ms', header: 'Duration', width: '80px', render: (v: unknown) => v ? `${(v as number).toFixed(1)}ms` : '-' },
    { key: 'trigger', header: 'Trigger', width: '80px' },
    { key: 'result', header: 'Result', render: (v: unknown) => {
      const s = String(v ?? '');
      return s.length > 50 ? s.slice(0, 50) + '...' : s || '-';
    }},
  ];

  const activeCount = jobs?.filter(j => j.status === 'active').length ?? 0;
  const overdueCount = jobs?.filter(j => j.status === 'failed').length ?? 0;

  return (
    <div>
      <div className="page-header">
        <h1>Scheduler</h1>
        <p>Manage scheduled jobs and view execution history</p>
      </div>

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Jobs" value={jobs?.length ?? '-'} color="accent" loading={jobsLoading} />
        <MetricCard title="Active" value={activeCount} color="success" loading={jobsLoading} />
        <MetricCard title="Failed" value={overdueCount} color={overdueCount > 0 ? 'danger' : 'success'} loading={jobsLoading} />
        <MetricCard title="Total Executions" value={execData?.pagination?.total?.toLocaleString() ?? '-'} color="info" loading={execLoading && !!selectedJob} />
      </div>

      <div className="card mb-4">
        <div className="flex items-center justify-between mb-4">
          <div className="card-title" style={{ margin: 0 }}>Jobs</div>
          <button className="btn btn-sm btn-primary" onClick={() => setShowCreate(!showCreate)}>
            {showCreate ? 'Cancel' : 'Create Job'}
          </button>
        </div>

        {showCreate && (
          <div className="card" style={{ marginBottom: 16, background: 'var(--bg-primary)' }}>
            <div className="grid grid-cols-2 gap-3">
              <div className="form-group">
                <label>Name</label>
                <input className="form-input" value={createForm.name} onChange={(e) => setCreateForm({ ...createForm, name: e.target.value })} />
              </div>
              <div className="form-group">
                <label>Type</label>
                <select className="form-select" value={createForm.type} onChange={(e) => setCreateForm({ ...createForm, type: e.target.value })}>
                  <option value="cron">Cron</option>
                  <option value="once">Once</option>
                  <option value="interval">Interval</option>
                </select>
              </div>
              <div className="form-group">
                <label>Schedule (cron)</label>
                <input className="form-input" value={createForm.schedule} onChange={(e) => setCreateForm({ ...createForm, schedule: e.target.value })} />
              </div>
              <div className="form-group">
                <label>Handler</label>
                <input className="form-input" value={createForm.handler} onChange={(e) => setCreateForm({ ...createForm, handler: e.target.value })} />
              </div>
            </div>
            <button className="btn btn-primary" onClick={handleCreate} disabled={createLoading}>
              {createLoading ? 'Creating...' : 'Create Job'}
            </button>
          </div>
        )}

        <DataTable
          columns={jobColumns}
          data={(jobs || []) as unknown as Record<string, unknown>[]}
          loading={jobsLoading}
          onRowClick={(row) => { setSelectedJob(row.id as string); setExecPage(1); }}
          emptyMessage="No jobs configured"
        />
      </div>

      {selectedJob && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Execution History: {selectedInfo?.name || selectedJob}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  Max retries: {selectedInfo.max_retries} · Retry delay: {selectedInfo.retry_delay_seconds}s · Timeout: {selectedInfo.timeout_seconds}s
                  · Concurrency: {selectedInfo.concurrency_policy} · Tags: {selectedInfo.tags.join(', ') || 'none'}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm btn-primary" onClick={handleTrigger}>Trigger Now</button>
            </div>
          </div>

          {triggerStatus && (
            <div className={`callout ${triggerStatus.includes('failed') || triggerStatus.includes('Error') ? 'error' : 'info'}`}>
              {triggerStatus}
              <button className="btn btn-sm" style={{ marginLeft: 12 }} onClick={() => setTriggerStatus(null)}>Dismiss</button>
            </div>
          )}

          <DataTable
            columns={execColumns}
            data={(execData?.data || []) as unknown as Record<string, unknown>[]}
            loading={execLoading}
            pagination={execData?.pagination}
            onPageChange={setExecPage}
            emptyMessage="No execution history"
          />
        </div>
      )}
    </div>
  );
}
