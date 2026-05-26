import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { readTextFile } from "@tauri-apps/plugin-fs";
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

const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "heic", "bmp", "svg"];
const TEXT_EXTS = ["txt", "log", "csv", "json", "yml", "yaml", "xml", "toml", "ini", "env"];
const MD_EXTS = ["md", "markdown", "mdx"];
const CODE_EXTS = [
  "rs", "ts", "tsx", "js", "jsx", "py", "go", "swift", "java", "c", "cpp", "h", "hpp",
  "rb", "php", "sh", "bash", "zsh", "css", "scss", "vue", "svelte", "html", "sql",
  "kt", "dart", "lua", "r", "pl", "scala", "ex", "exs", "clj",
];
const VIDEO_EXTS = ["mp4", "mov", "avi", "mkv", "webm"];
const AUDIO_EXTS = ["mp3", "wav", "m4a", "flac", "ogg"];
const ARCHIVE_EXTS = ["zip", "tar", "gz", "rar", "7z"];

const MAX_PREVIEW_BYTES = 50_000;

function bigIconFor(r: SearchResult) {
  if (r.is_dir) return <Folder size={96} className="text-blue-400" />;
  const ext = r.file_type?.toLowerCase() ?? "";
  if (IMAGE_EXTS.includes(ext)) return <FileImage size={96} className="text-purple-400" />;
  if (VIDEO_EXTS.includes(ext)) return <FileVideo size={96} className="text-pink-400" />;
  if (AUDIO_EXTS.includes(ext)) return <FileAudio size={96} className="text-yellow-400" />;
  if (ARCHIVE_EXTS.includes(ext)) return <FileArchive size={96} className="text-amber-400" />;
  if (TEXT_EXTS.includes(ext) || MD_EXTS.includes(ext) || ext === "pdf" || ext === "docx")
    return <FileText size={96} className="text-green-400" />;
  if (CODE_EXTS.includes(ext)) return <Code size={96} className="text-orange-400" />;
  return <File size={96} className="text-gray-400" />;
}

function formatSize(bytes: number | null): string {
  if (bytes === null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) {
    return `Today at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) {
    return `Yesterday at ${d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }
  return d.toLocaleString([], {
    year: "numeric", month: "short", day: "numeric",
    hour: "2-digit", minute: "2-digit",
  });
}

type PreviewKind = "image" | "pdf" | "markdown" | "text" | "code" | "icon";

function previewKind(r: SearchResult): PreviewKind {
  if (r.is_dir) return "icon";
  const ext = r.file_type?.toLowerCase() ?? "";
  if (IMAGE_EXTS.includes(ext)) return "image";
  if (ext === "pdf") return "pdf";
  if (MD_EXTS.includes(ext)) return "markdown";
  if (TEXT_EXTS.includes(ext)) return "text";
  if (CODE_EXTS.includes(ext)) return "code";
  return "icon";
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

    readTextFile(result.path)
      .then((content) => {
        if (cancelled) return;
        // Truncate to avoid melting the UI on huge files
        if (content.length > MAX_PREVIEW_BYTES) {
          setTextContent(content.slice(0, MAX_PREVIEW_BYTES) + "\n\n… (truncated)");
        } else {
          setTextContent(content);
        }
      })
      .catch((e) => {
        if (cancelled) return;
        setTextError(String(e));
      });

    return () => { cancelled = true; };
  }, [result?.path, kind]);

  if (!result) {
    return (
      <div className="h-full flex items-center justify-center text-neutral-700 text-sm">
        Select a file to preview
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Preview area */}
      <div className="flex-1 min-h-0 overflow-hidden bg-neutral-900/40">
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
          <div className="h-full overflow-y-auto p-5 text-sm prose-invert prose-sm max-w-none">
            <div className="markdown-preview">
              <ReactMarkdown>{textContent}</ReactMarkdown>
            </div>
          </div>
        )}

        {(kind === "text" || kind === "code") && textContent !== null && (
          <pre className="h-full overflow-auto p-4 text-xs font-mono text-neutral-300 whitespace-pre">
            {textContent}
          </pre>
        )}

        {(kind === "text" || kind === "code" || kind === "markdown") &&
          textContent === null && !textError && (
            <div className="h-full flex items-center justify-center text-xs text-neutral-600">
              loading…
            </div>
          )}

        {textError && (
          <div className="h-full flex items-center justify-center p-4 text-xs text-red-400 text-center">
            failed to read: {textError}
          </div>
        )}

        {kind === "icon" && (
          <div className="h-full flex items-center justify-center p-4">
            <div className="flex flex-col items-center gap-4 text-center">
              {bigIconFor(result)}
              <div className="text-sm text-neutral-300 max-w-[90%] truncate">
                {result.filename}
              </div>
              {result.content_snippet && (
                <div className="text-xs text-neutral-500 italic max-w-[90%] line-clamp-6 whitespace-pre-wrap">
                  {result.content_snippet}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Metadata */}
      <div className="border-t border-neutral-800 px-4 py-3 text-xs space-y-1.5 shrink-0">
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
      <span className="text-neutral-600">{label}</span>
      <span className={`text-neutral-300 truncate ${mono ? "font-mono text-[11px]" : ""}`} title={value}>
        {value}
      </span>
    </div>
  );
}
