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
};

const UI_SCALE_PROFILES: Record<UiScaleLevel, UiScaleProfile> = {
  1: {
    fontScale: 0.92,
    subjectCardMinWidth: 192,
    subjectCardFixedWidth: 192,
    subjectCardHeight: 388,
    subjectCardPosterHeight: 222,
  },
  2: {
    fontScale: 1,
    subjectCardMinWidth: 210,
    subjectCardFixedWidth: 210,
    subjectCardHeight: 414,
    subjectCardPosterHeight: 238,
  },
  3: {
    fontScale: 1.08,
    subjectCardMinWidth: 228,
    subjectCardFixedWidth: 228,
    subjectCardHeight: 442,
    subjectCardPosterHeight: 254,
  },
  4: {
    fontScale: 1.16,
    subjectCardMinWidth: 246,
    subjectCardFixedWidth: 246,
    subjectCardHeight: 470,
    subjectCardPosterHeight: 270,
  },
  5: {
    fontScale: 1.24,
    subjectCardMinWidth: 264,
    subjectCardFixedWidth: 264,
    subjectCardHeight: 498,
    subjectCardPosterHeight: 286,
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
