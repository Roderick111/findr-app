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
} from "lucide-react";
import type { DoctorReport, LicenseState } from "../types";

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
    loadReport();
    const interval = setInterval(loadReport, 2000);
    return () => clearInterval(interval);
  }, [loadReport]);

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
      <div className="h-screen flex items-center justify-center settings-root text-neutral-200">
        <Loader2 size={20} className="animate-spin text-neutral-500" />
      </div>
    );
  }

  return (
    <div className="h-screen flex flex-col settings-root text-neutral-200">
      <header className="px-6 py-4 border-b border-neutral-800 shrink-0">
        <h1 className="text-lg font-semibold">Settings</h1>
      </header>
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-8">
        {error && (
          <div className="text-sm text-red-400 bg-red-950/30 px-3 py-2 rounded">
            {error}
          </div>
        )}
        <ScanPathsSection report={report} onRefresh={loadReport} />
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
        <h2 className="text-sm font-medium text-neutral-300">{title}</h2>
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

  const handleAddPath = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    try {
      await invoke("add_scan_path", { path: selected });
      onRefresh();
    } catch (e) {
      alert(`Failed to add path: ${e}`);
    }
  };

  const handleRemovePath = async (path: string) => {
    setRemoving(path);
    try {
      await invoke("remove_scan_path", { path });
      onRefresh();
    } catch (e) {
      alert(`Failed to remove path: ${e}`);
    } finally {
      setRemoving(null);
    }
  };

  return (
    <Section
      icon={<FolderOpen size={16} className="text-blue-400" />}
      title="Scan Paths"
    >
      <div className="space-y-1.5">
        {report?.scan_paths.map((sp) => (
          <div
            key={sp.path}
            className="flex items-center justify-between gap-2 text-sm bg-neutral-900 px-3 py-2 rounded"
          >
            <span
              className={`truncate ${sp.exists ? "text-neutral-300" : "text-red-400"}`}
            >
              {sp.path}
              {!sp.exists && " (missing)"}
            </span>
            <button
              onClick={() => handleRemovePath(sp.path)}
              disabled={removing === sp.path}
              className="text-neutral-600 hover:text-red-400 transition-colors shrink-0"
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
        className="mt-2 flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 transition-colors"
      >
        <Plus size={12} /> Add folder
      </button>
    </Section>
  );
}

function SearchHotkeySection() {
  const isMac =
    typeof navigator !== "undefined" && navigator.platform.includes("Mac");
  return (
    <Section
      icon={<Keyboard size={16} className="text-yellow-400" />}
      title="Search Hotkey"
    >
      <div className="text-sm text-neutral-400">
        <kbd className="px-2 py-0.5 bg-neutral-800 border border-neutral-700 rounded text-neutral-300 text-xs">
          {isMac ? "Cmd+Shift+F" : "Ctrl+Shift+F"}
        </kbd>
        <span className="ml-2 text-neutral-600 text-xs">
          Customization coming in a future update
        </span>
      </div>
    </Section>
  );
}

function LaunchAtLoginSection() {
  const [enabled, setEnabled] = useState(false);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    invoke<boolean>("get_autostart_status")
      .then((v) => {
        setEnabled(v);
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }, []);

  const toggle = async () => {
    const next = !enabled;
    try {
      await invoke("set_autostart", { enabled: next });
      setEnabled(next);
    } catch (e) {
      alert(`Failed to set autostart: ${e}`);
    }
  };

  if (!loaded) return null;

  return (
    <Section
      icon={<Power size={16} className="text-green-400" />}
      title="Launch at Login"
    >
      <button
        onClick={toggle}
        className={`relative w-10 h-5 rounded-full transition-colors ${enabled ? "bg-blue-600" : "bg-neutral-700"}`}
      >
        <span
          className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${enabled ? "left-5.5" : "left-0.5"}`}
        />
      </button>
    </Section>
  );
}

function SemanticSearchSection() {
  const [status, setStatus] = useState<string | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    invoke<string>("get_api_key_status")
      .then(setStatus)
      .catch(() => setStatus("unknown"));
  }, []);

  const handleSave = async () => {
    if (!keyInput.trim()) return;
    setSaving(true);
    try {
      await invoke("set_api_key", { key: keyInput.trim() });
      setStatus("configured");
      setKeyInput("");
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      alert(`Failed to save key: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Section
      icon={<Brain size={16} className="text-purple-400" />}
      title="Semantic Search"
    >
      <div className="space-y-2">
        <div className="flex items-center gap-2 text-sm">
          <span className="text-neutral-500">OpenRouter API key:</span>
          <span
            className={
              status === "configured" ? "text-green-400" : "text-neutral-500"
            }
          >
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
            className="flex-1 px-2.5 py-1.5 bg-neutral-900 border border-neutral-700 rounded text-sm text-neutral-200 placeholder-neutral-600 outline-none focus:border-blue-500/50"
          />
          <button
            onClick={handleSave}
            disabled={saving || !keyInput.trim()}
            className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-500 rounded text-xs font-medium transition-colors"
          >
            {saved ? <Check size={14} /> : saving ? "..." : "Save"}
          </button>
        </div>
        <a
          href="https://openrouter.ai/keys"
          target="_blank"
          rel="noreferrer"
          className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors"
        >
          Get an API key at openrouter.ai
        </a>
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
    const d = new Date(iso);
    return d.toLocaleString();
  };

  return (
    <Section
      icon={<HardDrive size={16} className="text-cyan-400" />}
      title="Index Status"
    >
      <div className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-sm">
        <Stat label="Files indexed" value={db.files_indexed.toLocaleString()} />
        <Stat
          label="Content indexed"
          value={db.content_indexed.toLocaleString()}
        />
        <Stat
          label="OCR"
          value={`${ocr.ocr_completed}/${ocr.total_images} images`}
        />
        <Stat
          label="Semantic vectors"
          value={
            hnsw.index_exists
              ? hnsw.vector_count.toLocaleString()
              : "not built"
          }
        />
        <Stat label="DB size" value={formatBytes(db.size_bytes)} />
        <Stat
          label="Content index"
          value={formatBytes(report.content_index.size_bytes)}
        />
        <Stat label="Last sync" value={formatTime(db.last_updated)} />
        <Stat
          label="Last full reindex"
          value={formatTime(db.last_full_reindex)}
        />
      </div>
      {syncStatus && (
        <div className="mt-2 text-xs text-neutral-500">
          Sync: {syncStatus}
        </div>
      )}
    </Section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-neutral-500">{label}</span>
      <span className="text-neutral-300">{value}</span>
    </div>
  );
}

function ReindexSection({ onRefresh }: { onRefresh: () => void }) {
  const [running, setRunning] = useState(false);
  const [confirmed, setConfirmed] = useState(false);

  const handleReindex = async () => {
    if (!confirmed) {
      setConfirmed(true);
      return;
    }
    setRunning(true);
    setConfirmed(false);
    try {
      await invoke("run_reindex");
      onRefresh();
    } catch (e) {
      alert(`Reindex failed: ${e}`);
    } finally {
      setRunning(false);
    }
  };

  return (
    <Section
      icon={<RefreshCw size={16} className="text-orange-400" />}
      title="Reindex"
    >
      <div className="flex items-center gap-3">
        <button
          onClick={handleReindex}
          disabled={running}
          className={`px-3 py-1.5 rounded text-xs font-medium transition-colors ${
            confirmed
              ? "bg-red-600 hover:bg-red-500"
              : "bg-neutral-800 hover:bg-neutral-700 border border-neutral-700"
          } disabled:opacity-50`}
        >
          {running
            ? "Rebuilding..."
            : confirmed
              ? "Confirm rebuild"
              : "Rebuild Index"}
        </button>
        {confirmed && (
          <button
            onClick={() => setConfirmed(false)}
            className="text-xs text-neutral-500 hover:text-neutral-300"
          >
            Cancel
          </button>
        )}
      </div>
      <p className="mt-1.5 text-xs text-neutral-600">
        Deletes and rebuilds the entire index. May take several minutes.
      </p>
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

  const statusLabel = {
    active: "Active",
    trial: "Trial",
    trial_expired: "Trial expired",
    invalid: "Invalid",
    unknown: "Unknown",
  }[status ?? "unknown"];

  const statusColor = {
    active: "text-green-400",
    trial: "text-amber-400",
    trial_expired: "text-red-400",
    invalid: "text-red-400",
    unknown: "text-neutral-500",
  }[status ?? "unknown"];

  return (
    <Section
      icon={<Shield size={16} className="text-emerald-400" />}
      title="License"
    >
      <div className="space-y-2">
        <div className="text-sm">
          <span className="text-neutral-500">Status: </span>
          <span className={statusColor}>{statusLabel}</span>
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
                className="flex-1 px-2.5 py-1.5 bg-neutral-900 border border-neutral-700 rounded text-sm text-neutral-200 placeholder-neutral-600 outline-none focus:border-blue-500/50"
              />
              <button
                onClick={handleActivate}
                disabled={activating || !keyInput.trim()}
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-500 rounded text-xs font-medium transition-colors"
              >
                {activating ? "..." : "Activate"}
              </button>
            </div>
            {error && <p className="text-xs text-red-400">{error}</p>}
            <a
              href="https://polar.sh/findr"
              target="_blank"
              rel="noreferrer"
              className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors"
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
    invoke<string>("get_findr_version")
      .then(setFindrVersion)
      .catch(() => {});
  }, []);

  return (
    <Section
      icon={<Info size={16} className="text-neutral-400" />}
      title="About"
    >
      <div className="space-y-1 text-sm">
        <div className="flex justify-between">
          <span className="text-neutral-500">Desktop version</span>
          <span className="text-neutral-300">{appVersion || "..."}</span>
        </div>
        <div className="flex justify-between">
          <span className="text-neutral-500">findr CLI version</span>
          <span className="text-neutral-300">{findrVersion || "..."}</span>
        </div>
        {report && (
          <div className="flex justify-between">
            <span className="text-neutral-500">Platform</span>
            <span className="text-neutral-300">
              {report.os.os} ({report.os.arch})
            </span>
          </div>
        )}
        <div className="pt-1">
          <a
            href="https://github.com/Roderick111/findr-app"
            target="_blank"
            rel="noreferrer"
            className="text-xs text-blue-400/70 hover:text-blue-400 transition-colors"
          >
            GitHub
          </a>
        </div>
      </div>
    </Section>
  );
}
