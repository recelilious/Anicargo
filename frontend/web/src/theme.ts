import { webDarkTheme, webLightTheme } from "@fluentui/react-components";

const fontFamilyBase = "\"JetBrains Mono Variable\", \"Maple Mono CN\", monospace";

type FluentTheme = typeof webLightTheme;

function createLightTheme(): FluentTheme {
  return {
    ...webLightTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#4B2C23",
    colorBrandForeground2: "#6C4A3E",
    colorBrandForeground2Hover: "#5D3D33",
    colorBrandForeground2Pressed: "#4B2C23",
    colorBrandForegroundLink: "#4B2C23",
    colorBrandForegroundLinkHover: "#5D3D33",
    colorBrandForegroundLinkPressed: "#3A221B",
    colorBrandForegroundOnLight: "#4B2C23",
    colorBrandForegroundOnLightHover: "#5D3D33",
    colorBrandForegroundOnLightPressed: "#3A221B",
    colorBrandBackground: "#4B2C23",
    colorBrandBackgroundHover: "#5C392F",
    colorBrandBackgroundPressed: "#3C241D",
    colorBrandBackgroundSelected: "#5C392F",
    colorBrandBackground2: "#E8DCCE",
    colorBrandBackground2Hover: "#DFD0C1",
    colorBrandBackground2Pressed: "#D3C2B0",
    colorBrandBackground3Static: "#D6C3B2",
    colorBrandBackground4Static: "#F3E9E0",
    colorBrandStroke1: "#967D6F",
    colorBrandStroke2: "#6C4A3E",
    colorNeutralForeground1: "#251713",
    colorNeutralForeground2: "#4B2C23",
    colorNeutralForeground3: "#736259",
    colorNeutralForeground4: "#9A867B",
    colorNeutralBackground1: "#FFFaf6",
    colorNeutralBackground2: "#F5EEE8",
    colorNeutralBackground3: "#EEE3D8",
    colorNeutralBackground4: "#E6D7CA",
    colorNeutralBackground5: "#D9C7B7",
    colorNeutralStroke1: "#DED0C4",
    colorNeutralStroke2: "#C8B6A7",
    colorNeutralStrokeAccessible: "#8B7365",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "#F0E5DC",
    colorSubtleBackgroundPressed: "#E7D9CF",
    colorSubtleBackgroundLightAlphaHover: "rgba(75, 44, 35, 0.08)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(75, 44, 35, 0.14)",
    colorTransparentStroke: "rgba(75, 44, 35, 0.16)"
  };
}

function createDarkTheme(): FluentTheme {
  return {
    ...webDarkTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#E7D8CC",
    colorBrandForeground2: "#F3E5D8",
    colorBrandForeground2Hover: "#FBF1E9",
    colorBrandForeground2Pressed: "#D6C3B2",
    colorBrandForegroundLink: "#E7D8CC",
    colorBrandForegroundLinkHover: "#FBF1E9",
    colorBrandForegroundLinkPressed: "#D6C3B2",
    colorBrandForegroundOnLight: "#4B2C23",
    colorBrandForegroundOnLightHover: "#5C392F",
    colorBrandForegroundOnLightPressed: "#3C241D",
    colorBrandBackground: "#8B6A5B",
    colorBrandBackgroundHover: "#9A7667",
    colorBrandBackgroundPressed: "#77584B",
    colorBrandBackgroundSelected: "#9A7667",
    colorBrandBackground2: "#2C221D",
    colorBrandBackground2Hover: "#342823",
    colorBrandBackground2Pressed: "#241B17",
    colorBrandBackground3Static: "#3A2C26",
    colorBrandBackground4Static: "#191412",
    colorBrandStroke1: "#9A7B6B",
    colorBrandStroke2: "#D6C3B2",
    colorNeutralForeground1: "#F3E8DE",
    colorNeutralForeground2: "#D7C6BA",
    colorNeutralForeground3: "#B6A394",
    colorNeutralForeground4: "#8F7B6D",
    colorNeutralBackground1: "#1D1A18",
    colorNeutralBackground2: "#23201D",
    colorNeutralBackground3: "#2B2724",
    colorNeutralBackground4: "#34302C",
    colorNeutralBackground5: "#413A35",
    colorNeutralStroke1: "#37302C",
    colorNeutralStroke2: "#54453C",
    colorNeutralStrokeAccessible: "#B29D8E",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(214, 195, 178, 0.08)",
    colorSubtleBackgroundPressed: "rgba(214, 195, 178, 0.14)",
    colorSubtleBackgroundLightAlphaHover: "rgba(214, 195, 178, 0.08)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(214, 195, 178, 0.14)",
    colorTransparentStroke: "rgba(214, 195, 178, 0.14)"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
