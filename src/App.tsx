import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { File, Folder, FileText, FileImage, Code, Settings } from "lucide-react";
import { useDebounced } from "./hooks/useDebounced";
import { Preview } from "./components/Preview";
import { ActionsPanel } from "./components/ActionsPanel";
import { UpdateBanner } from "./components/UpdateBanner";
import type { SearchResponse, SearchResult } from "./types";

const LIMIT = 30;
const RECENT_LIMIT = 20;
const DEBOUNCE_MS = 200;

const isMac = navigator.userAgent.includes("Mac");
const modKey = isMac ? "⌘" : "Ctrl+";

function iconFor(r: SearchResult) {
  if (r.is_dir) return <Folder size={16} style={{ color: "var(--icon-folder)" }} className="shrink-0" />;
  const ext = r.file_type?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "heic", "gif", "webp", "svg"].includes(ext))
    return <FileImage size={16} style={{ color: "var(--icon-image)" }} className="shrink-0" />;
  if (["md", "txt", "pdf", "docx", "csv"].includes(ext))
    return <FileText size={16} style={{ color: "var(--icon-doc)" }} className="shrink-0" />;
  if (["rs", "ts", "tsx", "js", "py", "go", "swift", "java"].includes(ext))
    return <Code size={16} style={{ color: "var(--icon-code)" }} className="shrink-0" />;
  return <File size={16} style={{ color: "var(--icon-default)" }} className="shrink-0" />;
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
  const [showActions, setShowActions] = useState(false);
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
      await openPath(r.path);
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
    try {
      await writeText(r.path);
      trackAction(r.path, "copy");
      flashToast("Path copied");
    } catch (e) {
      setError(`failed to copy path: ${e}`);
    }
  }, [trackAction, flashToast]);

  const copyFilename = useCallback(async (r: SearchResult) => {
    try {
      await writeText(r.filename);
      trackAction(r.path, "copy");
      flashToast("Filename copied");
    } catch (e) {
      setError(`failed to copy filename: ${e}`);
    }
  }, [trackAction, flashToast]);

  const moveToTrash = useCallback(async (r: SearchResult) => {
    try {
      await invoke("move_to_trash", { path: r.path });
      trackAction(r.path, "trash");
      flashToast("Moved to Trash");
      setResults((prev) => {
        const next = prev.filter((item) => item.path !== r.path);
        setSelected((s) => Math.max(0, Math.min(s, next.length - 1)));
        return next;
      });
    } catch (e) {
      setError(`failed to trash: ${e}`);
    }
  }, [trackAction, flashToast]);

  const openSettings = useCallback(() => {
    invoke("open_settings").catch(() => {});
  }, []);

  const wasHiddenRef = useRef(false);

  useEffect(() => {
    const win = getCurrentWindow();
    const unlistenFocus = win.onFocusChanged(({ payload: focused }) => {
      if (focused && wasHiddenRef.current) {
        // Only clear context when re-shown after being hidden (overlay dismiss)
        wasHiddenRef.current = false;
        setQuery("");
        setSelected(0);
        setShowActions(false);
      }
      if (focused) {
        setTimeout(() => inputRef.current?.focus(), 50);
      }
    });
    const unlistenVisibility = win.listen("tauri://window-hidden", () => {
      wasHiddenRef.current = true;
    });
    return () => {
      unlistenFocus.then((u) => u());
      unlistenVisibility.then((u) => u());
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
    if (showActions) return;

    const handler = (e: KeyboardEvent) => {
      const r = results[selected];
      const meta = e.metaKey || e.ctrlKey;

      if (e.key === "Escape") {
        e.preventDefault();
        hideOverlay();
        return;
      }
      if (e.key === "Tab" || (meta && e.key === "k")) {
        e.preventDefault();
        if (r) setShowActions(true);
        return;
      }
      if (meta && e.key === ",") {
        e.preventDefault();
        openSettings();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelected((s) => Math.min(s + 1, results.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelected((s) => Math.max(s - 1, 0));
      } else if (meta && e.key === "Enter" && r) {
        e.preventDefault();
        revealInFinder(r);
      } else if (e.key === "Enter" && r) {
        e.preventDefault();
        openFile(r);
      } else if (meta && e.shiftKey && e.key.toLowerCase() === "c" && r) {
        e.preventDefault();
        copyFilename(r);
      } else if (meta && !e.shiftKey && e.key === "c" && r) {
        // Let native copy work when text is selected in an input/textarea
        const activeEl = document.activeElement;
        if (
          activeEl instanceof HTMLInputElement &&
          activeEl.selectionStart !== null &&
          activeEl.selectionEnd !== null &&
          activeEl.selectionStart !== activeEl.selectionEnd
        ) {
          return;
        }
        if (
          activeEl instanceof HTMLTextAreaElement &&
          activeEl.selectionStart !== null &&
          activeEl.selectionEnd !== null &&
          activeEl.selectionStart !== activeEl.selectionEnd
        ) {
          return;
        }
        const sel = window.getSelection()?.toString();
        if (sel && sel.length > 0) return;
        e.preventDefault();
        copyPath(r);
      } else if (meta && e.key === "Backspace" && r && document.activeElement !== inputRef.current) {
        e.preventDefault();
        moveToTrash(r);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [results, selected, showActions, openFile, revealInFinder, copyPath, copyFilename, moveToTrash, hideOverlay, openSettings]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLDivElement>(
      `[data-idx="${selected}"]`,
    );
    el?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  const statusLine = useMemo(() => {
    if (error) return <span style={{ color: "var(--error)" }}>{error}</span>;
    if (loading) return <span style={{ color: "var(--text-tertiary)" }}>searching…</span>;
    if (totalResults === null) return <span style={{ color: "var(--text-tertiary)" }}>loading…</span>;
    const modeLabel =
      mode === "recent"
        ? "recent"
        : `${totalResults} result${totalResults === 1 ? "" : "s"}`;
    return (
      <span style={{ color: "var(--text-secondary)" }}>
        {modeLabel} · {elapsedMs}ms
      </span>
    );
  }, [error, loading, totalResults, elapsedMs, mode]);

  const selectedResult = results[selected] ?? null;

  return (
    <div className="h-screen flex flex-col relative overlay-root" style={{ color: "var(--text-primary)" }}>
      <div data-tauri-drag-region className="absolute inset-x-0 top-0 h-2 z-50" />
      <UpdateBanner />

      <div className="px-4 pt-3 pb-2" style={{ borderBottom: "1px solid var(--border)" }}>
        <input
          ref={inputRef}
          autoFocus
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search files…"
          className="w-full bg-transparent text-xl outline-none"
          style={{ color: "var(--text-primary)", caretColor: "var(--accent)" }}
        />
      </div>

      <div className="flex-1 flex min-h-0">
        <div className="w-[42%] min-w-[280px] flex flex-col" style={{ borderRight: "1px solid var(--border)" }}>
          <div ref={listRef} className="flex-1 overflow-y-auto">
            {results.length === 0 && !loading ? (
              <div className="flex items-center justify-center h-full">
                <span className="text-sm" style={{ color: "var(--text-tertiary)" }}>
                  {query.trim().length > 0 ? "No results found" : "Start typing to search"}
                </span>
              </div>
            ) : (
              results.map((r, i) => (
                <div
                  key={`${r.path}-${i}`}
                  data-idx={i}
                  className="px-4 py-2 flex gap-3 items-center cursor-default"
                  style={{
                    background: i === selected ? "var(--bg-active)" : "transparent",
                  }}
                  onMouseEnter={(e) => {
                    (e.currentTarget as HTMLDivElement).style.background =
                      i === selected ? "var(--bg-active)" : "var(--bg-hover)";
                  }}
                  onMouseLeave={(e) => {
                    (e.currentTarget as HTMLDivElement).style.background =
                      i === selected ? "var(--bg-active)" : "transparent";
                  }}
                  onClick={() => setSelected(i)}
                  onDoubleClick={() => openFile(r)}
                >
                  {iconFor(r)}
                  <div className="flex-1 min-w-0">
                    <div className="truncate text-sm" style={{ color: "var(--text-primary)" }}>{r.filename}</div>
                    <div className="truncate text-xs" style={{ color: "var(--text-secondary)" }}>{r.path}</div>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="flex-1 min-w-0 relative">
          <Preview result={selectedResult} />
          {showActions && selectedResult && (
            <ActionsPanel
              result={selectedResult}
              onOpen={() => openFile(selectedResult)}
              onReveal={() => revealInFinder(selectedResult)}
              onCopyPath={() => copyPath(selectedResult)}
              onCopyFilename={() => copyFilename(selectedResult)}
              onTrash={() => moveToTrash(selectedResult)}
              onSettings={openSettings}
              onClose={() => setShowActions(false)}
            />
          )}
        </div>
      </div>

      <div className="px-4 py-1.5 text-xs flex justify-between items-center" style={{ borderTop: "1px solid var(--border)" }}>
        <div className="flex items-center gap-2">
          {statusLine}
          <button
            onClick={openSettings}
            className="transition-colors"
            style={{ color: "var(--text-tertiary)" }}
            title="Settings"
          >
            <Settings size={13} />
          </button>
        </div>
        <span style={{ color: "var(--text-secondary)" }}>
          <Kbd>↵</Kbd> open · <Kbd>{modKey}K</Kbd> actions
        </span>
      </div>

      {toast && (
        <div
          className="absolute bottom-10 left-1/2 -translate-x-1/2 text-xs px-3 py-1.5 rounded shadow-lg backdrop-blur"
          style={{ background: "var(--toast-bg)", color: "var(--toast-text)" }}
        >
          {toast}
        </div>
      )}
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <span
      className="inline-block px-1 mx-0.5 text-[10px] rounded align-middle"
      style={{
        background: "var(--kbd-bg)",
        border: "1px solid var(--kbd-border)",
        color: "var(--text-secondary)",
      }}
    >
      {children}
    </span>
  );
}
