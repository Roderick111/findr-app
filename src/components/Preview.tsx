import { useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import {
  File,
  Folder,
  FileText,
  FileImage,
  Code,
  FileVideo,
  FileAudio,
  FileArchive,
} from "lucide-react";
import type { SearchResult } from "../types";
import {
  IMAGE_EXTS, TEXT_EXTS, MD_EXTS, CODE_EXTS, VIDEO_EXTS, AUDIO_EXTS, ARCHIVE_EXTS,
  formatSize, formatDate, previewKind,
} from "../utils/format";

interface PreviewText {
  text: string;
  truncated: boolean;
}

function bigIconFor(r: SearchResult) {
  if (r.is_dir) return <Folder size={96} style={{ color: "var(--icon-folder)" }} />;
  const ext = r.file_type?.toLowerCase() ?? "";
  if (IMAGE_EXTS.includes(ext)) return <FileImage size={96} style={{ color: "var(--icon-image)" }} />;
  if (VIDEO_EXTS.includes(ext)) return <FileVideo size={96} style={{ color: "var(--icon-image)" }} />;
  if (AUDIO_EXTS.includes(ext)) return <FileAudio size={96} style={{ color: "var(--warning)" }} />;
  if (ARCHIVE_EXTS.includes(ext)) return <FileArchive size={96} style={{ color: "var(--warning)" }} />;
  if (TEXT_EXTS.includes(ext) || MD_EXTS.includes(ext) || ext === "pdf" || ext === "docx")
    return <FileText size={96} style={{ color: "var(--icon-doc)" }} />;
  if (CODE_EXTS.includes(ext)) return <Code size={96} style={{ color: "var(--icon-code)" }} />;
  return <File size={96} style={{ color: "var(--icon-default)" }} />;
}

export function Preview({ result }: { result: SearchResult | null }) {
  const [textContent, setTextContent] = useState<string | null>(null);
  const [textError, setTextError] = useState<string | null>(null);

  const kind = result ? previewKind(result) : null;

  useEffect(() => {
    if (!result || (kind !== "text" && kind !== "code" && kind !== "markdown")) {
      setTextContent(null);
      setTextError(null);
      return;
    }
    let cancelled = false;
    setTextContent(null);
    setTextError(null);

    (async () => {
      try {
        const preview = await invoke<PreviewText>("read_preview_text", { path: result.path });
        if (cancelled) return;
        let text = preview.text;
        if (preview.truncated) {
          text += "\n\n… (truncated)";
        }
        setTextContent(text);
      } catch (e) {
        if (cancelled) return;
        setTextError(String(e));
      }
    })();

    return () => { cancelled = true; };
  }, [result?.path, kind]);

  if (!result) {
    return (
      <div className="h-full flex items-center justify-center text-sm" style={{ color: "var(--text-tertiary)" }}>
        Select a file to preview
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 min-h-0 overflow-hidden" style={{ background: "var(--preview-bg)" }}>
        {kind === "image" && (
          <div className="h-full flex items-center justify-center p-4">
            <img
              src={convertFileSrc(result.path)}
              alt={result.filename}
              className="max-h-full max-w-full object-contain rounded shadow-lg"
            />
          </div>
        )}

        {kind === "pdf" && (
          <embed
            src={convertFileSrc(result.path)}
            type="application/pdf"
            className="w-full h-full"
          />
        )}

        {kind === "markdown" && textContent !== null && (
          <div className="h-full overflow-y-auto p-5 text-sm max-w-none">
            <div className="markdown-preview">
              <ReactMarkdown>{textContent}</ReactMarkdown>
            </div>
          </div>
        )}

        {(kind === "text" || kind === "code") && textContent !== null && (
          <pre
            className="h-full overflow-auto p-4 text-xs font-mono whitespace-pre"
            style={{ color: "var(--text-primary)" }}
          >
            {textContent}
          </pre>
        )}

        {(kind === "text" || kind === "code" || kind === "markdown") &&
          textContent === null && !textError && (
            <div className="h-full flex items-center justify-center text-xs" style={{ color: "var(--text-tertiary)" }}>
              loading…
            </div>
          )}

        {textError && (
          <div className="h-full flex items-center justify-center p-4 text-xs text-center" style={{ color: "var(--error)" }}>
            failed to read: {textError}
          </div>
        )}

        {kind === "icon" && (
          <div className="h-full flex items-center justify-center p-4">
            <div className="flex flex-col items-center gap-4 text-center">
              {bigIconFor(result)}
              <div className="text-sm max-w-[90%] truncate" style={{ color: "var(--text-primary)" }}>
                {result.filename}
              </div>
              {result.content_snippet && (
                <div className="text-xs italic max-w-[90%] line-clamp-6 whitespace-pre-wrap" style={{ color: "var(--text-secondary)" }}>
                  {result.content_snippet}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="px-4 py-3 text-xs space-y-1.5 shrink-0" style={{ borderTop: "1px solid var(--border)" }}>
        <Row label="Path" value={result.path} mono />
        <Row
          label="Type"
          value={result.is_dir ? "Folder" : (result.file_type?.toUpperCase() ?? "—")}
        />
        <Row label="Size" value={formatSize(result.size_bytes)} />
        <Row label="Modified" value={formatDate(result.modified)} />
        {result.interactions > 0 && (
          <Row label="Opens" value={String(result.interactions)} />
        )}
      </div>
    </div>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid grid-cols-[80px_1fr] gap-2">
      <span style={{ color: "var(--text-tertiary)" }}>{label}</span>
      <span
        className={`truncate ${mono ? "font-mono text-[11px]" : ""}`}
        style={{ color: "var(--text-primary)" }}
        title={value}
      >
        {value}
      </span>
    </div>
  );
}
