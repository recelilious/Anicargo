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
    colorNeutralBackground1: "#FFFAF6",
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
    colorTransparentStroke: "rgba(75, 44, 35, 0.16)",
    colorStrokeFocus1: "#FFF7F1",
    colorStrokeFocus2: "#4B2C23"
  };
}

function createDarkTheme(): FluentTheme {
  return {
    ...webDarkTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#B7C7D9",
    colorBrandForeground2: "#D4DFEB",
    colorBrandForeground2Hover: "#ECF3FB",
    colorBrandForeground2Pressed: "#C3D0DF",
    colorBrandForegroundLink: "#ADC4DD",
    colorBrandForegroundLinkHover: "#D7E3F1",
    colorBrandForegroundLinkPressed: "#B9CBDF",
    colorBrandForegroundOnLight: "#0F141B",
    colorBrandForegroundOnLightHover: "#0B1016",
    colorBrandForegroundOnLightPressed: "#070C12",
    colorBrandBackground: "#334254",
    colorBrandBackgroundHover: "#3D4D62",
    colorBrandBackgroundPressed: "#293748",
    colorBrandBackgroundSelected: "#425368",
    colorBrandBackground2: "#1E2A36",
    colorBrandBackground2Hover: "#263342",
    colorBrandBackground2Pressed: "#18212B",
    colorBrandBackground3Static: "#425368",
    colorBrandBackground4Static: "#141A22",
    colorBrandStroke1: "#5C6E84",
    colorBrandStroke2: "#92A8C1",
    colorNeutralForeground1: "#EDF2F7",
    colorNeutralForeground2: "#CAD3DD",
    colorNeutralForeground3: "#A9B3BF",
    colorNeutralForeground4: "#8893A1",
    colorNeutralBackground1: "#151B22",
    colorNeutralBackground2: "#1B222B",
    colorNeutralBackground3: "#212A35",
    colorNeutralBackground4: "#283241",
    colorNeutralBackground5: "#303B4D",
    colorNeutralStroke1: "#313C4A",
    colorNeutralStroke2: "#455364",
    colorNeutralStrokeAccessible: "#90A2B7",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(183, 199, 217, 0.10)",
    colorSubtleBackgroundPressed: "rgba(183, 199, 217, 0.16)",
    colorSubtleBackgroundLightAlphaHover: "rgba(183, 199, 217, 0.10)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(183, 199, 217, 0.16)",
    colorTransparentStroke: "rgba(183, 199, 217, 0.18)",
    colorStrokeFocus1: "#EDF2F7",
    colorStrokeFocus2: "#7E97B2"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
