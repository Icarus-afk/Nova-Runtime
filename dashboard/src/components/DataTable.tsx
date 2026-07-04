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
        <div className="loading-spinner">Loading</div>
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
                <td colSpan={columns.length} className="empty-message">
                  {emptyMessage}
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
