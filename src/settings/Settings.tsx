import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import {
  FolderOpen,
  HardDrive,
  Keyboard,
  Power,
  RefreshCw,
  Shield,
  Info,
  Plus,
  X,
  Check,
  Brain,
  Loader2,
  Sun,
  Moon,
  Monitor,
} from "lucide-react";
import { useTheme } from "../hooks/useTheme";
import type { DoctorReport, LicenseState, ThemePreference } from "../types";

export function Settings() {
  const [report, setReport] = useState<DoctorReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [syncStatus, setSyncStatus] = useState<string | null>(null);

  const loadReport = useCallback(async () => {
    try {
      const r = await invoke<DoctorReport>("get_doctor_report");
      setReport(r);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval> | null = null;
    let consecutiveFailures = 0;
    const MAX_FAILURES = 3;
    const POLL_INTERVAL = 30_000;

    const poll = async () => {
      try {
        const r = await invoke<DoctorReport>("get_doctor_report");
        setReport(r);
        setError(null);
        consecutiveFailures = 0;
      } catch (e) {
        setError(String(e));
        consecutiveFailures++;
        if (consecutiveFailures >= MAX_FAILURES && intervalId) {
          clearInterval(intervalId);
          intervalId = null;
        }
      } finally {
        setLoading(false);
      }
    };

    const startPolling = () => {
      if (intervalId) return;
      consecutiveFailures = 0;
      intervalId = setInterval(poll, POLL_INTERVAL);
    };

    const stopPolling = () => {
      if (intervalId) {
        clearInterval(intervalId);
        intervalId = null;
      }
    };

    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        poll();
        startPolling();
      } else {
        stopPolling();
      }
    };

    // Initial load + start polling only if visible
    poll();
    if (document.visibilityState === "visible") {
      startPolling();
    }

    document.addEventListener("visibilitychange", handleVisibility);
    return () => {
      stopPolling();
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("index-sync", (event) => {
      setSyncStatus(event.payload);
      if (event.payload === "complete") {
        setTimeout(() => setSyncStatus(null), 3000);
      }
    });
    return () => {
      unlisten.then((u) => u());
    };
  }, []);

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center settings-root" style={{ color: "var(--text-primary)" }}>
        <Loader2 size={20} className="animate-spin" style={{ color: "var(--text-tertiary)" }} />
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col settings-root" style={{ color: "var(--text-primary)" }}>
      <header className="px-6 py-4 shrink-0" style={{ borderBottom: "1px solid var(--border)" }}>
        <h1 className="text-lg font-semibold">Settings</h1>
      </header>
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-8">
        {error && (
          <div className="text-sm px-3 py-2 rounded" style={{ color: "var(--error)", background: "var(--bg-tertiary)" }}>
            {error}
          </div>
        )}
        <ScanPathsSection report={report} onRefresh={loadReport} />
        <ThemeSection />
        <SearchHotkeySection />
        <LaunchAtLoginSection />
        <SemanticSearchSection />
        <IndexStatusSection report={report} syncStatus={syncStatus} />
        <ReindexSection onRefresh={loadReport} />
        <LicenseSection />
        <AboutSection report={report} />
      </div>
    </div>
  );
}

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="flex items-center gap-2 mb-3">
        {icon}
        <h2 className="text-sm font-medium" style={{ color: "var(--text-secondary)" }}>{title}</h2>
      </div>
      <div className="pl-6">{children}</div>
    </section>
  );
}

function ScanPathsSection({
  report,
  onRefresh,
}: {
  report: DoctorReport | null;
  onRefresh: () => void;
}) {
  const [removing, setRemoving] = useState<string | null>(null);
  const [sectionError, setSectionError] = useState<string | null>(null);

  const handleAddPath = async () => {
    setSectionError(null);
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    try {
      await invoke("add_scan_path", { path: selected });
      onRefresh();
    } catch (e) {
      setSectionError(`Failed to add path: ${e}`);
    }
  };

  const handleRemovePath = async (path: string) => {
    setSectionError(null);
    setRemoving(path);
    try {
      await invoke("remove_scan_path", { path });
      onRefresh();
    } catch (e) {
      setSectionError(`Failed to remove path: ${e}`);
    } finally {
      setRemoving(null);
    }
  };

  return (
    <Section
      icon={<FolderOpen size={16} style={{ color: "var(--icon-folder)" }} />}
      title="Scan Paths"
    >
      <div className="space-y-1.5">
        {report?.scan_paths.map((sp) => (
          <div
            key={sp.path}
            className="flex items-center justify-between gap-2 text-sm px-3 py-2 rounded"
            style={{ background: "var(--bg-tertiary)" }}
          >
            <span
              className="truncate"
              style={{ color: sp.exists ? "var(--text-primary)" : "var(--error)" }}
            >
              {sp.path}
              {!sp.exists && " (missing)"}
            </span>
            <button
              onClick={() => handleRemovePath(sp.path)}
              disabled={removing === sp.path}
              className="transition-colors shrink-0"
              style={{ color: "var(--text-tertiary)" }}
            >
              {removing === sp.path ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <X size={14} />
              )}
            </button>
          </div>
        ))}
      </div>
      <button
        onClick={handleAddPath}
        className="mt-2 flex items-center gap-1.5 text-xs transition-colors"
        style={{ color: "var(--accent)" }}
      >
        <Plus size={12} /> Add folder
      </button>
      {sectionError && <p className="error-message">{sectionError}</p>}
    </Section>
  );
}

function ThemeSection() {
  const { preference, setPreference } = useTheme();

  const options: { value: ThemePreference; label: string; icon: React.ReactNode }[] = [
    { value: "light", label: "Light", icon: <Sun size={14} /> },
    { value: "dark", label: "Dark", icon: <Moon size={14} /> },
    { value: "system", label: "System", icon: <Monitor size={14} /> },
  ];

  return (
    <Section
      icon={<Sun size={16} style={{ color: "var(--warning)" }} />}
      title="Appearance"
    >
      <div className="flex gap-1 rounded-lg p-0.5" style={{ background: "var(--bg-tertiary)" }}>
        {options.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setPreference(opt.value)}
            className="flex-1 flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors"
            style={{
              background: preference === opt.value ? "var(--accent)" : "transparent",
              color: preference === opt.value ? "var(--accent-text)" : "var(--text-secondary)",
            }}
          >
            {opt.icon}
            {opt.label}
          </button>
        ))}
      </div>
    </Section>
  );
}

function SearchHotkeySection() {
  const isMac =
    typeof navigator !== "undefined" &&
    ((navigator as any).userAgentData?.platform?.includes("Mac") ??
      navigator.platform.includes("Mac"));
  return (
    <Section
      icon={<Keyboard size={16} style={{ color: "var(--warning)" }} />}
      title="Search Hotkey"
    >
      <div className="text-sm" style={{ color: "var(--text-secondary)" }}>
        <kbd
          className="px-2 py-0.5 rounded text-xs"
          style={{
            background: "var(--kbd-bg)",
            border: "1px solid var(--kbd-border)",
            color: "var(--text-primary)",
          }}
        >
          {isMac ? "Cmd+Shift+F" : "Ctrl+Shift+F"}
        </kbd>
        <span className="ml-2 text-xs" style={{ color: "var(--text-tertiary)" }}>
          Customization coming in a future update
        </span>
      </div>
    </Section>
  );
}

function LaunchAtLoginSection() {
  const [enabled, setEnabled] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [sectionError, setSectionError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("get_autostart_status")
      .then((v) => {
        setEnabled(v);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const toggle = async () => {
    setSectionError(null);
    const next = !enabled;
    try {
      await invoke("set_autostart", { enabled: next });
      setEnabled(next);
    } catch (e) {
      setSectionError(`Failed to set autostart: ${e}`);
    }
  };

  if (!loaded) return null;

  return (
    <Section
      icon={<Power size={16} style={{ color: "var(--success)" }} />}
      title="Launch at Login"
    >
      <button
        onClick={toggle}
        className="relative w-10 h-5 rounded-full transition-colors"
        style={{ background: enabled ? "var(--accent)" : "var(--bg-tertiary)" }}
      >
        <span
          className="absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform"
          style={{ left: enabled ? "22px" : "2px" }}
        />
      </button>
      {sectionError && <p className="error-message">{sectionError}</p>}
    </Section>
  );
}

function SemanticSearchSection() {
  const [status, setStatus] = useState<string | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [sectionError, setSectionError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("get_api_key_status")
      .then(setStatus)
      .catch(() => setStatus("unknown"));
  }, []);

  const handleSave = async () => {
    if (!keyInput.trim()) return;
    setSaving(true);
    setSectionError(null);
    try {
      await invoke("set_api_key", { key: keyInput.trim() });
      setStatus("configured");
      setKeyInput("");
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setSectionError(`Failed to save key: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Section
      icon={<Brain size={16} style={{ color: "var(--icon-image)" }} />}
      title="Semantic Search"
    >
      <div className="space-y-2">
        <div className="flex items-center gap-2 text-sm">
          <span style={{ color: "var(--text-secondary)" }}>OpenRouter API key:</span>
          <span style={{ color: status === "configured" ? "var(--success)" : "var(--text-tertiary)" }}>
            {status === "configured" ? "configured" : "not configured"}
          </span>
        </div>
        <div className="flex gap-2">
          <input
            type="password"
            value={keyInput}
            onChange={(e) => setKeyInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            placeholder="sk-or-..."
            className="flex-1 px-2.5 py-1.5 rounded text-sm outline-none"
            style={{
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              color: "var(--text-primary)",
            }}
          />
          <button
            onClick={handleSave}
            disabled={saving || !keyInput.trim()}
            className="px-3 py-1.5 rounded text-xs font-medium transition-colors disabled:opacity-40"
            style={{ background: "var(--accent)", color: "var(--accent-text)" }}
          >
            {saved ? <Check size={14} /> : saving ? "..." : "Save"}
          </button>
        </div>
        <a
          href="https://openrouter.ai/keys"
          target="_blank"
          rel="noreferrer"
          className="text-xs transition-colors"
          style={{ color: "var(--accent)" }}
        >
          Get an API key at openrouter.ai
        </a>
        {sectionError && <p className="error-message">{sectionError}</p>}
      </div>
    </Section>
  );
}

function IndexStatusSection({
  report,
  syncStatus,
}: {
  report: DoctorReport | null;
  syncStatus: string | null;
}) {
  if (!report) return null;
  const db = report.database;
  const ocr = report.ocr;
  const hnsw = report.hnsw;

  const formatBytes = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatTime = (iso: string | null) => {
    if (!iso) return "never";
    return new Date(iso).toLocaleString();
  };

  return (
    <Section
      icon={<HardDrive size={16} style={{ color: "var(--accent)" }} />}
      title="Index Status"
    >
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-sm">
        <Stat label="Files indexed" value={db.files_indexed.toLocaleString()} />
        <Stat label="Content indexed" value={db.content_indexed.toLocaleString()} />
        <Stat label="OCR" value={`${ocr.ocr_completed}/${ocr.total_images} images`} />
        <Stat label="Semantic vectors" value={hnsw.index_exists ? hnsw.vector_count.toLocaleString() : "not built"} />
        <Stat label="DB size" value={formatBytes(db.size_bytes)} />
        <Stat label="Content index" value={formatBytes(report.content_index.size_bytes)} />
        <Stat label="Last sync" value={formatTime(db.last_updated)} />
        <Stat label="Last full reindex" value={formatTime(db.last_full_reindex)} />
      </div>
      {syncStatus && (
        <div className="mt-2 text-xs" style={{ color: "var(--text-tertiary)" }}>
          Sync: {syncStatus}
        </div>
      )}
    </Section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span style={{ color: "var(--text-primary)" }}>{value}</span>
    </div>
  );
}

function ReindexSection({ onRefresh }: { onRefresh: () => void }) {
  const [running, setRunning] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const [sectionError, setSectionError] = useState<string | null>(null);

  const handleReindex = async () => {
    if (!confirmed) {
      setConfirmed(true);
      return;
    }
    setRunning(true);
    setConfirmed(false);
    setSectionError(null);
    try {
      await invoke("run_reindex");
      onRefresh();
    } catch (e) {
      setSectionError(`Reindex failed: ${e}`);
    } finally {
      setRunning(false);
    }
  };

  return (
    <Section
      icon={<RefreshCw size={16} style={{ color: "var(--warning)" }} />}
      title="Reindex"
    >
      <div className="flex items-center gap-3">
        <button
          onClick={handleReindex}
          disabled={running}
          className="px-3 py-1.5 rounded text-xs font-medium transition-colors disabled:opacity-50"
          style={{
            background: confirmed ? "var(--error)" : "var(--bg-tertiary)",
            color: confirmed ? "var(--accent-text)" : "var(--text-primary)",
            border: confirmed ? "none" : "1px solid var(--border)",
          }}
        >
          {running ? "Rebuilding..." : confirmed ? "Confirm rebuild" : "Rebuild Index"}
        </button>
        {confirmed && (
          <button
            onClick={() => setConfirmed(false)}
            className="text-xs"
            style={{ color: "var(--text-secondary)" }}
          >
            Cancel
          </button>
        )}
      </div>
      <p className="mt-1.5 text-xs" style={{ color: "var(--text-tertiary)" }}>
        Deletes and rebuilds the entire index. May take several minutes.
      </p>
      {sectionError && <p className="error-message">{sectionError}</p>}
    </Section>
  );
}

function LicenseSection() {
  const [status, setStatus] = useState<string | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [activating, setActivating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<LicenseState>("get_license_state")
      .then((s) => setStatus(s.status))
      .catch(() => setStatus("unknown"));
  }, []);

  const handleActivate = async () => {
    if (!keyInput.trim()) return;
    setActivating(true);
    setError(null);
    try {
      const state = await invoke<LicenseState>("activate_license", {
        key: keyInput.trim(),
      });
      setStatus(state.status);
      setKeyInput("");
    } catch (e) {
      setError(String(e));
    } finally {
      setActivating(false);
    }
  };

  const statusLabel: Record<string, string> = {
    active: "Active", trial: "Trial", trial_expired: "Trial expired",
    invalid: "Invalid", unknown: "Unknown",
  };
  const statusColor: Record<string, string> = {
    active: "var(--success)", trial: "var(--warning)", trial_expired: "var(--error)",
    invalid: "var(--error)", unknown: "var(--text-tertiary)",
  };

  return (
    <Section
      icon={<Shield size={16} style={{ color: "var(--success)" }} />}
      title="License"
    >
      <div className="space-y-2">
        <div className="text-sm">
          <span style={{ color: "var(--text-secondary)" }}>Status: </span>
          <span style={{ color: statusColor[status ?? "unknown"] }}>
            {statusLabel[status ?? "unknown"]}
          </span>
        </div>
        {status !== "active" && (
          <>
            <div className="flex gap-2">
              <input
                type="text"
                value={keyInput}
                onChange={(e) => setKeyInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleActivate()}
                placeholder="Enter license key"
                className="flex-1 px-2.5 py-1.5 rounded text-sm outline-none"
                style={{
                  background: "var(--bg-tertiary)",
                  border: "1px solid var(--border)",
                  color: "var(--text-primary)",
                }}
              />
              <button
                onClick={handleActivate}
                disabled={activating || !keyInput.trim()}
                className="px-3 py-1.5 rounded text-xs font-medium transition-colors disabled:opacity-40"
                style={{ background: "var(--accent)", color: "var(--accent-text)" }}
              >
                {activating ? "..." : "Activate"}
              </button>
            </div>
            {error && <p className="text-xs" style={{ color: "var(--error)" }}>{error}</p>}
            <a
              href="https://polar.sh/findr"
              target="_blank"
              rel="noreferrer"
              className="text-xs transition-colors"
              style={{ color: "var(--accent)" }}
            >
              Purchase a license
            </a>
          </>
        )}
      </div>
    </Section>
  );
}

function AboutSection({ report }: { report: DoctorReport | null }) {
  const [appVersion, setAppVersion] = useState("");
  const [findrVersion, setFindrVersion] = useState("");

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => {});
    invoke<string>("get_findr_version").then(setFindrVersion).catch(() => {});
  }, []);

  return (
    <Section
      icon={<Info size={16} style={{ color: "var(--text-secondary)" }} />}
      title="About"
    >
      <div className="space-y-1 text-sm">
        <div className="flex justify-between">
          <span style={{ color: "var(--text-secondary)" }}>Desktop version</span>
          <span style={{ color: "var(--text-primary)" }}>{appVersion || "..."}</span>
        </div>
        <div className="flex justify-between">
          <span style={{ color: "var(--text-secondary)" }}>findr CLI version</span>
          <span style={{ color: "var(--text-primary)" }}>{findrVersion || "..."}</span>
        </div>
        {report && (
          <div className="flex justify-between">
            <span style={{ color: "var(--text-secondary)" }}>Platform</span>
            <span style={{ color: "var(--text-primary)" }}>
              {report.os.os} ({report.os.arch})
            </span>
          </div>
        )}
        <div className="pt-1">
          <a
            href="https://github.com/Roderick111/findr-app"
            target="_blank"
            rel="noreferrer"
            className="text-xs transition-colors"
            style={{ color: "var(--accent)" }}
          >
            GitHub
          </a>
        </div>
      </div>
    </Section>
  );
}
