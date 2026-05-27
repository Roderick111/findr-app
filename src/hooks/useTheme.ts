import { createContext, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ThemePreference } from "../types";

interface ThemeContextValue {
  preference: ThemePreference;
  resolved: "light" | "dark";
  setPreference: (p: ThemePreference) => void;
}

export const ThemeContext = createContext<ThemeContextValue>({
  preference: "dark",
  resolved: "dark",
  setPreference: () => {},
});

export function useTheme() {
  return useContext(ThemeContext);
}

function getSystemTheme(): "light" | "dark" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function resolve(pref: ThemePreference): "light" | "dark" {
  return pref === "system" ? getSystemTheme() : pref;
}

function applyTheme(pref: ThemePreference) {
  const r = resolve(pref);
  document.documentElement.setAttribute("data-theme", r);
  return r;
}

export function useThemeProvider() {
  const [preference, setPreferenceState] = useState<ThemePreference>(() => {
    const stored = document.documentElement.getAttribute("data-theme");
    return (stored as ThemePreference) || "dark";
  });
  const [resolved, setResolved] = useState<"light" | "dark">(() => applyTheme("dark"));

  useEffect(() => {
    invoke<string>("get_theme")
      .then((t) => {
        const pref = (t as ThemePreference) || "dark";
        setPreferenceState(pref);
        setResolved(applyTheme(pref));
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen<string>("theme-changed", (event) => {
      const pref = (event.payload as ThemePreference) || "dark";
      setPreferenceState(pref);
      setResolved(applyTheme(pref));
    });
    return () => { unlisten.then((u) => u()); };
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", resolved);
  }, [resolved]);

  useEffect(() => {
    if (preference !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => setResolved(getSystemTheme());
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [preference]);

  const setPreference = (p: ThemePreference) => {
    setPreferenceState(p);
    setResolved(applyTheme(p));
    invoke("set_theme", { theme: p }).catch(() => {});
  };

  return { preference, resolved, setPreference };
}
