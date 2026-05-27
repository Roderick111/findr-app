import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ActionsPanel } from "./ActionsPanel";
import type { SearchResult } from "../types";

function makeResult(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    path: "/test/file.txt",
    filename: "file.txt",
    score: 1.0,
    match_type: "exact",
    size_bytes: 1024,
    modified: null,
    file_type: "txt",
    content_snippet: null,
    is_dir: false,
    interactions: 0,
    ...overrides,
  };
}

function makeHandlers() {
  return {
    onOpen: vi.fn(),
    onReveal: vi.fn(),
    onCopyPath: vi.fn(),
    onCopyFilename: vi.fn(),
    onTrash: vi.fn(),
    onSettings: vi.fn(),
    onClose: vi.fn(),
  };
}

describe("ActionsPanel", () => {
  it("renders the filename in the header", () => {
    render(
      <ActionsPanel
        result={makeResult({ filename: "README.md" })}
        {...makeHandlers()}
      />,
    );
    expect(screen.getByText("README.md")).toBeInTheDocument();
  });

  it("renders all action labels", () => {
    render(<ActionsPanel result={makeResult()} {...makeHandlers()} />);

    expect(screen.getByText("Open")).toBeInTheDocument();
    expect(screen.getByText("Reveal in Finder")).toBeInTheDocument();
    expect(screen.getByText("Copy Path")).toBeInTheDocument();
    expect(screen.getByText("Copy Filename")).toBeInTheDocument();
    expect(screen.getByText("Move to Trash")).toBeInTheDocument();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("calls handler and onClose when action button is clicked", async () => {
    const user = userEvent.setup();
    const handlers = makeHandlers();
    render(<ActionsPanel result={makeResult()} {...handlers} />);

    await user.click(screen.getByText("Open"));
    expect(handlers.onOpen).toHaveBeenCalledOnce();
    expect(handlers.onClose).toHaveBeenCalledOnce();
  });

  it("calls onClose when Escape is pressed", async () => {
    const user = userEvent.setup();
    const handlers = makeHandlers();
    render(<ActionsPanel result={makeResult()} {...handlers} />);

    await user.keyboard("{Escape}");
    expect(handlers.onClose).toHaveBeenCalledOnce();
  });

  it("renders 6 action buttons", () => {
    render(<ActionsPanel result={makeResult()} {...makeHandlers()} />);
    // 6 actions: open, reveal, copy-path, copy-name, trash, settings
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(6);
  });

  it("displays keyboard shortcut keys", () => {
    render(<ActionsPanel result={makeResult()} {...makeHandlers()} />);
    // The Enter key shortcut for Open
    const kbds = screen.getAllByText("↵"); // ↵
    expect(kbds.length).toBeGreaterThanOrEqual(1);
  });

  it("calls onClose when clicking outside panel", async () => {
    const user = userEvent.setup();
    const handlers = makeHandlers();
    render(
      <div>
        <div data-testid="outside">outside</div>
        <ActionsPanel result={makeResult()} {...handlers} />
      </div>,
    );

    await user.click(screen.getByTestId("outside"));
    expect(handlers.onClose).toHaveBeenCalled();
  });
});
