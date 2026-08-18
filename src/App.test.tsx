import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

const mockInvoke = vi.mocked(invoke);

describe("App onboarding footer", () => {
  it("shows indexing progress and the global hotkey", async () => {
    mockInvoke.mockImplementation(async (command) => {
      if (command === "get_doctor_report") {
        return { scan_paths: [{ path: "/Users/test", exists: true, custom: false }] } as never;
      }
      if (command === "get_index_activity") {
        return {
          phase: "indexing",
          message: "Preparing search — finding your files…",
          active: true,
        } as never;
      }
      if (command === "get_recent_files") {
        return {
          query: "",
          mode: "recent",
          elapsed_ms: 2,
          total_results: 0,
          results: [],
        } as never;
      }
      return undefined as never;
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Preparing search — finding your files…")).toBeInTheDocument();
    });
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-label",
      "Preparing search — finding your files…",
    );
    expect(screen.getByText(/Shift\+F/)).toBeInTheDocument();
    expect(screen.getByText("open anytime")).toBeInTheDocument();
  });
});
