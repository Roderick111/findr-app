import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LicenseState, LicenseStatus } from "../types";

interface Props {
  children: React.ReactNode;
}

export function LicenseGate({ children }: Props) {
  const [status, setStatus] = useState<LicenseStatus | null>(null);
  const [trialDays, setTrialDays] = useState(0);
  const [keyInput, setKeyInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [activating, setActivating] = useState(false);

  const checkLicense = useCallback(async () => {
    try {
      const state = await invoke<LicenseState>("get_license_state");
      setStatus(state.status);
      if (state.status === "trial") {
        const days = await invoke<number>("get_trial_days_remaining");
        setTrialDays(days);
      }
    } catch (e) {
      console.error("License check failed:", e);
      setStatus("unknown");
      setError("Unable to verify license. Please restart the app.");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    checkLicense();
  }, [checkLicense]);

  const handleActivate = async () => {
    const trimmed = keyInput.trim();
    if (!trimmed) return;
    setActivating(true);
    setError(null);
    try {
      const state = await invoke<LicenseState>("activate_license", {
        key: trimmed,
      });
      setStatus(state.status);
    } catch (e) {
      setError(String(e));
    } finally {
      setActivating(false);
    }
  };

  const handleStartTrial = async () => {
    try {
      const state = await invoke<LicenseState>("start_trial");
      setStatus(state.status);
      const days = await invoke<number>("get_trial_days_remaining");
      setTrialDays(days);
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center overlay-root">
        <span className="text-sm" style={{ color: "var(--text-tertiary)" }}>Loading...</span>
      </div>
    );
  }

  if (status === "active") return <>{children}</>;

  if (status === "trial") {
    return (
      <>
        <TrialBanner days={trialDays} onActivate={() => setStatus(null)} />
        {children}
      </>
    );
  }

  if (status === "unknown") {
    return (
      <div className="h-screen flex flex-col items-center justify-center gap-4 overlay-root">
        <span className="text-sm" style={{ color: "var(--text-tertiary)" }}>
          Checking license...
        </span>
        {error && (
          <p className="text-xs text-center max-w-[300px]" style={{ color: "var(--error)" }}>{error}</p>
        )}
        <button
          onClick={() => {
            setLoading(true);
            setError(null);
            checkLicense();
          }}
          className="text-xs px-3 py-1.5 rounded transition-colors"
          style={{
            background: "var(--bg-hover)",
            border: "1px solid var(--border)",
            color: "var(--text-secondary)",
          }}
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="h-screen flex items-center justify-center overlay-root">
      <div className="w-[380px] flex flex-col gap-5 p-8">
        <div className="text-center">
          <h1 className="text-2xl font-semibold mb-1" style={{ color: "var(--text-primary)" }}>
            findr
          </h1>
          <p className="text-sm" style={{ color: "var(--text-secondary)" }}>
            {status === "trial_expired"
              ? "Your trial has expired"
              : "Activate your license to get started"}
          </p>
        </div>

        <div className="flex flex-col gap-3">
          <input
            type="text"
            value={keyInput}
            onChange={(e) => setKeyInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleActivate()}
            placeholder="Enter license key"
            className="w-full px-3 py-2 rounded-lg text-sm outline-none"
            style={{
              background: "var(--bg-hover)",
              border: "1px solid var(--border)",
              color: "var(--text-primary)",
            }}
            autoFocus
          />
          <button
            onClick={handleActivate}
            disabled={activating || !keyInput.trim()}
            className="w-full py-2 rounded-lg text-sm font-medium transition-colors disabled:opacity-40"
            style={{ background: "var(--accent)", color: "var(--accent-text)" }}
          >
            {activating ? "Activating..." : "Activate License"}
          </button>
        </div>

        {error && (
          <p className="text-xs text-center" style={{ color: "var(--error)" }}>{error}</p>
        )}

        {status !== "trial_expired" && (
          <button
            onClick={handleStartTrial}
            className="text-xs transition-colors"
            style={{ color: "var(--text-tertiary)" }}
          >
            Start 14-day free trial
          </button>
        )}

        <a
          href="https://polar.sh/findr"
          target="_blank"
          rel="noreferrer"
          className="text-xs text-center transition-colors"
          style={{ color: "var(--accent)" }}
        >
          Purchase a license
        </a>
      </div>
    </div>
  );
}

function TrialBanner({
  days,
  onActivate,
}: {
  days: number;
  onActivate: () => void;
}) {
  const [dismissed, setDismissed] = useState(false);
  if (dismissed) return null;

  return (
    <div
      className="absolute top-0 inset-x-0 z-40 flex items-center justify-between px-4 py-1.5 text-xs"
      style={{
        background: "var(--warning)",
        color: "var(--bg-secondary)",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <span>
        Trial: {days} day{days !== 1 ? "s" : ""} remaining
      </span>
      <div className="flex gap-3">
        <button onClick={onActivate} className="font-medium opacity-90 hover:opacity-100">
          Activate
        </button>
        <button onClick={() => setDismissed(true)} className="opacity-70 hover:opacity-100">
          Dismiss
        </button>
      </div>
    </div>
  );
}
