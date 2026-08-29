import { useState } from 'react';
import { useApi, useApiLazy } from '../hooks/useApi';
import { api } from '../api/client';
import type { JobInfo, JobExecution } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import StatusBadge from '../components/StatusBadge';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

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
  const [editingJob, setEditingJob] = useState<JobInfo | null>(null);
  const [createForm, setCreateForm] = useState({ name: '', type: 'cron', schedule: '*/5 * * * *', handler: 'echo hello', payload: '{}', max_retries: 3 });
  const [formError, setFormError] = useState<string | null>(null);
  const [triggerStatus, setTriggerStatus] = useState<string | null>(null);
  const [deleteJobId, setDeleteJobId] = useState<string | null>(null);

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

  const handlePauseResume = async (job: JobInfo) => {
    try {
      if (job.status === 'paused') {
        await api.resumeJob(job.id);
      } else {
        await api.pauseJob(job.id);
      }
      refetchJobs();
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Failed');
    }
  };

  const handleDelete = async () => {
    if (!deleteJobId) return;
    try {
      await api.deleteJob(deleteJobId);
      if (selectedJob === deleteJobId) setSelectedJob(null);
      setDeleteJobId(null);
      refetchJobs();
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Delete failed');
    }
  };

  const handleCreateOrUpdate = async () => {
    if (!createForm.name.trim()) {
      setFormError('Name required');
      return;
    }
    setFormError(null);
    try {
      let payload: Record<string, unknown> = {};
      try {
        payload = JSON.parse(createForm.payload || '{}');
      } catch {
        setFormError('Payload must be valid JSON');
        return;
      }
      if (editingJob) {
        await api.updateJob(editingJob.id, { name: createForm.name, schedule: createForm.schedule, enabled: true } as any);
      } else {
        await execCreate(() => api.createJob({ ...createForm, payload }));
      }
      setShowCreate(false);
      setEditingJob(null);
      setCreateForm({ name: '', type: 'cron', schedule: '*/5 * * * *', handler: 'echo hello', payload: '{}', max_retries: 3 });
      refetchJobs();
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : 'Save failed');
    }
  };

  const openEdit = (job: JobInfo) => {
    setEditingJob(job);
    setCreateForm({ name: job.name, type: job.type, schedule: job.schedule || '*/5 * * * *', handler: job.handler, payload: JSON.stringify(job.payload || {}, null, 2), max_retries: job.max_retries });
    setShowCreate(true);
    setFormError(null);
  };

  const openCreate = () => {
    setEditingJob(null);
    setCreateForm({ name: '', type: 'cron', schedule: '*/5 * * * *', handler: 'echo hello', payload: '{}', max_retries: 3 });
    setFormError(null);
    setShowCreate(true);
  };

  const jobColumns: any[] = [
    { key: 'name', header: 'Job Name' },
    { key: 'type', header: 'Type', width: '80px' },
    { key: 'schedule', header: 'Schedule', width: '120px', render: (v: unknown) => (v as string) || '-' },
    { key: 'status', header: 'Status', width: '90px', render: (v: unknown) => <StatusBadge status={statusMap[v as string] || 'degraded'} label={v as string} /> },
    { key: 'next_run_at', header: 'Next Run', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'actions',
      header: '',
      width: '160px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => openEdit(row as JobInfo)}>Edit</button>
          <button className="btn btn-sm" onClick={() => handlePauseResume(row as JobInfo)}>{row.status === 'paused' ? 'Resume' : 'Pause'}</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(row.id as string)}>Delete</button>
        </div>
      ),
    },
  ];

  const execColumns: any[] = [
    { key: 'id', header: 'Run ID', width: '140px', render: (v: unknown) => <span style={{ fontFamily: 'var(--font-mono)', fontSize: 10 }}>{String(v).slice(0, 8)}...</span> },
    { key: 'status', header: 'Status', width: '90px', render: (v: unknown) => {
      const s = v as string;
      return <StatusBadge status={s === 'success' ? 'healthy' : s === 'failed' || s === 'timeout' ? 'critical' : 'degraded'} label={s} />;
    }},
    { key: 'started_at', header: 'Started', width: '130px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    { key: 'duration_ms', header: 'Duration', width: '70px', render: (v: unknown) => v ? `${(v as number).toFixed(0)}ms` : '-' },
    { key: 'trigger', header: 'Trigger', width: '70px' },
    { key: 'result', header: 'Result', render: (v: unknown) => {
      const s = String(v ?? '');
      return s.length > 40 ? s.slice(0, 40) + '...' : s || '-';
    }},
  ];

  const activeCount = jobs?.filter(j => j.status === 'active').length ?? 0;
  const pausedCount = jobs?.filter(j => j.status === 'paused').length ?? 0;

  return (
    <div>
      <div className="page-header">
        <div className="flex justify-between items-center">
          <div>
            <h1>Scheduler</h1>
            <p>Cron and interval jobs with retries and concurrency control</p>
          </div>
          <button className="btn btn-primary" onClick={openCreate}>+ Create Job</button>
        </div>
      </div>

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Jobs" value={jobs?.length ?? '-'} color="accent" loading={jobsLoading} />
        <MetricCard title="Active" value={activeCount} color="success" loading={jobsLoading} />
        <MetricCard title="Paused" value={pausedCount} color={pausedCount > 0 ? 'warning' : 'success'} loading={jobsLoading} />
        <MetricCard title="Executions" value={execData?.pagination?.total?.toLocaleString() ?? '-'} color="info" loading={execLoading && !!selectedJob} />
      </div>

      <div className="card mb-4">
        <div className="flex justify-between items-center mb-4">
          <div className="card-title" style={{ margin: 0 }}>Jobs</div>
          <button className="btn btn-sm" onClick={() => refetchJobs()}>Refresh</button>
        </div>
        <DataTable
          columns={jobColumns}
          data={(jobs || []) as unknown as Record<string, unknown>[]}
          loading={jobsLoading}
          onRowClick={(row) => { setSelectedJob(row.id as string); setExecPage(1); }}
          emptyMessage="No jobs — create one to get started"
        />
      </div>

      {selectedJob && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>History: {selectedInfo?.name || selectedJob}</div>
              {selectedInfo && (
                <div className="text-sm text-muted mt-2">
                  Handler: <span style={{ fontFamily: 'var(--font-mono)' }}>{selectedInfo.handler || '—'}</span> · Max retries: {selectedInfo.max_retries} · Timeout: {selectedInfo.timeout_seconds}s · Tags: {selectedInfo.tags.join(', ') || 'none'}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm btn-primary" onClick={handleTrigger}>Trigger Now</button>
              {selectedInfo && <button className="btn btn-sm" onClick={() => handlePauseResume(selectedInfo)}>{selectedInfo.status === 'paused' ? 'Resume' : 'Pause'}</button>}
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(selectedJob)}>Delete Job</button>
              <button className="btn btn-sm" onClick={() => setSelectedJob(null)}>Close</button>
            </div>
          </div>

          {triggerStatus && <div className={`callout ${triggerStatus.includes('failed') ? 'error' : 'info'}`}>{triggerStatus}</div>}

          <DataTable
            columns={execColumns}
            data={(execData?.data || []) as unknown as Record<string, unknown>[]}
            loading={execLoading}
            pagination={execData?.pagination}
            onPageChange={setExecPage}
            emptyMessage="No executions yet — trigger the job"
          />
        </div>
      )}

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title={editingJob ? 'Edit Job' : 'Create Job'} size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreateOrUpdate} disabled={createLoading}>{createLoading ? 'Saving...' : editingJob ? 'Update' : 'Create'}</button>
          </div>
        }>
        <div className="grid grid-cols-2 gap-3">
          <div className="form-group">
            <label>Name *</label>
            <input className="form-input" value={createForm.name} onChange={e => setCreateForm({ ...createForm, name: e.target.value })} placeholder="daily-report" />
          </div>
          <div className="form-group">
            <label>Type</label>
            <select className="form-select" value={createForm.type} onChange={e => setCreateForm({ ...createForm, type: e.target.value })}>
              <option value="cron">Cron</option>
              <option value="once">Once</option>
              <option value="interval">Interval</option>
            </select>
          </div>
          <div className="form-group">
            <label>Schedule {createForm.type === 'cron' && '(cron)'}</label>
            <input className="form-input" value={createForm.schedule} onChange={e => setCreateForm({ ...createForm, schedule: e.target.value })} placeholder="0 * * * * (every hour)" />
            <div className="form-hint">Examples: */5 * * * * (5m), 0 0 * * * (daily), @hourly</div>
          </div>
          <div className="form-group">
            <label>Handler</label>
            <input className="form-input" value={createForm.handler} onChange={e => setCreateForm({ ...createForm, handler: e.target.value })} placeholder="echo hello or https://hook" />
          </div>
          <div className="form-group">
            <label>Max Retries</label>
            <input className="form-input" type="number" value={createForm.max_retries} onChange={e => setCreateForm({ ...createForm, max_retries: parseInt(e.target.value, 10) || 0 })} />
          </div>
          <div className="form-group">
            <label>Payload (JSON)</label>
            <textarea className="form-input json-editor" value={createForm.payload} onChange={e => setCreateForm({ ...createForm, payload: e.target.value })} rows={3} placeholder='{"key": "value"}' />
          </div>
        </div>
        {formError && <div className="callout error" style={{ marginTop: 12 }}>{formError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteJobId} onClose={() => setDeleteJobId(null)} onConfirm={handleDelete} title="Delete Job" message={`Delete job ${deleteJobId}? This cannot be undone.`} confirmText="Delete" variant="danger" />
    </div>
  );
}
