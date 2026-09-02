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

function formatRelative(ts: number | null | undefined): string {
  if (ts == null || ts === 0) return '—';
  const ms = ts < 1e12 ? ts * 1000 : ts;
  const diff = ms - Date.now();
  const abs = Math.abs(diff);
  const sec = Math.round(abs / 1000);
  const future = diff > 0;
  let rel: string;
  if (sec < 60) rel = `${sec}s`;
  else if (sec < 3600) rel = `${Math.floor(sec / 60)}m ${sec % 60 ? `${sec % 60}s` : ''}`.trim();
  else if (sec < 86400) {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    rel = m ? `${h}h ${m}m` : `${h}h`;
  } else {
    const d = Math.floor(sec / 86400);
    const h = Math.floor((sec % 86400) / 3600);
    rel = h ? `${d}d ${h}h` : `${d}d`;
  }
  return future ? `in ${rel}` : `${rel} ago`;
}

function formatAbsolute(ts: number | null | undefined): string {
  if (ts == null || ts === 0) return '';
  const ms = ts < 1e12 ? ts * 1000 : ts;
  try {
    return new Date(ms).toLocaleString();
  } catch {
    return String(ts);
  }
}

function TypePill({ type }: { type: string }) {
  const t = String(type).toLowerCase();
  const styleMap: Record<string, { bg: string; color: string; border: string }> = {
    cron: { bg: 'var(--bg-primary)', color: 'var(--text-secondary)', border: 'var(--border)' },
    interval: { bg: 'rgba(59,130,246,0.08)', color: '#2563eb', border: 'rgba(59,130,246,0.2)' },
    once: { bg: 'rgba(16,185,129,0.08)', color: '#059669', border: 'rgba(16,185,129,0.2)' },
  };
  const s = styleMap[t] || styleMap.cron;
  return (
    <span
      title={t}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        fontSize: 11,
        fontWeight: 600,
        letterSpacing: '0.03em',
        textTransform: 'uppercase',
        padding: '2px 7px',
        borderRadius: 999,
        background: s.bg,
        color: s.color,
        border: `1px solid ${s.border}`,
        fontFamily: 'var(--font-mono)',
        lineHeight: 1.2,
      }}
    >
      {t}
    </span>
  );
}

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
  const selectedInfo = jobs?.find((j) => j.id === selectedJob);

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
    setTriggerStatus(null);
  };

  const handleTrigger = async () => {
    if (!selectedJob) return;
    setTriggerStatus(null);
    try {
      await api.triggerJob(selectedJob);
      setTriggerStatus('Job triggered successfully');
      const detail = await api.getJob(selectedJob);
      setJobDetail(detail);
      refetchJobs();
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Trigger failed');
    }
  };

  const handlePauseResume = async (job: JobInfo) => {
    setTriggerStatus(null);
    try {
      if (job.status === 'paused') {
        await api.resumeJob(job.id);
      } else {
        await api.pauseJob(job.id);
      }
      refetchJobs();
      // refresh detail if viewing this job
      if (selectedJob === job.id) {
        try {
          const detail = await api.getJob(job.id);
          setJobDetail(detail);
        } catch {}
      }
    } catch (err: unknown) {
      setTriggerStatus(err instanceof Error ? err.message : 'Failed to toggle job');
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
      setFormError('Name is required');
      return;
    }
    if (createForm.type === 'cron' && !createForm.schedule.trim()) {
      setFormError('Schedule is required for cron jobs');
      return;
    }
    setFormError(null);
    let payload: Record<string, unknown> = {};
    const raw = createForm.payload.trim();
    if (raw && raw !== '{}' && raw !== '') {
      try {
        const parsed = JSON.parse(raw);
        if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
          setFormError('Payload must be a JSON object (e.g. {"key":"value"})');
          return;
        }
        payload = parsed as Record<string, unknown>;
      } catch (e) {
        setFormError(e instanceof Error ? `Invalid JSON: ${e.message}` : 'Payload must be valid JSON');
        return;
      }
    } else if (raw) {
      try {
        payload = JSON.parse(raw);
      } catch {
        setFormError('Payload must be valid JSON');
        return;
      }
    }
    try {
      await api.createJob({
        name: createForm.name.trim(),
        type: createForm.type,
        schedule: createForm.schedule,
        payload,
        max_retries: createForm.max_retries,
      });
      setShowCreate(false);
      setCreateForm({ name: '', type: 'cron', schedule: '*/5 * * * *', payload: '{}', max_retries: 3 });
      refetchJobs();
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : 'Create failed');
    }
  };

  const isJobsEmpty = !jobsLoading && (!jobs || jobs.length === 0);

  const jobColumns: any[] = [
    {
      key: 'name',
      header: 'Job Name',
      render: (_: unknown, row: any) => (
        <span
          style={{ fontWeight: 500, color: 'var(--text-primary)', fontSize: 13, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', display: 'block', maxWidth: 220 }}
          title={String(row.name)}
        >
          {String(row.name)}
        </span>
      ),
    },
    { key: 'type', header: 'Type', width: '96px', render: (v: unknown) => <TypePill type={String(v)} /> },
    {
      key: 'status',
      header: 'Status',
      width: '108px',
      render: (v: unknown) => <StatusBadge status={statusMap[String(v)] || 'degraded'} label={String(v)} />,
    },
    {
      key: 'next_run_at',
      header: 'Next Run',
      width: '148px',
      render: (v: unknown) => {
        if (v == null || v === 0 || v === '') return <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>—</span>;
        const rel = formatRelative(v as number);
        const abs = formatAbsolute(v as number);
        return (
          <span title={abs} style={{ fontSize: 12, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
            {rel}
            <span style={{ color: 'var(--text-muted)', marginLeft: 6, fontSize: 11 }}>{abs ? `· ${abs.split(',')[0]}` : ''}</span>
          </span>
        );
      },
    },
    {
      key: 'last_run_at',
      header: 'Last Run',
      width: '148px',
      render: (v: unknown) => {
        if (v == null || v === 0 || v === '') return <span style={{ color: 'var(--text-muted)', fontSize: 12 }}>—</span>;
        const rel = formatRelative(v as number);
        const abs = formatAbsolute(v as number);
        return (
          <span title={abs} style={{ fontSize: 12, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
            {rel}
            <span style={{ color: 'var(--text-muted)', marginLeft: 6, fontSize: 11 }}>{abs ? `· ${abs.split(',')[0]}` : ''}</span>
          </span>
        );
      },
    },
    {
      key: 'actions',
      header: '',
      width: '148px',
      render: (_: unknown, row: any) => (
        <div className="actions" onClick={(e) => e.stopPropagation()}>
          <button className="btn btn-sm" onClick={() => handlePauseResume(row as JobInfo)}>
            {row.status === 'paused' ? 'Resume' : 'Pause'}
          </button>
          <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(row.id as string)}>
            Delete
          </button>
        </div>
      ),
    },
  ];

  const payloadPretty = (() => {
    if (!jobDetail) return null;
    const p = jobDetail.payload;
    if (!p || (typeof p === 'object' && Object.keys(p as object).length === 0)) return null;
    try {
      return JSON.stringify(p, null, 2);
    } catch {
      return String(p);
    }
  })();

  const successHint = triggerStatus ? /success|triggered/i.test(triggerStatus) : false;
  const errorHint = triggerStatus ? /fail|error/i.test(triggerStatus) : false;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 0 }}>
        <div className="flex justify-between items-center" style={{ gap: 12 }}>
          <div>
            <h1 style={{ margin: 0, fontSize: 20, fontWeight: 700, letterSpacing: '-0.02em' }}>Scheduler</h1>
            <p style={{ margin: '2px 0 0', color: 'var(--text-muted)', fontSize: 13, lineHeight: 1.4 }}>
              Cron, interval and one-shot jobs — schedule, retry, and control concurrency
            </p>
          </div>
          <button
            className="btn btn-primary"
            onClick={() => {
              setFormError(null);
              setShowCreate(true);
            }}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 6, fontWeight: 600 }}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden>
              <path d="M7 2.5v9M2.5 7h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
            Create Job
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-4" style={{ gap: 12 }}>
        <MetricCard title="Jobs" value={jobs?.length ?? '-'} color="accent" loading={jobsLoading} />
        <MetricCard title="Pending" value={stats?.jobs_pending ?? stats?.total_scheduled ?? '-'} color="info" loading={jobsLoading} />
        <MetricCard title="Running" value={stats?.jobs_running ?? '-'} color="success" loading={jobsLoading} />
        <MetricCard title="Failed" value={stats?.jobs_failed ?? stats?.total_failures ?? '-'} color={(stats?.jobs_failed ?? stats?.total_failures ?? 0) > 0 ? 'danger' : 'success'} loading={jobsLoading} />
      </div>

      {/* Jobs table */}
      <div className="card" style={{ padding: 14 }}>
        <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
          <div className="card-title" style={{ margin: 0, fontSize: 13, fontWeight: 600 }}>
            Jobs
            {!jobsLoading && jobs && jobs.length > 0 && (
              <span style={{ marginLeft: 8, fontWeight: 400, color: 'var(--text-muted)', fontSize: 12 }}>
                {jobs.length} total · click a row for details
              </span>
            )}
          </div>
          <button className="btn btn-sm" onClick={() => refetchJobs()}>
            Refresh
          </button>
        </div>
        <DataTable
          columns={jobColumns}
          data={(jobs || []) as unknown as Record<string, unknown>[]}
          loading={jobsLoading}
          onRowClick={(row) => openJob(row.id as string)}
          emptyMessage="No jobs — create one to get started"
        />
        {isJobsEmpty && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 12,
              marginTop: 10,
              padding: '10px 12px',
              background: 'var(--bg-primary)',
              border: '1px dashed var(--border)',
              borderRadius: 6,
            }}
          >
            <div style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.4 }}>
              <span style={{ fontWeight: 600, color: 'var(--text-secondary)' }}>No jobs — create one.</span> Cron jobs need a schedule like <code style={{ fontSize: 11 }}>*/5 * * * *</code>; use Once/Interval for ad-hoc runs.
            </div>
            <button className="btn btn-sm btn-primary" onClick={() => setShowCreate(true)}>
              Create Job
            </button>
          </div>
        )}
        {!isJobsEmpty && (
          <div style={{ marginTop: 8, fontSize: 11, color: 'var(--text-muted)', display: 'flex', gap: 12, flexWrap: 'wrap' }}>
            <span>
              Status via badge · Next/Last = relative + date on hover · <code style={{ fontSize: 11 }}>cron</code> is recurring, <code style={{ fontSize: 11 }}>interval</code> is periodic, <code style={{ fontSize: 11 }}>once</code> is one-shot
            </span>
          </div>
        )}
      </div>

      {/* Selected job detail */}
      {selectedJob && (
        <div className="card" style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div className="flex items-center justify-between" style={{ gap: 12, flexWrap: 'wrap' }}>
            <div style={{ minWidth: 0 }}>
              <div className="card-title" style={{ margin: 0, display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <span style={{ fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--text-muted)', fontWeight: 600 }}>Job</span>
                <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {selectedInfo?.name || selectedJob}
                </span>
                {selectedInfo && <TypePill type={selectedInfo.type} />}
                {jobDetail && <StatusBadge status={statusMap[jobDetail.status] || 'degraded'} label={jobDetail.status} />}
              </div>
              {detailLoading && <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 6 }}>Loading details…</div>}
              {!detailLoading && jobDetail && (
                <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 6, display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                  <span>
                    Schedule:{' '}
                    <code style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-secondary)', fontSize: 11 }}>
                      {(jobDetail as any).schedule ?? jobDetail.schedule ?? (jobDetail.type === 'cron' ? selectedInfo?.schedule ?? '—' : '—')}
                    </code>
                  </span>
                  <span style={{ color: 'var(--border-strong)' }}>·</span>
                  <span>
                    Max retries: <strong style={{ color: 'var(--text-secondary)' }}>{jobDetail.max_retries}</strong>
                  </span>
                </div>
              )}
            </div>
            <div className="flex gap-2" style={{ flexShrink: 0, flexWrap: 'wrap' }}>
              <button className="btn btn-sm btn-primary" onClick={handleTrigger} disabled={detailLoading}>
                Trigger Now
              </button>
              {jobDetail && (
                <button className="btn btn-sm" onClick={() => handlePauseResume(jobDetail)}>
                  {jobDetail.status === 'paused' ? 'Resume' : 'Pause'}
                </button>
              )}
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteJobId(selectedJob)}>
                Delete
              </button>
              <button className="btn btn-sm" onClick={closeJob}>
                Close
              </button>
            </div>
          </div>

          {triggerStatus && (
            <div className={`callout ${errorHint ? 'error' : successHint ? 'info' : 'info'}`} style={{ margin: 0 }}>
              {triggerStatus}
            </div>
          )}

          {detailLoading ? (
            <div style={{ padding: 12, display: 'flex', alignItems: 'center', gap: 10, color: 'var(--text-muted)', fontSize: 12, background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6 }}>
              <span className="loading-spinner" style={{ padding: 0, width: 16, height: 16 }} />
              Fetching job details…
            </div>
          ) : jobDetail ? (
            <>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: 10 }}>
                <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                  <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 4 }}>Schedule Type</div>
                  <div style={{ fontSize: 13, color: 'var(--text-primary)', fontFamily: 'var(--font-mono)' }}>{jobDetail.type}</div>
                </div>
                <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                  <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 4 }}>State</div>
                  <div style={{ fontSize: 13, color: 'var(--text-primary)' }}>
                    <StatusBadge status={statusMap[jobDetail.status] || 'degraded'} label={jobDetail.status} />
                  </div>
                </div>
                <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                  <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 4 }}>Max Retries</div>
                  <div style={{ fontSize: 13, color: 'var(--text-primary)', fontFamily: 'var(--font-mono)' }}>{jobDetail.max_retries}</div>
                </div>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                  <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 4 }}>Next Run</div>
                  <div title={formatAbsolute(jobDetail.next_run_at)} style={{ fontSize: 13, color: 'var(--text-primary)' }}>
                    {jobDetail.next_run_at ? (
                      <>
                        <span style={{ fontWeight: 500 }}>{formatRelative(jobDetail.next_run_at)}</span>
                        <span style={{ color: 'var(--text-muted)', marginLeft: 6, fontSize: 11 }}>{formatAbsolute(jobDetail.next_run_at)}</span>
                      </>
                    ) : (
                      <span style={{ color: 'var(--text-muted)' }}>— not scheduled</span>
                    )}
                  </div>
                </div>
                <div style={{ background: 'var(--bg-primary)', border: '1px solid var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                  <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 4 }}>Last Run</div>
                  <div title={formatAbsolute(jobDetail.last_run_at)} style={{ fontSize: 13, color: 'var(--text-primary)' }}>
                    {jobDetail.last_run_at ? (
                      <>
                        <span style={{ fontWeight: 500 }}>{formatRelative(jobDetail.last_run_at)}</span>
                        <span style={{ color: 'var(--text-muted)', marginLeft: 6, fontSize: 11 }}>{formatAbsolute(jobDetail.last_run_at)}</span>
                      </>
                    ) : (
                      <span style={{ color: 'var(--text-muted)' }}>— never run</span>
                    )}
                  </div>
                </div>
              </div>

              <div>
                <div style={{ fontSize: 11, fontWeight: 600, letterSpacing: '0.04em', textTransform: 'uppercase', color: 'var(--text-muted)', marginBottom: 6 }}>Payload</div>
                {payloadPretty ? (
                  <pre
                    style={{
                      margin: 0,
                      maxHeight: 200,
                      overflow: 'auto',
                      background: 'var(--bg-primary)',
                      border: '1px solid var(--border)',
                      borderRadius: 6,
                      padding: '10px 12px',
                      fontFamily: 'var(--font-mono)',
                      fontSize: 11.5,
                      lineHeight: 1.5,
                      color: 'var(--text-secondary)',
                      whiteSpace: 'pre-wrap',
                      wordBreak: 'break-all',
                    }}
                  >
                    {payloadPretty}
                  </pre>
                ) : (
                  <div style={{ fontSize: 12, color: 'var(--text-muted)', background: 'var(--bg-primary)', border: '1px dashed var(--border)', borderRadius: 6, padding: '10px 12px' }}>
                    No payload — this job runs without extra data. Add JSON payload when creating a job if the handler expects it.
                  </div>
                )}
              </div>
            </>
          ) : (
            <div style={{ fontSize: 12, color: 'var(--text-muted)', background: 'var(--bg-primary)', border: '1px dashed var(--border)', borderRadius: 6, padding: '10px 12px' }}>
              Could not load job details. The job may have been deleted — try refreshing.
            </div>
          )}
        </div>
      )}

      <Modal
        isOpen={showCreate}
        onClose={() => setShowCreate(false)}
        title="Create Job"
        size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={() => setShowCreate(false)}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleCreate}>
              Create
            </button>
          </div>
        }
      >
        <div className="grid grid-cols-2" style={{ gap: 12 }}>
          <div className="form-group" style={{ margin: 0 }}>
            <label>
              Name <span style={{ color: 'var(--text-danger, #dc2626)' }}>*</span>
            </label>
            <input
              className="form-input"
              value={createForm.name}
              onChange={(e) => setCreateForm({ ...createForm, name: e.target.value })}
              placeholder="daily-report"
            />
            <div className="form-hint">Unique within scheduler. Use lowercase with dashes.</div>
          </div>
          <div className="form-group" style={{ margin: 0 }}>
            <label>Type</label>
            <select className="form-select" value={createForm.type} onChange={(e) => setCreateForm({ ...createForm, type: e.target.value })}>
              <option value="cron">Cron</option>
              <option value="once">Once</option>
              <option value="interval">Interval</option>
            </select>
            <div className="form-hint">{createForm.type === 'cron' ? 'Recurring on cron schedule' : createForm.type === 'interval' ? 'Periodic (server interval)' : 'One-shot — runs once'}</div>
          </div>
          {createForm.type === 'cron' ? (
            <div className="form-group" style={{ margin: 0 }}>
              <label>
                Schedule (cron) <span style={{ color: 'var(--text-danger, #dc2626)' }}>*</span>
              </label>
              <input
                className="form-input"
                value={createForm.schedule}
                onChange={(e) => setCreateForm({ ...createForm, schedule: e.target.value })}
                placeholder="0 * * * * (every hour)"
                style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }}
              />
              <div className="form-hint">
                Examples: <code style={{ fontSize: 11 }}>*/5 * * * *</code> (every 5m), <code style={{ fontSize: 11 }}>0 0 * * *</code> (daily midnight), <code style={{ fontSize: 11 }}>0 * * * *</code> (hourly). Server validates cron syntax.
              </div>
            </div>
          ) : (
            <div className="form-group" style={{ margin: 0, display: 'flex', alignItems: 'center', background: 'var(--bg-primary)', border: '1px dashed var(--border)', borderRadius: 6, padding: '10px 12px' }}>
              <span style={{ fontSize: 12, color: 'var(--text-muted)', lineHeight: 1.4 }}>
                {createForm.type === 'once' ? 'Runs once — schedule not used. Use payload for run args.' : 'Interval jobs repeat on a fixed interval (server default unless extended).'}
              </span>
            </div>
          )}
          <div className="form-group" style={{ margin: 0 }}>
            <label>Max Retries</label>
            <input
              className="form-input"
              type="number"
              min={0}
              max={20}
              value={createForm.max_retries}
              onChange={(e) => setCreateForm({ ...createForm, max_retries: parseInt(e.target.value, 10) || 0 })}
            />
            <div className="form-hint">0 = no retry. Failed runs retry with backoff.</div>
          </div>
          <div className="form-group" style={{ gridColumn: '1 / -1', margin: 0 }}>
            <label>
              Payload <span style={{ fontWeight: 400, color: 'var(--text-muted)', textTransform: 'none', letterSpacing: 0 }}>(JSON, optional)</span>
            </label>
            <textarea
              className="form-input json-editor"
              value={createForm.payload}
              onChange={(e) => setCreateForm({ ...createForm, payload: e.target.value })}
              rows={4}
              placeholder='{"key": "value"}'
              style={{ fontFamily: 'var(--font-mono)', fontSize: 12, minHeight: 96 }}
            />
            <div className="form-hint">Must be a JSON object. Sent to the job runner as the action payload — keep it small and serializable.</div>
          </div>
        </div>
        {formError && <div className="callout error" style={{ marginTop: 12 }}>{formError}</div>}
      </Modal>

      <ConfirmDialog
        isOpen={!!deleteJobId}
        onClose={() => setDeleteJobId(null)}
        onConfirm={handleDelete}
        title="Delete Job"
        message={`Delete job "${deleteJobId}"? This cannot be undone.`}
        confirmText="Delete"
        variant="danger"
      />
    </div>
  );
}
