import { Component, type ReactNode, type ErrorInfo } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

/**
 * Catches uncaught render errors and shows a fallback UI with a reload button.
 *
 * Usage: wrap top-level components in main.tsx:
 *   <ErrorBoundary>
 *     <LicenseGate><App /></LicenseGate>
 *   </ErrorBoundary>
 *
 *   <ErrorBoundary>
 *     <Settings />
 *   </ErrorBoundary>
 */
export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("ErrorBoundary caught:", error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div
          className="h-screen flex flex-col items-center justify-center gap-4 overlay-root"
          style={{ color: "var(--text-primary)" }}
        >
          <h1 className="text-lg font-semibold">Something went wrong</h1>
          <p
            className="text-sm max-w-[400px] text-center"
            style={{ color: "var(--text-secondary)" }}
          >
            {this.state.error?.message ?? "An unexpected error occurred."}
          </p>
          <button
            onClick={() => window.location.reload()}
            className="px-4 py-2 rounded-lg text-sm font-medium transition-colors"
            style={{
              background: "var(--accent)",
              color: "var(--accent-text)",
            }}
          >
            Reload
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
