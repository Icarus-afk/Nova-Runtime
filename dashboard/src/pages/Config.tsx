import { useState, useMemo } from 'react';
import { useApi } from '../hooks/useApi';
import { api } from '../api/client';
import type { ConfigEntry } from '../types';
import { CheckIcon, AlertIcon } from '../components/Icons';

function formatValue(value: unknown): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

export default function ConfigPage() {
  const [search, setSearch] = useState('');
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  const { data: entries, loading } = useApi<ConfigEntry[]>(() => api.getConfig(), []);

  const sections = useMemo(() => {
    if (!entries) return {};
    const groups: Record<string, ConfigEntry[]> = {};
    for (const entry of entries) {
      const section = entry.key.split('.')[0] || 'general';
      if (!groups[section]) groups[section] = [];
      groups[section].push(entry);
    }
    return groups;
  }, [entries]);

  const filteredEntries = useMemo(() => {
    if (!search.trim() || !entries) return entries;
    const q = search.toLowerCase();
    return entries.filter(
      (e) => e.key.toLowerCase().includes(q) || e.description.toLowerCase().includes(q)
    );
  }, [entries, search]);

  const selectedEntry = selectedKey ? entries?.find(e => e.key === selectedKey) : null;

  return (
    <div>
      <div className="page-header">
        <h1>Configuration</h1>
        <p>View runtime configuration (read-only)</p>
      </div>

      <div className="callout info mb-4">
        Configuration is read-only in this view. Use the REST API or CLI to modify configuration values.
      </div>

      <div className="flex gap-4">
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="flex items-center gap-2 mb-4">
            <input
              className="form-input"
              style={{ width: 300 }}
              placeholder="Search config keys..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <span className="text-sm text-muted">{entries?.length ?? 0} entries</span>
          </div>

          {loading ? (
            <div className="loading-spinner">Loading configuration</div>
          ) : !entries || entries.length === 0 ? (
            <div className="card">
              <div className="text-muted" style={{ textAlign: 'center', padding: 40 }}>No configuration entries</div>
            </div>
          ) : (
            <div>
              {search.trim() ? (
                <div className="card">
                  <div className="data-table-wrapper">
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th>Key</th>
                          <th>Value</th>
                          <th>Type</th>
                          <th>Mutable</th>
                        </tr>
                      </thead>
                      <tbody>
                        {(filteredEntries || []).map((entry) => (
                          <tr key={entry.key} onClick={() => setSelectedKey(entry.key)} style={{ cursor: 'pointer' }}>
                            <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>{entry.key}</td>
                            <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, maxWidth: 300 }} className="truncate">
                              {formatValue(entry.value)}
                            </td>
                            <td>{entry.type}</td>
                            <td>{entry.mutable ? '✓' : '-'}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              ) : (
                Object.entries(sections).map(([section, sectionEntries]) => (
                  <div key={section} className="card mb-4">
                    <div className="card-title">{section}</div>
                    <div className="data-table-wrapper">
                      <table className="data-table">
                        <thead>
                          <tr>
                            <th>Key</th>
                            <th>Value</th>
                            <th>Type</th>
                            <th>Mutable</th>
                            <th>Requires Restart</th>
                          </tr>
                        </thead>
                        <tbody>
                          {sectionEntries.map((entry) => (
                            <tr key={entry.key} onClick={() => setSelectedKey(entry.key)} style={{ cursor: 'pointer' }}>
                              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12 }}>
                                {entry.key.replace(section + '.', '')}
                              </td>
                              <td style={{ fontFamily: 'var(--font-mono)', fontSize: 12, maxWidth: 300 }} className="truncate">
                                {formatValue(entry.value)}
                              </td>
                              <td>{entry.type}</td>
                              <td>{entry.mutable ? <CheckIcon size={14} style={{ color: 'var(--success)' }} /> : '-'}</td>
                              <td>{entry.requires_restart ? <AlertIcon size={14} style={{ color: 'var(--warning)' }} /> : '-'}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  </div>
                ))
              )}
            </div>
          )}
        </div>

        {selectedEntry && (
          <div className="detail-panel" style={{ width: 320, flexShrink: 0 }}>
            <div className="flex items-center justify-between mb-4">
              <div className="card-title" style={{ margin: 0 }}>Details</div>
              <button className="btn btn-sm" onClick={() => setSelectedKey(null)}>Close</button>
            </div>
            <div className="detail-row">
              <span className="detail-label">Key</span>
              <span className="detail-value" style={{ fontSize: 11 }}>{selectedEntry.key}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Value</span>
              <span className="detail-value">{formatValue(selectedEntry.value)}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Type</span>
              <span className="detail-value">{selectedEntry.type}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Mutable</span>
              <span className="detail-value">{selectedEntry.mutable ? 'Yes' : 'No'}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Requires Restart</span>
              <span className="detail-value">{selectedEntry.requires_restart ? 'Yes' : 'No'}</span>
            </div>
            <div className="detail-row">
              <span className="detail-label">Default</span>
              <span className="detail-value">{formatValue(selectedEntry.default_value)}</span>
            </div>
            <div style={{ marginTop: 8, fontSize: 12, color: 'var(--text-secondary)' }}>
              {selectedEntry.description}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
