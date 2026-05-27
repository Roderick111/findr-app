import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openInOS } from "@tauri-apps/plugin-shell";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { File, Folder, FileText, FileImage, Code, Settings } from "lucide-react";
import { useDebounced } from "./hooks/useDebounced";
import { Preview } from "./components/Preview";
import { UpdateBanner } from "./components/UpdateBanner";
import type { SearchResponse, SearchResult } from "./types";

const LIMIT = 30;
const RECENT_LIMIT = 20;
const DEBOUNCE_MS = 200;

function iconFor(r: SearchResult) {
  if (r.is_dir) return <Folder size={16} className="text-blue-400 shrink-0" />;
  const ext = r.file_type?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "heic", "gif", "webp", "svg"].includes(ext))
    return <FileImage size={16} className="text-purple-400 shrink-0" />;
  if (["md", "txt", "pdf", "docx", "csv"].includes(ext))
    return <FileText size={16} className="text-green-400 shrink-0" />;
  if (["rs", "ts", "tsx", "js", "py", "go", "swift", "java"].includes(ext))
    return <Code size={16} className="text-orange-400 shrink-0" />;
  return <File size={16} className="text-gray-400 shrink-0" />;
}

export default function App() {
  const [query, setQuery] = useState("");
  const debounced = useDebounced(query, DEBOUNCE_MS);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [elapsedMs, setElapsedMs] = useState<number | null>(null);
  const [totalResults, setTotalResults] = useState<number | null>(null);
  const [mode, setMode] = useState<string>("idle");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [selected, setSelected] = useState(0);
  const requestIdRef = useRef(0);
  const listRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const flashToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 1200);
  }, []);

  const trackAction = useCallback((path: string, action: string) => {
    invoke("track_interaction", { path, action }).catch(() => {});
  }, []);

  const hideOverlay = useCallback(() => {
    invoke("hide_overlay").catch(() => {});
  }, []);

  const openFile = useCallback(async (r: SearchResult) => {
    try {
      await openInOS(r.path);
      trackAction(r.path, "open");
      hideOverlay();
    } catch (e) {
      setError(`failed to open: ${e}`);
    }
  }, [trackAction, hideOverlay]);

  const revealInFinder = useCallback(async (r: SearchResult) => {
    try {
      await revealItemInDir(r.path);
      trackAction(r.path, "finder");
      flashToast("Revealed in Finder");
      hideOverlay();
    } catch (e) {
      setError(`failed to reveal: ${e}`);
    }
  }, [trackAction, flashToast, hideOverlay]);

  const copyPath = useCallback(async (r: SearchResult) => {
    await writeText(r.path);
    trackAction(r.path, "copy");
    flashToast("Path copied");
  }, [trackAction, flashToast]);

  const copyFilename = useCallback(async (r: SearchResult) => {
    await writeText(r.filename);
    trackAction(r.path, "copy");
    flashToast("Filename copied");
  }, [trackAction, flashToast]);

  // Reset state and refocus input when window becomes visible
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        setQuery("");
        setSelected(0);
        // Slight delay so input is mounted/visible
        setTimeout(() => inputRef.current?.focus(), 50);
      }
    });
    return () => {
      unlisten.then((u) => u());
    };
  }, []);

  useEffect(() => {
    const trimmed = debounced.trim();
    const isRecent = trimmed.length < 2;
    const myRequestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);

    const promise = isRecent
      ? invoke<SearchResponse>("get_recent_files", { limit: RECENT_LIMIT })
      : invoke<SearchResponse>("search", {
          query: trimmed,
          limit: LIMIT,
          noSemantic: true,
        });

    promise
      .then((r) => {
        if (myRequestId !== requestIdRef.current) return;
        setResults(r.results);
        setElapsedMs(r.elapsed_ms);
        setTotalResults(r.total_results);
        setMode(r.mode);
        setSelected(0);
      })
      .catch((e: string) => {
        if (myRequestId !== requestIdRef.current) return;
        setError(e);
        setResults([]);
      })
      .finally(() => {
        if (myRequestId === requestIdRef.current) setLoading(false);
      });
  }, [debounced]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const r = results[selected];
      const meta = e.metaKey || e.ctrlKey;

      if (e.key === "Escape") {
        e.preventDefault();
        hideOverlay();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, results.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (e.key === "Enter" && r) {
        e.preventDefault();
        openFile(r);
      } else if (meta && e.key === "o" && r) {
        e.preventDefault();
        openFile(r);
      } else if (meta && e.key === "r" && r) {
        e.preventDefault();
        revealInFinder(r);
      } else if (meta && e.shiftKey && e.key.toLowerCase() === "c" && r) {
        e.preventDefault();
        copyFilename(r);
      } else if (meta && !e.shiftKey && e.key === "c" && r) {
        const sel = window.getSelection()?.toString();
        if (sel && sel.length > 0) return;
        e.preventDefault();
        copyPath(r);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [results, selected, openFile, revealInFinder, copyPath, copyFilename, hideOverlay]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLDivElement>(
      `[data-idx="${selected}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const statusLine = useMemo(() => {
    if (error) return <span className="text-red-400">{error}</span>;
    if (loading) return <span className="text-gray-500">searching…</span>;
    if (totalResults === null) return <span className="text-gray-600">loading…</span>;
    const modeLabel =
      mode === "recent"
        ? "recent"
        : `${totalResults} result${totalResults === 1 ? "" : "s"}`;
    return (
      <span className="text-gray-500">
        {modeLabel} · {elapsedMs}ms
      </span>
    );
  }, [error, loading, totalResults, elapsedMs, mode]);

  const selectedResult = results[selected] ?? null;

  return (
    <div className="h-screen flex flex-col text-neutral-200 relative overlay-root">
      {/* Drag region (top 8px of window) */}
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 h-2 z-50" />
      <UpdateBanner />

      {/* Search bar */}
      <div className="px-4 pt-3 pb-2 border-b border-white/5">
        <input
          ref={inputRef}
          autoFocus
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search files…"
          className="w-full bg-transparent text-xl outline-none placeholder-neutral-600"
        />
      </div>

      {/* Split: list (left) + preview (right) */}
      <div className="flex-1 flex min-h-0">
        <div className="w-[42%] min-w-[280px] flex flex-col border-r border-white/5">
          <div ref={listRef} className="flex-1 overflow-y-auto">
            {results.map((r, i) => (
              <div
                key={`${r.path}-${i}`}
                data-idx={i}
                className={`px-4 py-2 flex gap-3 items-center cursor-default ${
                  i === selected ? "bg-white/10" : "hover:bg-white/5"
                }`}
                onClick={() => setSelected(i)}
                onDoubleClick={() => openFile(r)}
              >
                {iconFor(r)}
                <div className="flex-1 min-w-0">
                  <div className="truncate text-sm">{r.filename}</div>
                  <div className="truncate text-xs text-neutral-500">{r.path}</div>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="flex-1 min-w-0">
          <Preview result={selectedResult} />
        </div>
      </div>

      {/* Footer */}
      <div className="px-4 py-1.5 border-t border-white/5 text-xs flex justify-between items-center">
        <div className="flex items-center gap-2">
          {statusLine}
          <button
            onClick={() => invoke("open_settings").catch(() => {})}
            className="text-neutral-600 hover:text-neutral-400 transition-colors"
            title="Settings"
          >
            <Settings size={13} />
          </button>
        </div>
        <span className="text-neutral-600">
          <Kbd>↵</Kbd> open · <Kbd>⌘R</Kbd> reveal · <Kbd>⌘C</Kbd> path · <Kbd>⌘⇧C</Kbd> name · <Kbd>esc</Kbd> hide
        </span>
      </div>

      {/* Toast */}
      {toast && (
        <div className="absolute bottom-10 left-1/2 -translate-x-1/2 bg-neutral-800/90 text-neutral-100 text-xs px-3 py-1.5 rounded shadow-lg backdrop-blur">
          {toast}
        </div>
      )}
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-block px-1 mx-0.5 text-[10px] bg-white/10 border border-white/10 rounded text-neutral-400 align-middle">
      {children}
    </span>
  );
}
