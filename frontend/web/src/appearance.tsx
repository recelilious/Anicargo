import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";

import { useSession } from "./session";
import { anicargoThemes, type ResolvedAppearance, type ThemePreference } from "./theme";

const APPEARANCE_KEY = "anicargo.theme_preference";
const APPEARANCE_SUFFIX = "theme_preference";
const FAVICON_ID = "anicargo-favicon";

function buildFaviconHref(color: string) {
  const svg = `
    <svg viewBox="0 0 481 600" fill="none" xmlns="http://www.w3.org/2000/svg">
      <g clip-path="url(#anicargo-favicon-clip)">
        <circle cx="164.5" cy="435.5" r="130" stroke="${color}" stroke-width="69" />
        <rect x="280.707" y="317" width="60" height="158.344" transform="rotate(55 280.707 317)" fill="${color}" />
        <rect x="11" y="34.4146" width="60" height="732.555" transform="rotate(-35 11 34.4146)" fill="${color}" />
        <rect width="60" height="600" fill="${color}" />
      </g>
      <defs>
        <clipPath id="anicargo-favicon-clip">
          <rect width="481" height="600" fill="white" />
        </clipPath>
      </defs>
    </svg>
  `;

  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

function upsertFavicon(href: string) {
  const existing =
    (document.getElementById(FAVICON_ID) as HTMLLinkElement | null) ??
    (document.querySelector('link[rel~="icon"]') as HTMLLinkElement | null);
  const link =
    existing ??
    (() => {
      const created = document.createElement("link");
      created.id = FAVICON_ID;
      created.rel = "icon";
      created.type = "image/svg+xml";
      document.head.appendChild(created);
      return created;
    })();

  link.id = FAVICON_ID;
  link.rel = "icon";
  link.type = "image/svg+xml";
  link.href = href;
}

type AppearanceContextValue = {
  themePreference: ThemePreference;
  resolvedAppearance: ResolvedAppearance;
  setThemePreference: (value: ThemePreference) => void;
  fluentTheme: (typeof anicargoThemes)[ResolvedAppearance];
};

const AppearanceContext = createContext<AppearanceContextValue | null>(null);

function safeLocalStorageGet(key: string) {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeLocalStorageSet(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // ignore storage write failures on restricted browsers
  }
}

function readStoredPreference(key: string): ThemePreference | null {
  const stored = safeLocalStorageGet(key);
  if (stored === "light" || stored === "dark" || stored === "system") {
    return stored;
  }

  return null;
}

function buildAppearanceStorageKey(userId: number | null) {
  if (userId == null) {
    return APPEARANCE_KEY;
  }

  return `anicargo.user.${userId}.${APPEARANCE_SUFFIX}`;
}

function readSystemDarkMode() {
  if (typeof window.matchMedia !== "function") {
    return false;
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function AppearanceProvider({ children }: { children: ReactNode }) {
  const { bootstrap } = useSession();
  const viewer = bootstrap?.viewer;
  const storageKey =
    viewer?.kind === "user" ? buildAppearanceStorageKey(viewer.id) : APPEARANCE_KEY;
  const [themePreference, setThemePreference] = useState<ThemePreference>(
    () => readStoredPreference(APPEARANCE_KEY) ?? "system"
  );
  const [systemPrefersDark, setSystemPrefersDark] = useState(() => readSystemDarkMode());
  const previousStorageKeyRef = useRef(storageKey);
  const skipNextPersistRef = useRef(false);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      setSystemPrefersDark(false);
      return undefined;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    function handleChange(event: MediaQueryListEvent) {
      setSystemPrefersDark(event.matches);
    }

    setSystemPrefersDark(mediaQuery.matches);
    if (typeof mediaQuery.addEventListener === "function") {
      mediaQuery.addEventListener("change", handleChange);
    } else if (typeof mediaQuery.addListener === "function") {
      mediaQuery.addListener(handleChange);
    }

    return () => {
      if (typeof mediaQuery.removeEventListener === "function") {
        mediaQuery.removeEventListener("change", handleChange);
      } else if (typeof mediaQuery.removeListener === "function") {
        mediaQuery.removeListener(handleChange);
      }
    };
  }, []);

  useEffect(() => {
    if (previousStorageKeyRef.current === storageKey) {
      return;
    }

    previousStorageKeyRef.current = storageKey;
    skipNextPersistRef.current = true;

    const storedPreference = readStoredPreference(storageKey);
    if (storedPreference) {
      setThemePreference(storedPreference);
      return;
    }

    safeLocalStorageSet(storageKey, themePreference);
  }, [storageKey, themePreference]);

  useEffect(() => {
    if (skipNextPersistRef.current) {
      skipNextPersistRef.current = false;
      return;
    }

    safeLocalStorageSet(storageKey, themePreference);
  }, [storageKey, themePreference]);

  const resolvedAppearance: ResolvedAppearance =
    themePreference === "system" ? (systemPrefersDark ? "dark" : "light") : themePreference;

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedAppearance;
    document.documentElement.style.colorScheme = resolvedAppearance;
  }, [resolvedAppearance]);

  useEffect(() => {
    const faviconColor = resolvedAppearance === "dark" ? "#B7C7D9" : "#4B2C23";
    upsertFavicon(buildFaviconHref(faviconColor));
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
