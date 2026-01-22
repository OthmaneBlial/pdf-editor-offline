import React, { Component, ErrorInfo, ReactNode } from 'react';
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
                <div className="min-h-screen bg-[var(--bg-primary)] flex items-center justify-center p-4">
                    <div className="max-w-md w-full bg-[var(--card-bg)] rounded-2xl shadow-2xl border border-[var(--border-color)] p-8 text-center">
                        <div className="w-16 h-16 mx-auto mb-6 bg-red-100 rounded-full flex items-center justify-center">
                            <AlertTriangle className="w-8 h-8 text-red-600" />
                        </div>

                        <h2 className="text-xl font-bold text-[var(--text-primary)] mb-2">
                            Something went wrong
                        </h2>

                        <p className="text-[var(--text-secondary)] mb-6">
                            An unexpected error occurred. Please try refreshing the page or click the button below to retry.
                        </p>

                        {this.state.error && (
                            <details className="mb-6 text-left">
                                <summary className="text-sm text-[var(--text-secondary)] cursor-pointer hover:text-[var(--color-primary-600)]">
                                    Error details
                                </summary>
                                <pre className="mt-2 p-3 bg-[var(--bg-primary)] rounded-lg text-xs text-red-600 overflow-auto max-h-32">
                                    {this.state.error.message}
                                </pre>
                            </details>
                        )}

                        <button
                            onClick={this.handleRetry}
                            className="inline-flex items-center gap-2 px-6 py-3 bg-gradient-to-r from-[var(--color-primary-600)] to-[var(--color-primary-700)] text-white rounded-xl font-medium hover:shadow-lg hover:shadow-cyan-500/30 transition-all duration-300"
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
