import type { Column, PaginationInfo } from '../types';

interface DataTableProps<T> {
  columns: Column<T>[];
  data: T[];
  loading: boolean;
  pagination?: PaginationInfo;
  onPageChange?: (page: number) => void;
  onRowClick?: (row: T) => void;
  emptyMessage?: string;
  getRowId?: (row: T) => string;
}

export default function DataTable<T extends Record<string, unknown>>({
  columns,
  data,
  loading,
  pagination,
  onPageChange,
  onRowClick,
  emptyMessage = 'No data',
  getRowId,
}: DataTableProps<T>) {
  const renderPagination = () => {
    if (!pagination || pagination.total_pages <= 1) return null;
    const pages: number[] = [];
    const start = Math.max(1, pagination.page - 2);
    const end = Math.min(pagination.total_pages, pagination.page + 2);
    for (let i = start; i <= end; i++) pages.push(i);

    return (
      <div className="pagination">
        <button
          disabled={pagination.page <= 1}
          onClick={() => onPageChange?.(pagination.page - 1)}
        >
          Prev
        </button>
        {pages.map((p) => (
          <button
            key={p}
            className={p === pagination.page ? 'active' : ''}
            onClick={() => onPageChange?.(p)}
          >
            {p}
          </button>
        ))}
        <button
          disabled={pagination.page >= pagination.total_pages}
          onClick={() => onPageChange?.(pagination.page + 1)}
        >
          Next
        </button>
        <span>{pagination.total} total</span>
      </div>
    );
  };

  if (loading) {
    return (
      <div className="data-table-wrapper">
        <div style={{ padding: 32, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 12 }}>
          <div className="loading-spinner" style={{ padding: 0 }} />
          <span style={{ fontSize: 12, color: 'var(--text-muted)' }}>Loading…</span>
        </div>
      </div>
    );
  }

  return (
    <div>
      <div className="data-table-wrapper">
        <table className="data-table">
          <thead>
            <tr>
              {columns.map((col) => (
                <th key={col.key} style={{ width: col.width, textAlign: col.align || 'left' }}>
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 ? (
              <tr>
                <td colSpan={columns.length} style={{ padding: 0 }}>
                  <div className="empty-cta">
                    <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--text-primary)', marginBottom: 4 }}>{emptyMessage}</div>
                    <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Try adjusting filters or create a new entry</div>
                  </div>
                </td>
              </tr>
            ) : (
              data.map((row, i) => (
                <tr
                  key={getRowId?.(row) || i}
                  onClick={() => onRowClick?.(row)}
                  style={{ cursor: onRowClick ? 'pointer' : undefined }}
                >
                  {columns.map((col) => (
                    <td key={col.key} style={{ textAlign: col.align || 'left' }}>
                      {col.render ? col.render(row[col.key], row) : String(row[col.key] ?? '')}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
      {renderPagination()}
    </div>
  );
}
