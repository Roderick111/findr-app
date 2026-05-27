import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { open } from "@tauri-apps/plugin-fs";
import { Preview } from "./Preview";
import type { SearchResult } from "../types";

const mockOpen = vi.mocked(open);

function makeFileHandle(content: string) {
  const encoded = new TextEncoder().encode(content);
  return {
    read: vi.fn((buf: Uint8Array) => {
      const len = Math.min(encoded.length, buf.length);
      buf.set(encoded.slice(0, len));
      return Promise.resolve(len);
    }),
    close: vi.fn(() => Promise.resolve()),
  };
}

function makeResult(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    path: "/test/file.txt",
    filename: "file.txt",
    score: 1.0,
    match_type: "exact",
    size_bytes: 1024,
    modified: "2024-01-15T10:30:00Z",
    file_type: "txt",
    content_snippet: null,
    is_dir: false,
    interactions: 0,
    ...overrides,
  };
}

describe("Preview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default: open returns a handle that never resolves read (loading state)
    mockOpen.mockReturnValue(new Promise(() => {}) as any);
  });

  it("shows placeholder when result is null", () => {
    render(<Preview result={null} />);
    expect(screen.getByText("Select a file to preview")).toBeInTheDocument();
  });

  it("shows metadata rows for a file result", () => {
    render(<Preview result={makeResult({ path: "/foo/bar.txt", file_type: "txt", size_bytes: 2048 })} />);
    expect(screen.getByText("Path")).toBeInTheDocument();
    expect(screen.getByText("/foo/bar.txt")).toBeInTheDocument();
    expect(screen.getByText("Type")).toBeInTheDocument();
    expect(screen.getByText("TXT")).toBeInTheDocument();
    expect(screen.getByText("Size")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
  });

  it("shows 'Folder' as type for directories", () => {
    render(<Preview result={makeResult({ is_dir: true })} />);
    expect(screen.getByText("Folder")).toBeInTheDocument();
  });

  it("shows dash for null file_type", () => {
    render(<Preview result={makeResult({ file_type: null })} />);
    const typeValue = screen.getByText("—");
    expect(typeValue).toBeInTheDocument();
  });

  it("shows interactions count when > 0", () => {
    render(<Preview result={makeResult({ interactions: 5 })} />);
    expect(screen.getByText("Opens")).toBeInTheDocument();
    expect(screen.getByText("5")).toBeInTheDocument();
  });

  it("hides interactions row when 0", () => {
    render(<Preview result={makeResult({ interactions: 0 })} />);
    expect(screen.queryByText("Opens")).not.toBeInTheDocument();
  });

  it("renders icon view for directory", () => {
    render(<Preview result={makeResult({ is_dir: true, filename: "my-folder" })} />);
    expect(screen.getByText("my-folder")).toBeInTheDocument();
  });

  it("shows content_snippet for icon preview kind", () => {
    render(
      <Preview
        result={makeResult({
          is_dir: true,
          filename: "project",
          content_snippet: "A cool project",
        })}
      />,
    );
    expect(screen.getByText("A cool project")).toBeInTheDocument();
  });

  it("shows loading state for text files before content loads", () => {
    render(<Preview result={makeResult({ file_type: "txt" })} />);
    expect(screen.getByText("loading…")).toBeInTheDocument();
  });

  it("shows text content after loading", async () => {
    mockOpen.mockResolvedValue(makeFileHandle("Hello, world!") as any);

    render(<Preview result={makeResult({ file_type: "txt" })} />);

    await waitFor(() => {
      expect(screen.getByText("Hello, world!")).toBeInTheDocument();
    });
  });

  it("shows error when file open fails", async () => {
    mockOpen.mockRejectedValue(new Error("Permission denied"));

    render(<Preview result={makeResult({ file_type: "txt" })} />);

    await waitFor(() => {
      expect(screen.getByText(/failed to read/)).toBeInTheDocument();
      expect(screen.getByText(/Permission denied/)).toBeInTheDocument();
    });
  });

  it("renders image for image file types", () => {
    render(<Preview result={makeResult({ file_type: "png", path: "/test/photo.png", filename: "photo.png" })} />);
    const img = screen.getByRole("img");
    expect(img).toHaveAttribute("alt", "photo.png");
    expect(img).toHaveAttribute("src", "asset://localhost//test/photo.png");
  });
});
