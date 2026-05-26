import { useCallback, useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export function UpdateBanner() {
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    check()
      .then((update) => {
        if (update) setVersion(update.version);
      })
      .catch(() => {});
  }, []);

  const handleUpdate = useCallback(async () => {
    setInstalling(true);
    try {
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch {
      setInstalling(false);
    }
  }, []);

  if (!version || dismissed) return null;

  return (
    <div className="absolute top-0 inset-x-0 z-50 flex items-center justify-between px-4 py-1.5 bg-blue-900/80 border-b border-blue-700/50 text-xs">
      <span className="text-blue-200">
        Update available: v{version}
      </span>
      <div className="flex gap-3">
        <button
          onClick={handleUpdate}
          disabled={installing}
          className="text-blue-100 hover:text-white font-medium"
        >
          {installing ? "Installing..." : "Update now"}
        </button>
        <button
          onClick={() => setDismissed(true)}
          className="text-blue-400 hover:text-blue-200"
        >
          Later
        </button>
      </div>
    </div>
  );
}
