import { useState, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { IndexInfo, SearchResult } from '../types';
import MetricCard from '../components/MetricCard';
import DataTable from '../components/DataTable';
import Modal from '../components/Modal';
import ConfirmDialog from '../components/ConfirmDialog';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function scoreTone(score: number, max: number): { bg: string; color: string; label: string } {
  const normalized = max > 0 ? score / max : score;
  if (normalized >= 0.75 || score >= 1.5) return { bg: 'rgba(16,185,129,0.14)', color: 'var(--success)', label: 'high' };
  if (normalized >= 0.4 || score >= 0.6) return { bg: 'rgba(245,158,11,0.14)', color: 'var(--warning)', label: 'med' };
  return { bg: 'var(--bg-tertiary)', color: 'var(--text-muted)', label: 'low' };
}

type FieldDef = { name: string; type: string };

const FIELD_TYPES: Array<{ value: string; label: string; help: string }> = [
  { value: 'text', label: 'Text — Full-text, tokenized, BM25 searchable', help: 'e.g. title, body — split into words, ranked by relevance' },
  { value: 'keyword', label: 'Keyword — Exact match, not tokenized', help: 'e.g. email, status — exact filter, case-sensitive' },
  { value: 'integer', label: 'Integer — Whole numbers, range filters', help: 'Whole numbers — supports >, <, range queries' },
  { value: 'float', label: 'Float — Decimal numbers', help: 'Decimal numbers — prices, scores' },
  { value: 'boolean', label: 'Boolean — true/false', help: 'true / false flags' },
];

const TYPE_BADGE: Record<string, { bg: string; color: string }> = {
  text: { bg: 'rgba(99,102,241,0.12)', color: '#6366f1' },
  keyword: { bg: 'rgba(14,165,233,0.12)', color: '#0ea5e9' },
  integer: { bg: 'rgba(245,158,11,0.12)', color: '#d97706' },
  float: { bg: 'rgba(236,72,153,0.12)', color: '#ec4899' },
  boolean: { bg: 'rgba(16,185,129,0.12)', color: '#059669' },
};

const TEMPLATES: Array<{ id: string; label: string; name: string; hint: string; fields: FieldDef[] }> = [
  {
    id: 'products',
    label: 'Products',
    name: 'products',
    hint: 'title text, description text, price float, in_stock boolean',
    fields: [
      { name: 'title', type: 'text' },
      { name: 'description', type: 'text' },
      { name: 'price', type: 'float' },
      { name: 'in_stock', type: 'boolean' },
    ],
  },
  {
    id: 'blog',
    label: 'Blog',
    name: 'blog',
    hint: 'title text, body text, author keyword',
    fields: [
      { name: 'title', type: 'text' },
      { name: 'body', type: 'text' },
      { name: 'author', type: 'keyword' },
    ],
  },
  {
    id: 'logs',
    label: 'Logs',
    name: 'logs',
    hint: 'message text, level keyword, timestamp integer',
    fields: [
      { name: 'message', type: 'text' },
      { name: 'level', type: 'keyword' },
      { name: 'timestamp', type: 'integer' },
    ],
  },
];

export default function SearchPage() {
  const [selectedIndex, setSelectedIndex] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [queryResult, setQueryResult] = useState<SearchResult | null>(null);
  const [queryLoading, setQueryLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);

  const [showCreate, setShowCreate] = useState(false);
  const [newIndexName, setNewIndexName] = useState('');
  const [newFields, setNewFields] = useState<FieldDef[]>([
    { name: 'title', type: 'text' },
    { name: 'body', type: 'text' },
  ]);
  const [createError, setCreateError] = useState<string | null>(null);
  const [createLoading, setCreateLoading] = useState(false);
  const [indexFieldsCache, setIndexFieldsCache] = useState<Record<string, FieldDef[]>>({});

  const [showAddDocs, setShowAddDocs] = useState(false);
  const [docsJson, setDocsJson] = useState('[\n  {"id": "1", "title": "Hello world", "body": "This is a test document"}\n]');
  const [addDocsError, setAddDocsError] = useState<string | null>(null);
  const [addDocsLoading, setAddDocsLoading] = useState(false);
  const docsJsonError = useMemo(() => {
    try {
      const v = JSON.parse(docsJson);
      const arr = Array.isArray(v) ? v : [v];
      if (arr.length === 0) return 'Array is empty — add at least one document.';
      const missing = arr.findIndex((d: unknown) => !d || typeof d !== 'object' || !('id' in (d as Record<string, unknown>)));
      if (missing >= 0) return `Document ${missing + 1} is missing required field "id".`;
      return null;
    } catch (e: unknown) {
      return e instanceof Error ? e.message : 'Invalid JSON';
    }
  }, [docsJson]);

  const [deleteIndexName, setDeleteIndexName] = useState<string | null>(null);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' } | null>(null);
  const showToast = (message: string, type: 'success' | 'error' = 'success') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const { data: indexes, loading: indexesLoading, refetch: refetchIndexes } = useApi<IndexInfo[]>(
    () => api.getIndexes(),
    []
  );

  const selectedMeta = indexes?.find((i) => i.name === selectedIndex) ?? null;
  const maxScore = useMemo(() => (queryResult ? Math.max(0, ...queryResult.hits.map((h) => h.score)) : 0), [queryResult]);

  const displayFields: FieldDef[] | null = useMemo(() => {
    if (!selectedIndex) return null;
    if (indexFieldsCache[selectedIndex]) return indexFieldsCache[selectedIndex];
    if (queryResult && queryResult.hits.length > 0 && queryResult.hits[0].fields) {
      return Object.keys(queryResult.hits[0].fields).map((k) => ({ name: k, type: '—' }));
    }
    return null;
  }, [selectedIndex, indexFieldsCache, queryResult]);

  const applyTemplate = (t: (typeof TEMPLATES)[number]) => {
    setNewIndexName(t.name);
    setNewFields(t.fields.map((f) => ({ ...f })));
    setCreateError(null);
    setShowCreate(true);
  };

  const openCreateBlank = () => {
    setCreateError(null);
    setShowCreate(true);
  };

  const handleSearch = async () => {
    if (!selectedIndex || !query.trim()) return;
    setQueryLoading(true);
    setQueryError(null);
    try {
      const result = await api.searchQuery(selectedIndex, query);
      setQueryResult(result);
    } catch (err: unknown) {
      setQueryError(err instanceof Error ? err.message : 'Search failed');
    } finally {
      setQueryLoading(false);
    }
  };

  const handleCreateIndex = async () => {
    const trimmed = newIndexName.trim();
    if (!trimmed) {
      setCreateError('Name is required — use lowercase letters, e.g. products');
      return;
    }
    if (!/^[a-z0-9_-]+$/.test(trimmed)) {
      setCreateError('Name may only contain a-z, 0-9, _ and - (lowercase, no spaces)');
      return;
    }
    const cleaned = newFields.filter((f) => f.name.trim());
    if (cleaned.length === 0) {
      setCreateError('Add at least one field — every index needs ≥1 field');
      return;
    }
    const names = cleaned.map((f) => f.name.trim());
    const dup = names.find((n, i) => names.indexOf(n) !== i);
    if (dup) {
      setCreateError(`Duplicate field name: "${dup}"`);
      return;
    }
    const badName = cleaned.find((f) => !/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(f.name.trim()));
    if (badName) {
      setCreateError(`Field "${badName.name}" — use letters, digits, underscore; start with letter or _`);
      return;
    }
    setCreateLoading(true);
    setCreateError(null);
    try {
      await api.createIndex(trimmed, cleaned.map((f) => ({ name: f.name.trim(), type: f.type })));
      setIndexFieldsCache((prev) => ({ ...prev, [trimmed]: cleaned.map((f) => ({ name: f.name.trim(), type: f.type })) }));
      showToast(`Index "${trimmed}" created with ${cleaned.length} field${cleaned.length === 1 ? '' : 's'}`);
      setShowCreate(false);
      setNewIndexName('');
      setNewFields([
        { name: 'title', type: 'text' },
        { name: 'body', type: 'text' },
      ]);
      setSelectedIndex(trimmed);
      setQueryResult(null);
      setQuery('');
      setQueryError(null);
      refetchIndexes();
    } catch (err: unknown) {
      setCreateError(err instanceof Error ? err.message : 'Create failed');
    } finally {
      setCreateLoading(false);
    }
  };

  const handleDeleteIndex = async () => {
    if (!deleteIndexName) return;
    try {
      await api.deleteIndex(deleteIndexName);
      showToast(`Index "${deleteIndexName}" deleted`);
      if (selectedIndex === deleteIndexName) {
        setSelectedIndex(null);
        setQueryResult(null);
        setQuery('');
      }
      setDeleteIndexName(null);
      refetchIndexes();
    } catch (err: unknown) {
      showToast(err instanceof Error ? err.message : 'Delete failed', 'error');
    }
  };

  const handleAddDocs = async () => {
    if (!selectedIndex) return;
    if (docsJsonError) {
      setAddDocsError(docsJsonError);
      return;
    }
    setAddDocsError(null);
    setAddDocsLoading(true);
    try {
      const docs = JSON.parse(docsJson);
      const arr = Array.isArray(docs) ? docs : [docs];
      await api.addDocuments(selectedIndex, arr);
      showToast(`Added ${arr.length} document${arr.length === 1 ? '' : 's'} to ${selectedIndex}`);
      setShowAddDocs(false);
      refetchIndexes();
    } catch (err: unknown) {
      setAddDocsError(err instanceof Error ? err.message : 'Add failed — check JSON');
    } finally {
      setAddDocsLoading(false);
    }
  };

  const indexColumns: any[] = [
    {
      key: 'name',
      header: 'Index',
      render: (v: unknown) => (
        <span style={{ fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 700, color: 'var(--text-primary)', cursor: 'pointer' }}>{String(v)}</span>
      ),
    },
    {
      key: 'document_count',
      header: 'Docs',
      width: '84px',
      render: (v: unknown) => <span style={{ fontVariantNumeric: 'tabular-nums', fontSize: 12 }}>{(v as number).toLocaleString()}</span>,
    },
    {
      key: 'index_size_bytes',
      header: 'Size',
      width: '84px',
      render: (v: unknown) => <span style={{ color: 'var(--text-secondary)', fontSize: 12 }}>{formatBytes(v as number)}</span>,
    },
    {
      key: 'field_count',
      header: 'Fields',
      width: '120px',
      render: (_: unknown, row: any) => {
        const count = row.field_count as number;
        const cached = indexFieldsCache[row.name as string];
        if (cached && cached.length > 0) {
          return (
            <span style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
              {cached.slice(0, 3).map((f) => {
                const b = TYPE_BADGE[f.type] ?? { bg: 'var(--bg-tertiary)', color: 'var(--text-muted)' };
                return (
                  <span key={f.name} title={`${f.name}: ${f.type}`} style={{ fontSize: 10, fontWeight: 600, padding: '1px 6px', borderRadius: 999, background: b.bg, color: b.color, border: `1px solid ${b.color}18`, fontFamily: 'var(--font-mono)' }}>
                    {f.name}
                  </span>
                );
              })}
              {cached.length > 3 && <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>+{cached.length - 3}</span>}
              {cached.length <= 3 && <span className="badge" style={{ fontFamily: 'var(--font-mono)', fontSize: 10 }}>{count} fields</span>}
            </span>
          );
        }
        return <span className="badge" style={{ fontFamily: 'var(--font-mono)', fontSize: 11 }}>{String(count)}</span>;
      },
    },
    {
      key: 'actions',
      header: '',
      width: '204px',
      render: (_: unknown, row: any) => {
        const isActive = selectedIndex === row.name;
        return (
          <div className="actions" onClick={(e) => e.stopPropagation()} style={{ gap: 6 }}>
            <button
              className={`btn btn-sm ${isActive ? '' : 'btn-primary'}`}
              onClick={() => {
                setSelectedIndex(row.name as string);
                setQueryResult(null);
                setQueryError(null);
              }}
              title={isActive ? 'Selected' : 'Open index'}
            >
              {isActive ? 'Selected' : 'Open'}
            </button>
            <button
              className="btn btn-sm"
              onClick={() => {
                setSelectedIndex(row.name as string);
                setQueryResult(null);
                setQueryError(null);
                setAddDocsError(null);
                setShowAddDocs(true);
              }}
            >
              Add Docs
            </button>
            <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(row.name as string)}>
              Delete
            </button>
          </div>
        );
      },
    },
  ];

  const totalDocs = indexes?.reduce((s, i) => s + i.document_count, 0) ?? 0;
  const totalSize = indexes ? indexes.reduce((s, i) => s + i.index_size_bytes, 0) : 0;
  const cleanedPreviewCount = newFields.filter((f) => f.name.trim()).length;
  const previewName = newIndexName.trim() || '—';

  return (
    <div>
      {/* Header */}
      <div className="page-header" style={{ marginBottom: 12 }}>
        <div className="flex justify-between items-center" style={{ gap: 12 }}>
          <div style={{ minWidth: 0 }}>
            <h1 style={{ margin: 0, fontSize: 22, letterSpacing: -0.02 }}>Search</h1>
            <p style={{ margin: '4px 0 0', color: 'var(--text-muted)', fontSize: 12.5, lineHeight: 1.5 }}>
              Full-text search — create an index (like a table) with fields, add JSON documents, then query with BM25.
            </p>
          </div>
          <button
            className="btn btn-primary"
            onClick={openCreateBlank}
            style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexShrink: 0 }}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden>
              <path d="M7 2.5v9M2.5 7h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
            Create Index
          </button>
        </div>
      </div>

      {toast && (
        <div className={`callout ${toast.type === 'error' ? 'error' : 'info'}`} style={{ marginBottom: 12 }}>
          {toast.message}
        </div>
      )}

      {/* Stats — gap 12 */}
      <div className="grid grid-cols-3" style={{ gap: 12, marginBottom: 12 }}>
        <MetricCard title="Indexes" value={indexes?.length ?? '-'} color="accent" loading={indexesLoading} />
        <MetricCard title="Total Docs" value={totalDocs.toLocaleString() ?? '-'} color="info" loading={indexesLoading} />
        <MetricCard title="Total Size" value={indexes ? formatBytes(totalSize) : '-'} color="success" loading={indexesLoading} />
      </div>

      {/* Indexes table */}
      <div className="card" style={{ marginBottom: 12, padding: 14 }}>
        <div className="flex justify-between items-center" style={{ marginBottom: 10 }}>
          <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>Indexes</div>
          <div className="flex gap-2" style={{ alignItems: 'center' }}>
            {indexes && indexes.length > 0 && selectedIndex && (
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>
                Selected <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--text-primary)', fontWeight: 600 }}>{selectedIndex}</span>
              </span>
            )}
            <button className="btn btn-sm" onClick={() => refetchIndexes()}>
              Refresh
            </button>
          </div>
        </div>
        <DataTable
          columns={indexColumns}
          data={(indexes || []) as unknown as Record<string, unknown>[]}
          loading={indexesLoading}
          onRowClick={(row) => {
            setSelectedIndex(row.name as string);
            setQueryResult(null);
            setQueryError(null);
          }}
          emptyMessage="No indexes yet — create your first index below"
        />
      </div>

      {/* Empty state — what is an index + templates */}
      {!indexesLoading && (!indexes || indexes.length === 0) && (
        <div className="card" style={{ padding: 14, marginBottom: 12, border: '1px solid var(--border)', background: 'var(--bg-primary)' }}>
          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-start', flexWrap: 'wrap' }}>
            <div style={{ flex: '1 1 320px', minWidth: 260 }}>
              <div style={{ fontSize: 13, fontWeight: 700, color: 'var(--text-primary)', marginBottom: 4 }}>What is an index?</div>
              <p style={{ margin: 0, fontSize: 12.5, lineHeight: 1.6, color: 'var(--text-secondary)' }}>
                An index is like a table — it holds JSON documents and defines the <strong style={{ color: 'var(--text-primary)' }}>fields</strong> you can search.
                Each field has a type that controls how it is stored and queried (BM25 full-text for <code style={{ fontSize: 11 }}>text</code>, exact matching for{' '}
                <code style={{ fontSize: 11 }}>keyword</code>, numeric filters for <code style={{ fontSize: 11 }}>integer/float</code>).
              </p>
              <div style={{ display: 'flex', gap: 8, marginTop: 12, flexWrap: 'wrap' }}>
                {[
                  { n: '1', title: 'Create index with fields', desc: 'Pick field names & types' },
                  { n: '2', title: 'Add documents', desc: 'JSON with id + fields' },
                  { n: '3', title: 'Search', desc: 'BM25 ranked results' },
                ].map((s) => (
                  <div
                    key={s.n}
                    style={{
                      flex: '1 1 120px',
                      display: 'flex',
                      gap: 8,
                      alignItems: 'flex-start',
                      background: 'var(--bg-tertiary)',
                      border: '1px solid var(--border)',
                      borderRadius: 8,
                      padding: '8px 10px',
                    }}
                  >
                    <span
                      style={{
                        width: 22,
                        height: 22,
                        borderRadius: 999,
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--border)',
                        display: 'grid',
                        placeItems: 'center',
                        fontSize: 11,
                        fontWeight: 700,
                        color: 'var(--text-primary)',
                        flexShrink: 0,
                      }}
                    >
                      {s.n}
                    </span>
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)', lineHeight: 1.2 }}>{s.title}</div>
                      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>{s.desc}</div>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div style={{ flex: '1 1 300px', minWidth: 280 }}>
              <div style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.5, color: 'var(--text-muted)', marginBottom: 8, textTransform: 'uppercase' }}>
                One-click templates
              </div>
              <div style={{ display: 'grid', gap: 8 }}>
                {TEMPLATES.map((t) => (
                  <button
                    key={t.id}
                    onClick={() => applyTemplate(t)}
                    style={{
                      textAlign: 'left',
                      background: 'var(--bg-tertiary)',
                      border: '1px solid var(--border)',
                      borderRadius: 8,
                      padding: '10px 12px',
                      cursor: 'pointer',
                      display: 'flex',
                      justifyContent: 'space-between',
                      gap: 10,
                      alignItems: 'center',
                    }}
                  >
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--text-primary)' }}>{t.label}</div>
                      <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2, fontFamily: 'var(--font-mono)', lineHeight: 1.4 }}>{t.hint}</div>
                    </div>
                    <span
                      style={{
                        fontSize: 11,
                        fontWeight: 600,
                        color: 'var(--accent)',
                        background: 'var(--bg-primary)',
                        border: '1px solid var(--border)',
                        padding: '4px 8px',
                        borderRadius: 999,
                        flexShrink: 0,
                      }}
                    >
                      Use →
                    </span>
                  </button>
                ))}
              </div>
              <div style={{ marginTop: 10, display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                <button className="btn btn-sm btn-primary" onClick={openCreateBlank}>
                  Create blank index
                </button>
                <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Templates pre-fill the Create dialog — just click Create.</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Quick templates bar when indexes exist but none selected */}
      {!indexesLoading && indexes && indexes.length > 0 && !selectedIndex && (
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center', marginBottom: 12 }}>
          <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: 0.3 }}>Quick start:</span>
          {TEMPLATES.map((t) => (
            <button key={t.id} className="btn btn-sm" onClick={() => applyTemplate(t)} title={t.hint}>
              + {t.label}
            </button>
          ))}
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>— instant template → Create</span>
        </div>
      )}

      {/* Selected index: details + search */}
      {selectedIndex && (
        <div className="grid grid-cols-2" style={{ gap: 12 }}>
          {/* Left: Index info + query */}
          <div className="card" style={{ padding: 14, display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div className="flex justify-between items-center" style={{ gap: 8 }}>
              <div style={{ minWidth: 0 }}>
                <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>Index</div>
                <div
                  style={{
                    fontFamily: 'var(--font-mono)',
                    fontSize: 13,
                    fontWeight: 700,
                    color: 'var(--text-primary)',
                    marginTop: 2,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                  title={selectedIndex}
                >
                  {selectedIndex}
                </div>
              </div>
              <span className="badge" title="Fields in this index" style={{ flexShrink: 0 }}>
                {selectedMeta ? `${selectedMeta.field_count} fields` : '—'}
              </span>
            </div>

            {selectedMeta && (
              <div
                style={{
                  display: 'flex',
                  gap: 6,
                  flexWrap: 'wrap',
                  fontSize: 11,
                  color: 'var(--text-muted)',
                  background: 'var(--bg-tertiary)',
                  border: '1px solid var(--border)',
                  borderRadius: 6,
                  padding: '6px 8px',
                }}
              >
                <span>
                  <strong style={{ color: 'var(--text-primary)', fontWeight: 600 }}>{selectedMeta.document_count.toLocaleString()}</strong> docs
                </span>
                <span style={{ opacity: 0.5 }}>·</span>
                <span>{formatBytes(selectedMeta.index_size_bytes)}</span>
                <span style={{ opacity: 0.5 }}>·</span>
                <span>{selectedMeta.field_count} fields</span>
              </div>
            )}

            {/* Field list with type badges */}
            <div>
              <div style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', letterSpacing: 0.3, marginBottom: 6 }}>Fields</div>
              {displayFields && displayFields.length > 0 ? (
                <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                  {displayFields.map((f) => {
                    const meta = TYPE_BADGE[f.type] ?? { bg: 'var(--bg-tertiary)', color: 'var(--text-muted)' };
                    return (
                      <span
                        key={f.name}
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: 6,
                          padding: '3px 8px',
                          borderRadius: 999,
                          background: 'var(--bg-primary)',
                          border: '1px solid var(--border)',
                          fontSize: 11,
                        }}
                      >
                        <span style={{ fontFamily: 'var(--font-mono)', fontWeight: 600, color: 'var(--text-primary)' }}>{f.name}</span>
                        <span
                          style={{
                            fontSize: 10,
                            fontWeight: 700,
                            padding: '1px 5px',
                            borderRadius: 999,
                            background: meta.bg,
                            color: meta.color,
                            border: `1px solid ${meta.color}18`,
                            textTransform: 'uppercase',
                            letterSpacing: 0.3,
                          }}
                        >
                          {f.type}
                        </span>
                      </span>
                    );
                  })}
                </div>
              ) : selectedMeta ? (
                <div style={{ fontSize: 11, color: 'var(--text-muted)', background: 'var(--bg-tertiary)', border: '1px dashed var(--border)', borderRadius: 6, padding: '8px 10px' }}>
                  {selectedMeta.field_count} field{selectedMeta.field_count === 1 ? '' : 's'} defined. Add documents and run a search to see field values here.
                </div>
              ) : (
                <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>—</div>
              )}
            </div>

            <div className="flex gap-2" style={{ flexWrap: 'wrap' }}>
              <button
                className="btn btn-sm btn-primary"
                onClick={() => {
                  setAddDocsError(null);
                  setShowAddDocs(true);
                }}
              >
                + Add Documents
              </button>
              <button className="btn btn-sm btn-danger" onClick={() => setDeleteIndexName(selectedIndex)}>
                Delete Index
              </button>
              <button
                className="btn btn-sm"
                onClick={() => {
                  setSelectedIndex(null);
                  setQueryResult(null);
                  setQuery('');
                  setQueryError(null);
                }}
              >
                Close
              </button>
            </div>

            <div className="form-group" style={{ marginBottom: 0 }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                Query
                <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, fontSize: 11, color: 'var(--text-muted)' }}>BM25 — Enter to search</span>
              </label>
              <div className="flex gap-2">
                <input
                  className="form-input"
                  style={{ flex: 1, fontSize: 13 }}
                  placeholder="Try: hello, title:hello, price:>100"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') handleSearch();
                  }}
                  aria-label="Search query"
                />
                <button className="btn btn-primary" onClick={handleSearch} disabled={queryLoading || !query.trim()} style={{ minWidth: 78 }}>
                  {queryLoading ? 'Searching…' : 'Search'}
                </button>
              </div>
              <div className="form-hint">
                Tip: plain word searches <code style={{ fontSize: 11 }}>text</code> fields · <code style={{ fontSize: 11 }}>field:value</code> filters <code style={{ fontSize: 11 }}>keyword/integer</code>
              </div>
            </div>

            {queryError && <div className="callout error" style={{ margin: 0 }}>{queryError}</div>}

            <div
              style={{
                fontSize: 11,
                color: 'var(--text-muted)',
                lineHeight: 1.5,
                borderTop: '1px dashed var(--border)',
                paddingTop: 8,
                marginTop: 2,
              }}
            >
              Documents are JSON with an <code style={{ fontSize: 11 }}>id</code> field. Use <strong style={{ color: 'var(--text-secondary)' }}>Add Documents</strong> to index JSON — scores are BM25 relevance.
            </div>
          </div>

          {/* Right: Results */}
          <div className="card" style={{ padding: 14, display: 'flex', flexDirection: 'column', minHeight: 260 }}>
            <div className="flex justify-between items-center" style={{ marginBottom: 8 }}>
              <div className="card-title" style={{ margin: 0, fontSize: 11, fontWeight: 700, letterSpacing: 0.5 }}>
                Results{' '}
                {queryResult ? (
                  <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)', fontSize: 11 }}>
                    · {queryResult.total} in {queryResult.execution_time_ms.toFixed(1)}ms
                  </span>
                ) : null}
              </div>
              {queryResult && queryResult.hits.length > 0 && <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{queryResult.hits.length} shown</span>}
            </div>

            {!queryResult ? (
              <div
                style={{
                  flex: 1,
                  display: 'grid',
                  placeItems: 'center',
                  padding: 20,
                  textAlign: 'center',
                  color: 'var(--text-muted)',
                  border: '1px dashed var(--border)',
                  borderRadius: 8,
                  background: 'var(--bg-tertiary)',
                }}
              >
                <div>
                  <div
                    style={{
                      width: 28,
                      height: 28,
                      borderRadius: 6,
                      background: 'var(--bg-hover)',
                      display: 'grid',
                      placeItems: 'center',
                      margin: '0 auto 8px',
                      color: 'var(--text-muted)',
                    }}
                  >
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
                      <circle cx="11" cy="11" r="7" stroke="currentColor" strokeWidth="1.6" />
                      <path d="M16 16l4 4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
                    </svg>
                  </div>
                  <div style={{ fontSize: 12.5, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 4 }}>Search uses BM25 — try a word from your documents</div>
                  <div style={{ fontSize: 11, lineHeight: 1.5 }}>
                    Try <code style={{ fontSize: 11 }}>hello</code>, <code style={{ fontSize: 11 }}>title:hello</code> or <code style={{ fontSize: 11 }}>price: &gt;100</code> · results sort by score
                  </div>
                </div>
              </div>
            ) : queryResult.hits.length === 0 ? (
              <div
                style={{
                  flex: 1,
                  display: 'grid',
                  placeItems: 'center',
                  padding: 20,
                  textAlign: 'center',
                  border: '1px solid var(--border)',
                  borderRadius: 8,
                  background: 'var(--bg-tertiary)',
                }}
              >
                <div>
                  <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-secondary)', marginBottom: 4 }}>No results for “{query}”</div>
                  <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 10, lineHeight: 1.5 }}>
                    Check spelling, try a broader term, or verify field types — <code style={{ fontSize: 11 }}>text</code> is full-text; <code style={{ fontSize: 11 }}>keyword</code> needs an exact match.
                  </div>
                  <button
                    className="btn btn-sm"
                    onClick={() => {
                      setQuery('');
                      setQueryResult(null);
                    }}
                  >
                    Clear query
                  </button>
                </div>
              </div>
            ) : (
              <div className="data-table-wrapper" style={{ flex: 1 }}>
                <table className="data-table">
                  <thead>
                    <tr>
                      <th style={{ width: 72 }}>Score</th>
                      <th style={{ width: 140 }}>ID</th>
                      {queryResult.hits[0].fields &&
                        Object.keys(queryResult.hits[0].fields).map((k) => (
                          <th key={k} style={{ minWidth: 120 }}>
                            {k}
                          </th>
                        ))}
                    </tr>
                  </thead>
                  <tbody>
                    {queryResult.hits.map((hit) => {
                      const tone = scoreTone(hit.score, maxScore);
                      const fieldEntries = hit.fields ? Object.entries(hit.fields) : [];
                      return (
                        <tr key={hit.id}>
                          <td>
                            <span
                              title={`Score ${hit.score.toFixed(4)} — ${tone.label}`}
                              style={{
                                display: 'inline-flex',
                                alignItems: 'center',
                                justifyContent: 'center',
                                minWidth: 52,
                                padding: '2px 7px',
                                borderRadius: 999,
                                fontFamily: 'var(--font-mono)',
                                fontSize: 11,
                                fontWeight: 700,
                                background: tone.bg,
                                color: tone.color,
                                border: `1px solid ${tone.color}22`,
                                fontVariantNumeric: 'tabular-nums',
                              }}
                            >
                              {hit.score.toFixed(2)}
                            </span>
                          </td>
                          <td
                            style={{
                              fontFamily: 'var(--font-mono)',
                              fontSize: 11,
                              color: 'var(--text-primary)',
                              maxWidth: 160,
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}
                            title={hit.id}
                          >
                            {hit.id}
                          </td>
                          {fieldEntries.map(([k, v]) => {
                            const s = String(v ?? '');
                            const snippet = s.length > 96 ? s.slice(0, 96) + '…' : s;
                            return (
                              <td key={k} title={s} style={{ fontSize: 12, lineHeight: 1.5, maxWidth: 260 }}>
                                <span
                                  style={{
                                    display: '-webkit-box',
                                    WebkitLineClamp: 2 as unknown as number,
                                    WebkitBoxOrient: 'vertical',
                                    overflow: 'hidden',
                                    wordBreak: 'break-word',
                                  }}
                                >
                                  {snippet}
                                </span>
                              </td>
                            );
                          })}
                          {fieldEntries.length === 0 && <td style={{ color: 'var(--text-muted)', fontSize: 11 }}>-</td>}
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      )}

      {/* Create Index */}
      <Modal
        isOpen={showCreate}
        onClose={() => setShowCreate(false)}
        title="Create Index"
        size="md"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'space-between', alignItems: 'center', width: '100%' }}>
            <span style={{ fontSize: 11, color: 'var(--text-muted)', textAlign: 'left' }}>
              This will create index <strong style={{ color: 'var(--text-primary)', fontFamily: 'var(--font-mono)' }}>'{previewName}'</strong> with {cleanedPreviewCount} field{cleanedPreviewCount === 1 ? '' : 's'} — you can add documents right after.
            </span>
            <span style={{ display: 'inline-flex', gap: 8, flexShrink: 0 }}>
              <button className="btn" onClick={() => setShowCreate(false)}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={handleCreateIndex} disabled={createLoading}>
                {createLoading ? 'Creating…' : 'Create'}
              </button>
            </span>
          </div>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div style={{ background: 'var(--bg-tertiary)', border: '1px solid var(--border)', borderRadius: 8, padding: '10px 12px' }}>
            <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 4 }}>What is an index?</div>
            <p style={{ margin: 0, fontSize: 11.5, lineHeight: 1.6, color: 'var(--text-secondary)' }}>
              Like a table: a named collection with a fixed set of <strong style={{ color: 'var(--text-primary)' }}>fields</strong>. Add JSON documents whose keys match those
              fields, then search them with BM25 ranking.
            </p>
          </div>

          <div className="form-group" style={{ marginBottom: 0 }}>
            <label>
              Index name <span style={{ color: 'var(--danger)' }}>*</span>
            </label>
            <input
              className="form-input"
              value={newIndexName}
              onChange={(e) => setNewIndexName(e.target.value)}
              placeholder="products"
              style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }}
            />
            <div className="form-hint">Lowercase letters, digits, <code>_</code> and <code>-</code> only. Example: <code>products</code>, <code>blog</code>, <code>logs</code></div>
          </div>

          <div className="form-group" style={{ marginBottom: 0 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 6 }}>
              <label style={{ marginBottom: 0 }}>
                Fields <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)' }}>— at least 1</span>
              </label>
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{cleanedPreviewCount} field{cleanedPreviewCount === 1 ? '' : 's'}</span>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {newFields.map((f, idx) => {
                const meta = FIELD_TYPES.find((x) => x.value === f.type);
                return (
                  <div
                    key={idx}
                    style={{
                      border: '1px solid var(--border)',
                      borderRadius: 8,
                      padding: 8,
                      background: 'var(--bg-tertiary)',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 6,
                    }}
                  >
                    <div className="flex gap-2" style={{ alignItems: 'center' }}>
                      <input
                        className="form-input"
                        value={f.name}
                        onChange={(e) => {
                          const c = [...newFields];
                          c[idx].name = e.target.value;
                          setNewFields(c);
                        }}
                        placeholder="field name (e.g. title)"
                        style={{ flex: 1, fontFamily: 'var(--font-mono)', fontSize: 12 }}
                      />
                      <select
                        className="form-select"
                        value={f.type}
                        onChange={(e) => {
                          const c = [...newFields];
                          c[idx].type = e.target.value;
                          setNewFields(c);
                        }}
                        style={{ flex: 1, maxWidth: 320, fontSize: 12 }}
                      >
                        {FIELD_TYPES.map((t) => (
                          <option key={t.value} value={t.value}>
                            {t.label}
                          </option>
                        ))}
                      </select>
                      <button
                        className="btn btn-sm btn-danger"
                        onClick={() => setNewFields(newFields.filter((_, i) => i !== idx))}
                        aria-label="Remove field"
                        title="Remove field"
                        disabled={newFields.length <= 1}
                        style={{ opacity: newFields.length <= 1 ? 0.4 : 1 }}
                      >
                        ×
                      </button>
                    </div>
                    <div style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.4 }}>
                      <span
                        title={meta?.help}
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          width: 14,
                          height: 14,
                          borderRadius: 999,
                          background: 'var(--bg-primary)',
                          border: '1px solid var(--border)',
                          fontSize: 9,
                          flexShrink: 0,
                        }}
                      >
                        ?
                      </span>
                      <span>{meta?.help}</span>
                    </div>
                  </div>
                );
              })}
            </div>

            <div style={{ display: 'flex', gap: 8, marginTop: 8, alignItems: 'center', flexWrap: 'wrap' }}>
              <button className="btn btn-sm" onClick={() => setNewFields([...newFields, { name: '', type: 'text' }])}>
                + Add Field
              </button>
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Text = searchable; Keyword = exact; numbers/boolean for filters.</span>
            </div>
          </div>

          {createError && <div className="callout error" style={{ margin: 0 }}>{createError}</div>}
        </div>
      </Modal>

      {/* Add Documents */}
      <Modal
        isOpen={showAddDocs}
        onClose={() => setShowAddDocs(false)}
        title={`Add Documents — ${selectedIndex ?? ''}`}
        size="lg"
        footer={
          <div className="flex gap-2" style={{ justifyContent: 'flex-end', alignItems: 'center' }}>
            <span style={{ fontSize: 11, color: docsJsonError ? 'var(--danger)' : 'var(--text-muted)', marginRight: 'auto' }}>
              {docsJsonError
                ? `⚠ ${docsJsonError}`
                : `✓ Valid JSON — ${(() => {
                    try {
                      const v = JSON.parse(docsJson);
                      return Array.isArray(v) ? v.length : 1;
                    } catch {
                      return '?';
                    }
                  })()} doc(s)`}
            </span>
            <button className="btn" onClick={() => setShowAddDocs(false)}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleAddDocs} disabled={!!docsJsonError || addDocsLoading}>
              {addDocsLoading ? 'Adding…' : 'Add'}
            </button>
          </div>
        }
      >
        <div className="form-group">
          <label>
            JSON Array <span style={{ fontWeight: 400, textTransform: 'none', letterSpacing: 0, color: 'var(--text-muted)' }}>— each object needs an `id`</span>
          </label>
          <textarea
            className="form-input json-editor"
            value={docsJson}
            onChange={(e) => setDocsJson(e.target.value)}
            rows={10}
            spellCheck={false}
            style={{ fontFamily: 'var(--font-mono)', fontSize: 12, lineHeight: 1.6 }}
          />
          <div className="form-hint">
            Single object or array. Keys must match fields of the index. Example:{' '}
            <code style={{ fontSize: 11 }}>[{`{"id":"1","title":"Hello","body":"..."}`}]</code>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 8 }}>
          <button
            className="btn btn-sm"
            onClick={() => setDocsJson('[\n  {"id": "1", "title": "Hello world", "body": "This is a test document"}\n]')}
          >
            Example
          </button>
          <button
            className="btn btn-sm"
            onClick={() => {
              try {
                setDocsJson(JSON.stringify(JSON.parse(docsJson), null, 2));
              } catch {}
            }}
          >
            Format JSON
          </button>
          <span style={{ fontSize: 11, color: 'var(--text-muted)', alignSelf: 'center' }}>Bulk add — 1 to 1000 docs per request.</span>
        </div>
        {addDocsError && <div className="callout error" style={{ marginTop: 10 }}>{addDocsError}</div>}
      </Modal>

      <ConfirmDialog
        isOpen={!!deleteIndexName}
        onClose={() => setDeleteIndexName(null)}
        onConfirm={handleDeleteIndex}
        title="Delete Index"
        message={`Delete index "${deleteIndexName}" and all its documents? This cannot be undone.`}
        confirmText="Delete"
        variant="danger"
      />
    </div>
  );
}
