import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { useSession } from "./session";

const DEFAULT_UI_SCALE_LEVEL = 2;
const UI_SCALE_SUFFIX = "ui_scale_level";

export type UiScaleLevel = 1 | 2 | 3 | 4 | 5;

type UiScaleProfile = {
  fontScale: number;
  subjectCardMinWidth: number;
  subjectCardFixedWidth: number;
  subjectCardHeight: number;
  subjectCardPosterHeight: number;
  sidebarWidth: number;
  railPaddingX: number;
  railPaddingY: number;
  railGap: number;
  navGap: number;
  brandGap: number;
  brandLogoWidth: number;
  brandLogoHeight: number;
  contentPaddingX: number;
  contentPaddingTop: number;
  contentPaddingBottom: number;
};

const UI_SCALE_PROFILES: Record<UiScaleLevel, UiScaleProfile> = {
  1: {
    fontScale: 0.84,
    subjectCardMinWidth: 180,
    subjectCardFixedWidth: 180,
    subjectCardHeight: 356,
    subjectCardPosterHeight: 205,
    sidebarWidth: 192,
    railPaddingX: 12,
    railPaddingY: 18,
    railGap: 14,
    navGap: 6,
    brandGap: 8,
    brandLogoWidth: 28,
    brandLogoHeight: 34,
    contentPaddingX: 20,
    contentPaddingTop: 18,
    contentPaddingBottom: 28,
  },
  2: {
    fontScale: 1,
    subjectCardMinWidth: 210,
    subjectCardFixedWidth: 210,
    subjectCardHeight: 414,
    subjectCardPosterHeight: 238,
    sidebarWidth: 220,
    railPaddingX: 14,
    railPaddingY: 22,
    railGap: 18,
    navGap: 8,
    brandGap: 10,
    brandLogoWidth: 36,
    brandLogoHeight: 44,
    contentPaddingX: 28,
    contentPaddingTop: 24,
    contentPaddingBottom: 40,
  },
  3: {
    fontScale: 1.18,
    subjectCardMinWidth: 246,
    subjectCardFixedWidth: 246,
    subjectCardHeight: 484,
    subjectCardPosterHeight: 278,
    sidebarWidth: 254,
    railPaddingX: 16,
    railPaddingY: 26,
    railGap: 20,
    navGap: 10,
    brandGap: 12,
    brandLogoWidth: 42,
    brandLogoHeight: 52,
    contentPaddingX: 32,
    contentPaddingTop: 28,
    contentPaddingBottom: 44,
  },
  4: {
    fontScale: 1.36,
    subjectCardMinWidth: 282,
    subjectCardFixedWidth: 282,
    subjectCardHeight: 556,
    subjectCardPosterHeight: 318,
    sidebarWidth: 290,
    railPaddingX: 18,
    railPaddingY: 30,
    railGap: 22,
    navGap: 12,
    brandGap: 14,
    brandLogoWidth: 48,
    brandLogoHeight: 60,
    contentPaddingX: 36,
    contentPaddingTop: 32,
    contentPaddingBottom: 48,
  },
  5: {
    fontScale: 1.54,
    subjectCardMinWidth: 318,
    subjectCardFixedWidth: 318,
    subjectCardHeight: 626,
    subjectCardPosterHeight: 358,
    sidebarWidth: 326,
    railPaddingX: 20,
    railPaddingY: 34,
    railGap: 24,
    navGap: 14,
    brandGap: 16,
    brandLogoWidth: 54,
    brandLogoHeight: 68,
    contentPaddingX: 40,
    contentPaddingTop: 36,
    contentPaddingBottom: 56,
  },
};

type UiPreferencesContextValue = {
  uiScaleLevel: UiScaleLevel;
  uiScaleProfile: UiScaleProfile;
  setUiScaleLevel: (value: number) => void;
};

const UiPreferencesContext = createContext<UiPreferencesContextValue | null>(null);

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

function parseUiScaleLevel(value: string | null): UiScaleLevel | null {
  const parsed = Number(value);

  if (parsed >= 1 && parsed <= 5 && Number.isInteger(parsed)) {
    return parsed as UiScaleLevel;
  }

  return null;
}

function clampUiScaleLevel(value: number): UiScaleLevel {
  if (value <= 1) {
    return 1;
  }

  if (value >= 5) {
    return 5;
  }

  return Math.round(value) as UiScaleLevel;
}

function buildUiScaleStorageKey(userId: number | null) {
  if (userId == null) {
    return null;
  }

  return `anicargo.user.${userId}.${UI_SCALE_SUFFIX}`;
}

export function UiPreferencesProvider({ children }: { children: ReactNode }) {
  const { bootstrap } = useSession();
  const viewer = bootstrap?.viewer;
  const userStorageKey =
    viewer?.kind === "user" ? buildUiScaleStorageKey(viewer.id) : null;
  const [uiScaleLevel, setUiScaleLevelState] = useState<UiScaleLevel>(DEFAULT_UI_SCALE_LEVEL);
  const previousStorageKeyRef = useRef<string | null>(userStorageKey);
  const skipNextPersistRef = useRef(false);

  useEffect(() => {
    if (previousStorageKeyRef.current === userStorageKey) {
      return;
    }

    previousStorageKeyRef.current = userStorageKey;
    skipNextPersistRef.current = true;

    if (!userStorageKey) {
      return;
    }

    const storedLevel = parseUiScaleLevel(safeLocalStorageGet(userStorageKey));
    if (storedLevel != null) {
      setUiScaleLevelState(storedLevel);
      return;
    }

    safeLocalStorageSet(userStorageKey, String(uiScaleLevel));
  }, [uiScaleLevel, userStorageKey]);

  useEffect(() => {
    if (skipNextPersistRef.current) {
      skipNextPersistRef.current = false;
      return;
    }

    if (!userStorageKey) {
      return;
    }

    safeLocalStorageSet(userStorageKey, String(uiScaleLevel));
  }, [uiScaleLevel, userStorageKey]);

  const uiScaleProfile = useMemo(() => UI_SCALE_PROFILES[uiScaleLevel], [uiScaleLevel]);

  useEffect(() => {
    const root = document.documentElement;

    root.style.setProperty("--app-ui-font-scale", String(uiScaleProfile.fontScale));
    root.style.setProperty("--app-subject-card-min-width", `${uiScaleProfile.subjectCardMinWidth}px`);
    root.style.setProperty("--app-subject-card-fixed-width", `${uiScaleProfile.subjectCardFixedWidth}px`);
    root.style.setProperty("--app-subject-card-height", `${uiScaleProfile.subjectCardHeight}px`);
    root.style.setProperty("--app-subject-card-poster-height", `${uiScaleProfile.subjectCardPosterHeight}px`);
    root.style.setProperty("--app-sidebar-width", `${uiScaleProfile.sidebarWidth}px`);
    root.style.setProperty("--app-rail-padding-x", `${uiScaleProfile.railPaddingX}px`);
    root.style.setProperty("--app-rail-padding-y", `${uiScaleProfile.railPaddingY}px`);
    root.style.setProperty("--app-rail-gap", `${uiScaleProfile.railGap}px`);
    root.style.setProperty("--app-nav-gap", `${uiScaleProfile.navGap}px`);
    root.style.setProperty("--app-brand-gap", `${uiScaleProfile.brandGap}px`);
    root.style.setProperty("--app-brand-logo-width", `${uiScaleProfile.brandLogoWidth}px`);
    root.style.setProperty("--app-brand-logo-height", `${uiScaleProfile.brandLogoHeight}px`);
    root.style.setProperty("--app-content-padding-x", `${uiScaleProfile.contentPaddingX}px`);
    root.style.setProperty("--app-content-padding-top", `${uiScaleProfile.contentPaddingTop}px`);
    root.style.setProperty("--app-content-padding-bottom", `${uiScaleProfile.contentPaddingBottom}px`);
  }, [uiScaleProfile]);

  const value: UiPreferencesContextValue = {
    uiScaleLevel,
    uiScaleProfile,
    setUiScaleLevel: (nextValue: number) => {
      setUiScaleLevelState(clampUiScaleLevel(nextValue));
    },
  };

  return <UiPreferencesContext.Provider value={value}>{children}</UiPreferencesContext.Provider>;
}

export function useUiPreferences() {
  const context = useContext(UiPreferencesContext);

  if (!context) {
    throw new Error("UI preferences context is unavailable");
  }

  return context;
}
