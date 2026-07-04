import { Component } from 'react';

interface Props {
  children: React.ReactNode;
  pageName?: string;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error(`[${this.props.pageName || 'Page'}]`, error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="card" style={{ margin: 24, padding: 24 }}>
          <h2 style={{ color: 'var(--danger)', marginBottom: 8 }}>Something went wrong</h2>
          <p className="text-muted" style={{ marginBottom: 12 }}>
            {this.props.pageName || 'This page'} encountered an error.
          </p>
          <pre className="value-viewer" style={{ fontSize: 12, maxHeight: 200, overflow: 'auto' }}>
            {this.state.error?.message}
          </pre>
          <button className="btn btn-sm" style={{ marginTop: 12 }} onClick={() => { this.setState({ hasError: false, error: null }); window.location.reload(); }}>
            Reload Page
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
