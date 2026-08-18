import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { LicenseGate } from "./components/LicenseGate";
import { Settings } from "./settings/Settings";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ThemeContext, useThemeProvider } from "./hooks/useTheme";
import "./index.css";

const windowLabel = getCurrentWindow().label;

if (windowLabel === "main") {
  document.documentElement.classList.add("legacy-opaque-overlay");
  invoke<boolean>("uses_legacy_opaque_overlay")
    .then((useOpaque) => {
      document.documentElement.classList.toggle("legacy-opaque-overlay", useOpaque);
    })
    .catch(() => {});
}

function Root() {
  const themeCtx = useThemeProvider();

  return (
    <ThemeContext.Provider value={themeCtx}>
      <ErrorBoundary>
        {windowLabel === "settings" ? (
          <Settings />
        ) : (
          <LicenseGate>
            <App />
          </LicenseGate>
        )}
      </ErrorBoundary>
    </ThemeContext.Provider>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
