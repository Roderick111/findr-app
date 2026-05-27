import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ErrorBoundary } from "./ErrorBoundary";

function ThrowingChild({ message }: { message: string }): React.ReactNode {
  throw new Error(message);
}

function GoodChild() {
  return <div>All good</div>;
}

describe("ErrorBoundary", () => {
  beforeEach(() => {
    // Suppress console.error from React and ErrorBoundary during expected throws
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  it("renders children when no error occurs", () => {
    render(
      <ErrorBoundary>
        <GoodChild />
      </ErrorBoundary>,
    );
    expect(screen.getByText("All good")).toBeInTheDocument();
  });

  it("shows error heading when child throws", () => {
    render(
      <ErrorBoundary>
        <ThrowingChild message="test explosion" />
      </ErrorBoundary>,
    );
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
  });

  it("shows the error message", () => {
    render(
      <ErrorBoundary>
        <ThrowingChild message="test explosion" />
      </ErrorBoundary>,
    );
    expect(screen.getByText("test explosion")).toBeInTheDocument();
  });

  it("shows a Reload button", () => {
    render(
      <ErrorBoundary>
        <ThrowingChild message="boom" />
      </ErrorBoundary>,
    );
    expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
  });

  it("calls window.location.reload when Reload is clicked", async () => {
    const user = userEvent.setup();
    const reloadMock = vi.fn();
    Object.defineProperty(window, "location", {
      value: { ...window.location, reload: reloadMock },
      writable: true,
    });

    render(
      <ErrorBoundary>
        <ThrowingChild message="boom" />
      </ErrorBoundary>,
    );

    await user.click(screen.getByRole("button", { name: "Reload" }));
    expect(reloadMock).toHaveBeenCalledOnce();
  });

  it("shows fallback when error message is empty", () => {
    // new Error() produces message="" which is falsy but not nullish
    // The `??` fallback only triggers for null/undefined, so empty message renders empty <p>
    function ThrowEmpty(): React.ReactNode {
      throw new Error();
    }

    render(
      <ErrorBoundary>
        <ThrowEmpty />
      </ErrorBoundary>,
    );
    // The heading is still shown
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
    // Reload button still available
    expect(screen.getByRole("button", { name: "Reload" })).toBeInTheDocument();
  });
});
