import type { SearchResult } from "../types";

export const IMAGE_EXTS = ["png", "jpg", "jpeg", "gif", "webp", "heic", "bmp", "svg"];
export const TEXT_EXTS = ["txt", "log", "csv", "json", "jsonl", "yml", "yaml", "xml", "toml", "ini", "env"];
export const MD_EXTS = ["md", "markdown", "mdx"];
export const CODE_EXTS = [
  "rs", "ts", "tsx", "js", "jsx", "py", "go", "swift", "java", "c", "cpp", "h", "hpp",
  "rb", "php", "sh", "bash", "zsh", "css", "scss", "vue", "svelte", "html", "sql",
  "kt", "dart", "lua", "r", "pl", "scala", "ex", "exs", "clj",
];
export const VIDEO_EXTS = ["mp4", "mov", "avi", "mkv", "webm"];
export const AUDIO_EXTS = ["mp3", "wav", "m4a", "flac", "ogg"];
export const ARCHIVE_EXTS = ["zip", "tar", "gz", "rar", "7z"];

export function formatSize(bytes: number | null): string {
  if (bytes === null) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

export function formatDate(iso: string | null): string {
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

export type PreviewKind = "image" | "pdf" | "markdown" | "text" | "code" | "icon";

export function previewKind(r: SearchResult): PreviewKind {
  if (r.is_dir) return "icon";
  const ext = r.file_type?.toLowerCase() ?? "";
  if (IMAGE_EXTS.includes(ext)) return "image";
  if (ext === "pdf") return "pdf";
  if (MD_EXTS.includes(ext)) return "markdown";
  if (TEXT_EXTS.includes(ext)) return "text";
  if (CODE_EXTS.includes(ext)) return "code";
  return "icon";
}
