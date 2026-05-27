import { useCallback, useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export function UpdateBanner() {
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    check()
      .then((update) => {
        if (update) setVersion(update.version);
      })
      .catch((e) => {
        console.error("Update check failed:", e);
      });
  }, []);

  const handleUpdate = useCallback(async () => {
    setInstalling(true);
    setError(null);
    try {
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      } else {
        setInstalling(false);
      }
    } catch (e) {
      console.error("Update install failed:", e);
      setError("Update failed");
      setInstalling(false);
    }
  }, []);

  if (!version || dismissed) return null;

  return (
    <div
      className="absolute top-0 inset-x-0 z-50 flex items-center justify-between px-4 py-1.5 text-xs"
      style={{
        background: "var(--accent)",
        color: "var(--accent-text)",
        borderBottom: "1px solid var(--border)",
      }}
    >
      <span>{error ? error : `Update available: v${version}`}</span>
      <div className="flex gap-3">
        <button
          onClick={handleUpdate}
          disabled={installing}
          className="font-medium opacity-90 hover:opacity-100"
        >
          {installing ? "Installing..." : error ? "Retry" : "Update now"}
        </button>
        <button
          onClick={() => setDismissed(true)}
          className="opacity-70 hover:opacity-100"
        >
          Later
        </button>
      </div>
    </div>
  );
}
