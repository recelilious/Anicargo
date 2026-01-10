export interface LocalThemeSettings {
  accent: string;
  accent2: string;
  bgMain: string;
  bgSoft: string;
  ink: string;
  muted: string;
  radiusBase: number;
}

const storageKey = "anicargo.localTheme";

const defaultTheme: LocalThemeSettings = {
  accent: "#3c203f",
  accent2: "#5a365f",
  bgMain: "#231c29",
  bgSoft: "#2c2433",
  ink: "#ececf0",
  muted: "#b6b2c2",
  radiusBase: 14
};

export function loadLocalTheme(): LocalThemeSettings {
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (!raw) {
      return { ...defaultTheme };
    }
    const parsed = JSON.parse(raw) as Partial<LocalThemeSettings>;
    return {
      accent: parsed.accent ?? defaultTheme.accent,
      accent2: parsed.accent2 ?? defaultTheme.accent2,
      bgMain: parsed.bgMain ?? defaultTheme.bgMain,
      bgSoft: parsed.bgSoft ?? defaultTheme.bgSoft,
      ink: parsed.ink ?? defaultTheme.ink,
      muted: parsed.muted ?? defaultTheme.muted,
      radiusBase:
        typeof parsed.radiusBase === "number" && parsed.radiusBase > 0
          ? parsed.radiusBase
          : defaultTheme.radiusBase
    };
  } catch {
    return { ...defaultTheme };
  }
}

export function saveLocalTheme(theme: LocalThemeSettings) {
  window.localStorage.setItem(storageKey, JSON.stringify(theme));
}

export function resetLocalTheme(): LocalThemeSettings {
  window.localStorage.removeItem(storageKey);
  return { ...defaultTheme };
}

export function applyLocalTheme(theme: LocalThemeSettings) {
  const root = document.documentElement;
  root.style.setProperty("--accent", theme.accent);
  root.style.setProperty("--accent-2", theme.accent2);
  root.style.setProperty("--bg-main", theme.bgMain);
  root.style.setProperty("--bg-soft", theme.bgSoft);
  root.style.setProperty("--ink", theme.ink);
  root.style.setProperty("--muted", theme.muted);
  root.style.setProperty("--radius-base", `${theme.radiusBase}px`);

  const accentGlow = toRgba(theme.accent, 0.26);
  const accentGlow2 = toRgba(theme.accent2, 0.28);
  const panelTint = toRgba(theme.accent, 0.18);
  const panelTintStrong = toRgba(theme.accent, 0.3);
  const panelBase = toRgba(theme.bgSoft, 0.88);
  const panelBaseStrong = toRgba(theme.bgSoft, 0.96);

  root.style.setProperty("--accent-glow", accentGlow);
  root.style.setProperty("--accent-glow-2", accentGlow2);
  root.style.setProperty("--panel-tint", panelTint);
  root.style.setProperty("--panel-tint-strong", panelTintStrong);
  root.style.setProperty("--panel-base", panelBase);
  root.style.setProperty("--panel-base-strong", panelBaseStrong);
}

function toRgba(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "").trim();
  if (normalized.length === 3) {
    const r = parseInt(normalized[0] + normalized[0], 16);
    const g = parseInt(normalized[1] + normalized[1], 16);
    const b = parseInt(normalized[2] + normalized[2], 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
  if (normalized.length === 6) {
    const r = parseInt(normalized.slice(0, 2), 16);
    const g = parseInt(normalized.slice(2, 4), 16);
    const b = parseInt(normalized.slice(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }
  return `rgba(0, 0, 0, ${alpha})`;
}
