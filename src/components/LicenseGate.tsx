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
    } catch {
      setStatus("unknown");
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
        <span className="text-neutral-500 text-sm">Loading...</span>
      </div>
    );
  }

  if (status === "active" || status === "unknown") return <>{children}</>;

  if (status === "trial") {
    return (
      <>
        <TrialBanner days={trialDays} onActivate={() => setStatus("unknown")} />
        {children}
      </>
    );
  }

  return (
    <div className="h-screen flex items-center justify-center overlay-root">
      <div className="w-[380px] flex flex-col gap-5 p-8">
        <div className="text-center">
          <h1 className="text-2xl font-semibold text-neutral-100 mb-1">
            findr
          </h1>
          <p className="text-sm text-neutral-400">
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
            className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-neutral-200 placeholder-neutral-600 outline-none focus:border-blue-500/50"
            autoFocus
          />
          <button
            onClick={handleActivate}
            disabled={activating || !keyInput.trim()}
            className="w-full py-2 bg-blue-600 hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-500 rounded-lg text-sm font-medium transition-colors"
          >
            {activating ? "Activating..." : "Activate License"}
          </button>
        </div>

        {error && (
          <p className="text-xs text-red-400 text-center">{error}</p>
        )}

        {status !== "trial_expired" && (
          <button
            onClick={handleStartTrial}
            className="text-xs text-neutral-500 hover:text-neutral-300 transition-colors"
          >
            Start 14-day free trial
          </button>
        )}

        <a
          href="https://polar.sh/findr"
          target="_blank"
          rel="noreferrer"
          className="text-xs text-blue-400 hover:text-blue-300 text-center transition-colors"
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
    <div className="absolute top-0 inset-x-0 z-40 flex items-center justify-between px-4 py-1.5 bg-amber-900/80 border-b border-amber-700/50 text-xs">
      <span className="text-amber-200">
        Trial: {days} day{days !== 1 ? "s" : ""} remaining
      </span>
      <div className="flex gap-3">
        <button
          onClick={onActivate}
          className="text-amber-100 hover:text-white font-medium"
        >
          Activate
        </button>
        <button
          onClick={() => setDismissed(true)}
          className="text-amber-400 hover:text-amber-200"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
