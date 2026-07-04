import type { MetricCardData } from '../types';

interface MetricCardProps extends MetricCardData {
  color?: 'accent' | 'success' | 'warning' | 'danger' | 'info';
}

export default function MetricCard({ title, value, unit, change, changeDirection, color = 'accent', loading }: MetricCardProps) {
  if (loading) {
    return (
      <div className={`card ${color}`}>
        <div className="card-title">{title}</div>
        <div className="loading-skeleton" style={{ height: 32, width: '60%' }} />
        <div className="loading-skeleton" style={{ height: 12, width: '40%', marginTop: 8 }} />
      </div>
    );
  }

  return (
    <div className={`card ${color}`}>
      <div className="card-title">{title}</div>
      <div className="card-value">
        {value}
        {unit && <span className="card-unit">{unit}</span>}
      </div>
      {change !== undefined && (
        <div className={`card-change ${changeDirection === 'up' ? 'up' : 'down'}`}>
          {changeDirection === 'up' ? '↑' : '↓'} {Math.abs(change).toFixed(1)}%
        </div>
      )}
    </div>
  );
}
