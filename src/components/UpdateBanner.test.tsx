import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { check } from "@tauri-apps/plugin-updater";
import { UpdateBanner } from "./UpdateBanner";

const mockCheck = vi.mocked(check);

describe("UpdateBanner", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when no update available", async () => {
    mockCheck.mockResolvedValue(null);
    const { container } = render(<UpdateBanner />);

    // Wait for async check to complete
    await waitFor(() => {
      expect(mockCheck).toHaveBeenCalled();
    });

    // Should render nothing
    expect(container.firstChild).toBeNull();
  });

  it("shows update available message", async () => {
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      date: null,
      body: null,
      downloadAndInstall: vi.fn(),
    } as any);

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Update available: v2.0.0")).toBeInTheDocument();
    });
  });

  it("shows 'Update now' button when update available", async () => {
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      date: null,
      body: null,
      downloadAndInstall: vi.fn(),
    } as any);

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Update now")).toBeInTheDocument();
    });
  });

  it("shows 'Later' dismiss button", async () => {
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      date: null,
      body: null,
      downloadAndInstall: vi.fn(),
    } as any);

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Later")).toBeInTheDocument();
    });
  });

  it("dismisses banner when Later is clicked", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      date: null,
      body: null,
      downloadAndInstall: vi.fn(),
    } as any);

    const { container } = render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Later")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Later"));
    expect(container.firstChild).toBeNull();
  });

  it("shows 'Installing...' during install", async () => {
    const user = userEvent.setup();
    // First call: initial check returns update
    // Second call (on click): returns update that never resolves
    const downloadAndInstall = vi.fn(() => new Promise(() => {})); // never resolves
    mockCheck
      .mockResolvedValueOnce({
        version: "2.0.0",
        date: null,
        body: null,
        downloadAndInstall,
      } as any)
      .mockResolvedValueOnce({
        version: "2.0.0",
        date: null,
        body: null,
        downloadAndInstall,
      } as any);

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Update now")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Update now"));

    await waitFor(() => {
      expect(screen.getByText("Installing...")).toBeInTheDocument();
    });
  });

  it("shows error and Retry on install failure", async () => {
    const user = userEvent.setup();
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error("network"));
    mockCheck
      .mockResolvedValueOnce({
        version: "2.0.0",
        date: null,
        body: null,
        downloadAndInstall,
      } as any)
      .mockResolvedValueOnce({
        version: "2.0.0",
        date: null,
        body: null,
        downloadAndInstall,
      } as any);

    render(<UpdateBanner />);

    await waitFor(() => {
      expect(screen.getByText("Update now")).toBeInTheDocument();
    });

    await user.click(screen.getByText("Update now"));

    await waitFor(() => {
      expect(screen.getByText("Update failed")).toBeInTheDocument();
      expect(screen.getByText("Retry")).toBeInTheDocument();
    });
  });
});
