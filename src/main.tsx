import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import { LicenseGate } from "./components/LicenseGate";
import { Settings } from "./settings/Settings";
import "./index.css";

const windowLabel = getCurrentWindow().label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {windowLabel === "settings" ? (
      <Settings />
    ) : (
      <LicenseGate>
        <App />
      </LicenseGate>
    )}
  </React.StrictMode>,
);
