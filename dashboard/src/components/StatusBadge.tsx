import type { HealthStatus } from '../types';

interface StatusBadgeProps {
  status: HealthStatus;
  label?: string;
}

export default function StatusBadge({ status, label }: StatusBadgeProps) {
  return (
    <span className={`status-badge ${status}`}>
      <span className={`status-dot ${status}`} />
      {label || status}
    </span>
  );
}
