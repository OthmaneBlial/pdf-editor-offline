import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * Error Boundary component to catch and display React errors gracefully.
 * Prevents the entire app from crashing when a component throws an error.
 */
class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    // Log error to console (in production, send to error tracking service)
    console.error('ErrorBoundary caught an error:', error, errorInfo);
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: null });
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="min-h-screen bg-[var(--bg-app)] flex items-center justify-center p-4">
          <div className="max-w-md w-full bg-[var(--card-bg)] rounded-2xl shadow-2xl border-2 border-[var(--accent-secondary)] p-8 text-center">
            {/* Icon with Neo-Brutalist styling */}
            <div className="w-16 h-16 mx-auto mb-6 bg-[var(--accent-secondary)]/10 rounded-xl flex items-center justify-center border-2 border-[var(--accent-secondary)]">
              <AlertTriangle className="w-8 h-8 text-[var(--accent-secondary)]" />
            </div>

            {/* Title */}
            <h2 className="text-xl font-display font-bold text-[var(--text-primary)] mb-2">
              Something went wrong
            </h2>

            {/* Description */}
            <p className="text-[var(--text-secondary)] mb-6 font-body text-sm">
              An unexpected error occurred. Please try refreshing the page or click the button below to retry.
            </p>

            {/* Error details (collapsible) */}
            {this.state.error && (
              <details className="mb-6 text-left">
                <summary className="text-sm text-[var(--color-primary-600)] cursor-pointer hover:text-[var(--color-primary-700)] font-medium">
                  Error details
                </summary>
                <pre className="mt-2 p-3 bg-[var(--bg-primary)] rounded-lg text-xs text-[var(--accent-secondary)] overflow-auto max-h-32 border border-[var(--border-subtle)] font-mono">
                  {this.state.error.message}
                </pre>
              </details>
            )}

            {/* Retry button */}
            <button
              onClick={this.handleRetry}
              className="inline-flex items-center gap-2 px-6 py-3 bg-[var(--accent-primary)] hover:bg-[var(--accent-primary-dim)] text-[var(--neutral-950)] rounded-xl font-display font-semibold uppercase tracking-wide shadow-lg shadow-[var(--accent-primary-glow)] hover:shadow-xl hover:shadow-[var(--accent-primary-glow)] transition-all duration-200 hover:-translate-y-0.5"
            >
              <RefreshCw className="w-4 h-4" />
              Try Again
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

export default ErrorBoundary;
