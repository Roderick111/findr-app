import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { LicenseGate } from "./components/LicenseGate";
import { Settings } from "./settings/Settings";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ThemeContext, useThemeProvider } from "./hooks/useTheme";
import "./index.css";

const windowLabel = getCurrentWindow().label;

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
