import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { JobInfo } from '../types';
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
  const [jobDetail, setJobDetail] = useState<JobInfo | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [showCreate, setShowCreate] = useState(false);
  const [createForm, setCreateForm] = useState({ name: '', type: 'cron', schedule: '*/5 * * * *', payload: '{}', max_retries: 3 });
  const [formError, setFormError] = useState<string | null>(null);
  const [triggerStatus, setTriggerStatus] = useState<string | null>(null);
  const [deleteJobId, setDeleteJobId] = useState<string | null>(null);

  const { data: jobs, loading: jobsLoading, refetch: refetchJobs } = useApi<JobInfo[]>(() => api.getJobs(), []);
  const { data: stats } = useApi<any>(() => api.getSchedulerStats().catch(() => null), []);
  const selectedInfo = jobs?.find(j => j.id === selectedJob);

  const openJob = async (id: string) => {
    setSelectedJob(id);
    setTriggerStatus(null);
    setJobDetail(null);
    setDetailLoading(true);
    try {
      const detail = await api.getJob(id);
      setJobDetail(detail);
    } catch {
      setJobDetail(null);
    } finally {
      setDetailLoading(false);
    }
  };

  const closeJob = () => {
    setSelectedJob(null);
    setJobDetail(null);
  };

  const handleTrigger = async () => {
    if (!selectedJob) return;
    setTriggerStatus(null);
    try {
      await api.triggerJob(selectedJob);
      setTriggerStatus('Job triggered successfully');
      const detail = await api.getJob(selectedJob);
      setJobDetail(detail);
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
      const detail = await api.getJob(job.id);
      setJobDetail(detail);
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Failed');
    }
  };

  const handleDelete = async () => {
    if (!deleteJobId) return;
    try {
      await api.deleteJob(deleteJobId);
      if (selectedJob === deleteJobId) closeJob();
      setDeleteJobId(null);
      refetchJobs();
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Delete failed');
    }
  };

  const handleCreate = async () => {
    if (!createForm.name.trim()) {
      setFormError('Name required');
      return;
    }
    setFormError(null);
    let payload: Record<string, unknown> = {};
    if (createForm.payload.trim()) {
      try {
        payload = JSON.parse(createForm.payload);
      } catch {
        setFormError('Payload must be valid JSON');
        return;
      }
    }
    try {
      await api.createJob({ name: createForm.name.trim(), type: createForm.type, schedule: createForm.schedule, payload, max_retries: createForm.max_retries });
      setShowCreate(false);
      setCreateForm({ name: '', type: 'cron', schedule: '*/5 * * * *', payload: '{}', max_retries: 3 });
      refetchJobs();
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : 'Create failed');
    }
  };

  const jobColumns: any[] = [
    { key: 'name', header: 'Job Name' },
    { key: 'type', header: 'Type', width: '80px' },
    { key: 'status', header: 'Status', width: '90px', render: (v: unknown) => <StatusBadge status={statusMap[v as string] || 'degraded'} label={v as string} /> },
    { key: 'next_run_at', header: 'Next Run', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    { key: 'last_run_at', header: 'Last Run', width: '140px', render: (v: unknown) => v ? new Date(v as number).toLocaleString() : '-' },
    {
      key: 'actions',
      header: '',
      width: '130px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={e => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => handlePauseResume(row as JobInfo)}>{row.status === 'paused' ? 'Resume' : 'Pause'}</button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(row.id as string)}>Delete</button>
        </div>
      ),
    },
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
          <button className="btn btn-primary" onClick={() => setShowCreate(true)}>+ Create Job</button>
        </div>
      </div>

      <div className="grid grid-cols-4 mb-4">
        <MetricCard title="Jobs" value={jobs?.length ?? '-'} color="accent" loading={jobsLoading} />
        <MetricCard title="Pending" value={stats?.jobs_pending ?? '-'} color="info" loading={jobsLoading} />
        <MetricCard title="Running" value={stats?.jobs_running ?? '-'} color="success" loading={jobsLoading} />
        <MetricCard title="Failed" value={stats?.jobs_failed ?? '-'} color={stats?.jobs_failed > 0 ? 'danger' : 'success'} loading={jobsLoading} />
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
          onRowClick={(row) => openJob(row.id as string)}
          emptyMessage="No jobs — create one to get started"
        />
      </div>

      {selectedJob && (
        <div className="card">
          <div className="flex items-center justify-between mb-4">
            <div>
              <div className="card-title" style={{ margin: 0 }}>Job: {selectedInfo?.name || selectedJob}</div>
              {detailLoading && <div className="text-sm text-muted mt-2">Loading details...</div>}
              {jobDetail && (
                <div className="text-sm text-muted mt-2">
                  Type: <span style={{ fontFamily: 'var(--font-mono)' }}>{jobDetail.type}</span> · State: {jobDetail.status} · Max retries: {jobDetail.max_retries} · Retries used: {jobDetail.max_retries > 0 ? '? ' : '0'}
                </div>
              )}
            </div>
            <div className="flex gap-2">
              <button className="btn btn-sm btn-primary" onClick={handleTrigger}>Trigger Now</button>
              {jobDetail && <button className="btn btn-sm" onClick={() => handlePauseResume(jobDetail)}>{jobDetail.status === 'paused' ? 'Resume' : 'Pause'}</button>}
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(selectedJob)}>Delete Job</button>
              <button className="btn btn-sm" onClick={closeJob}>Close</button>
            </div>
          </div>

          {triggerStatus && <div className={`callout ${triggerStatus.includes('failed') ? 'error' : 'info'}`}>{triggerStatus}</div>}

          {jobDetail && (
            <div style={{ marginTop: 8 }}>
              <div className="detail-row"><span className="detail-label">Schedule Type</span><span className="detail-value">{jobDetail.type}</span></div>
              <div className="detail-row"><span className="detail-label">State</span><span className="detail-value">{jobDetail.status}</span></div>
              <div className="detail-row"><span className="detail-label">Next Run</span><span className="detail-value">{jobDetail.next_run_at ? new Date(jobDetail.next_run_at).toLocaleString() : '-'}</span></div>
              <div className="detail-row"><span className="detail-label">Last Run</span><span className="detail-value">{jobDetail.last_run_at ? new Date(jobDetail.last_run_at).toLocaleString() : '-'}</span></div>
              <div className="detail-row"><span className="detail-label">Max Retries</span><span className="detail-value">{jobDetail.max_retries}</span></div>
            </div>
          )}
        </div>
      )}

      <Modal isOpen={showCreate} onClose={() => setShowCreate(false)} title="Create Job" size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreate}>Create</button>
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
          {createForm.type === 'cron' && (
            <div className="form-group">
              <label>Schedule (cron)</label>
              <input className="form-input" value={createForm.schedule} onChange={e => setCreateForm({ ...createForm, schedule: e.target.value })} placeholder="0 * * * * (every hour)" />
              <div className="form-hint">Examples: */5 * * * * (5m), 0 0 * * * (daily), @hourly</div>
            </div>
          )}
          <div className="form-group">
            <label>Max Retries</label>
            <input className="form-input" type="number" value={createForm.max_retries} onChange={e => setCreateForm({ ...createForm, max_retries: parseInt(e.target.value, 10) || 0 })} />
          </div>
          <div className="form-group" style={{ gridColumn: '1 / -1' }}>
            <label>Payload (JSON, optional)</label>
            <textarea className="form-input json-editor" value={createForm.payload} onChange={e => setCreateForm({ ...createForm, payload: e.target.value })} rows={3} placeholder='{"key": "value"}' />
            <div className="form-hint">Sent to the job runner as the action payload</div>
          </div>
        </div>
        {formError && <div className="callout error" style={{ marginTop: 12 }}>{formError}</div>}
      </Modal>

      <ConfirmDialog isOpen={!!deleteJobId} onClose={() => setDeleteJobId(null)} onConfirm={handleDelete} title="Delete Job" message={`Delete job ${deleteJobId}? This cannot be undone.`} confirmText="Delete" variant="danger" />
    </div>
  );
}
