import { describe, it, expect } from "vitest";
import { formatSize, formatDate, previewKind } from "./format";
import type { SearchResult } from "../types";

function makeResult(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    path: "/test/file.txt",
    filename: "file.txt",
    score: 1.0,
    match_type: "exact",
    size_bytes: null,
    modified: null,
    file_type: null,
    content_snippet: null,
    is_dir: false,
    interactions: 0,
    ...overrides,
  };
}

describe("formatSize", () => {
  it("returns dash for null", () => {
    expect(formatSize(null)).toBe("—");
  });

  it("formats 0 bytes", () => {
    expect(formatSize(0)).toBe("0 B");
  });

  it("formats bytes under 1 KB", () => {
    expect(formatSize(500)).toBe("500 B");
  });

  it("formats exactly 1024 bytes as KB", () => {
    expect(formatSize(1024)).toBe("1.0 KB");
  });

  it("formats kilobytes", () => {
    expect(formatSize(2560)).toBe("2.5 KB");
  });

  it("formats megabytes", () => {
    expect(formatSize(1.5 * 1024 ** 2)).toBe("1.5 MB");
  });

  it("formats gigabytes", () => {
    expect(formatSize(2.3 * 1024 ** 3)).toBe("2.3 GB");
  });

  it("formats large gigabyte values", () => {
    expect(formatSize(100 * 1024 ** 3)).toBe("100.0 GB");
  });
});

describe("formatDate", () => {
  it("returns dash for null", () => {
    expect(formatDate(null)).toBe("—");
  });

  it("returns dash for empty string", () => {
    expect(formatDate("")).toBe("—");
  });

  it("formats today as 'Today at HH:MM'", () => {
    const now = new Date();
    const result = formatDate(now.toISOString());
    expect(result).toMatch(/^Today at \d{1,2}:\d{2}\s*(AM|PM)?$/);
  });

  it("formats yesterday as 'Yesterday at HH:MM'", () => {
    const yesterday = new Date();
    yesterday.setDate(yesterday.getDate() - 1);
    yesterday.setHours(14, 30, 0, 0);
    const result = formatDate(yesterday.toISOString());
    expect(result).toMatch(/^Yesterday at \d{1,2}:\d{2}\s*(AM|PM)?$/);
  });

  it("formats older dates with full date", () => {
    const old = new Date("2024-01-15T10:30:00Z");
    const result = formatDate(old.toISOString());
    // Should contain year, month abbreviation, day, and time
    expect(result).toMatch(/2024/);
    expect(result).toMatch(/Jan/);
  });
});

describe("previewKind", () => {
  it("returns icon for directory", () => {
    expect(previewKind(makeResult({ is_dir: true }))).toBe("icon");
  });

  it("returns image for png", () => {
    expect(previewKind(makeResult({ file_type: "png" }))).toBe("image");
  });

  it("returns image for jpg", () => {
    expect(previewKind(makeResult({ file_type: "jpg" }))).toBe("image");
  });

  it("returns image for svg", () => {
    expect(previewKind(makeResult({ file_type: "svg" }))).toBe("image");
  });

  it("returns pdf for pdf", () => {
    expect(previewKind(makeResult({ file_type: "pdf" }))).toBe("pdf");
  });

  it("returns markdown for md", () => {
    expect(previewKind(makeResult({ file_type: "md" }))).toBe("markdown");
  });

  it("returns markdown for mdx", () => {
    expect(previewKind(makeResult({ file_type: "mdx" }))).toBe("markdown");
  });

  it("returns text for txt", () => {
    expect(previewKind(makeResult({ file_type: "txt" }))).toBe("text");
  });

  it("returns text for json", () => {
    expect(previewKind(makeResult({ file_type: "json" }))).toBe("text");
  });

  it("returns text for csv", () => {
    expect(previewKind(makeResult({ file_type: "csv" }))).toBe("text");
  });

  it("returns code for rs", () => {
    expect(previewKind(makeResult({ file_type: "rs" }))).toBe("code");
  });

  it("returns code for tsx", () => {
    expect(previewKind(makeResult({ file_type: "tsx" }))).toBe("code");
  });

  it("returns code for py", () => {
    expect(previewKind(makeResult({ file_type: "py" }))).toBe("code");
  });

  it("returns icon for unknown extension", () => {
    expect(previewKind(makeResult({ file_type: "xyz" }))).toBe("icon");
  });

  it("returns icon for null file_type", () => {
    expect(previewKind(makeResult({ file_type: null }))).toBe("icon");
  });

  it("is case-insensitive for file extensions", () => {
    expect(previewKind(makeResult({ file_type: "PNG" }))).toBe("image");
    expect(previewKind(makeResult({ file_type: "Md" }))).toBe("markdown");
    expect(previewKind(makeResult({ file_type: "RS" }))).toBe("code");
  });
});
