import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

import { anicargoThemes, type ResolvedAppearance, type ThemePreference } from "./theme";

const APPEARANCE_KEY = "anicargo.theme_preference";

type AppearanceContextValue = {
  themePreference: ThemePreference;
  resolvedAppearance: ResolvedAppearance;
  setThemePreference: (value: ThemePreference) => void;
  fluentTheme: (typeof anicargoThemes)[ResolvedAppearance];
};

const AppearanceContext = createContext<AppearanceContextValue | null>(null);

function readStoredPreference(): ThemePreference {
  const stored = window.localStorage.getItem(APPEARANCE_KEY);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }

  return "system";
}

function readSystemDarkMode() {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const [themePreference, setThemePreference] = useState<ThemePreference>(() => readStoredPreference());
  const [systemPrefersDark, setSystemPrefersDark] = useState(() => readSystemDarkMode());

  useEffect(() => {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    function handleChange(event: MediaQueryListEvent) {
      setSystemPrefersDark(event.matches);
    }

    setSystemPrefersDark(mediaQuery.matches);
    mediaQuery.addEventListener("change", handleChange);

    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    window.localStorage.setItem(APPEARANCE_KEY, themePreference);
  }, [themePreference]);

  const resolvedAppearance: ResolvedAppearance =
    themePreference === "system" ? (systemPrefersDark ? "dark" : "light") : themePreference;

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedAppearance;
    document.documentElement.style.colorScheme = resolvedAppearance;
  }, [resolvedAppearance]);

  const value: AppearanceContextValue = {
    themePreference,
    resolvedAppearance,
    setThemePreference,
    fluentTheme: anicargoThemes[resolvedAppearance]
  };

  return <AppearanceContext.Provider value={value}>{children}</AppearanceContext.Provider>;
}

export function useAppearance() {
  const context = useContext(AppearanceContext);

  if (!context) {
    throw new Error("Appearance context is unavailable");
  }

  return context;
}
