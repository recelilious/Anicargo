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
    colorBrandForeground1: "#D6C3B2",
    colorBrandForeground2: "#FFF5EC",
    colorBrandForeground2Hover: "#FFFFFF",
    colorBrandForeground2Pressed: "#E8DCCE",
    colorBrandForegroundLink: "#D6C3B2",
    colorBrandForegroundLinkHover: "#FFF5EC",
    colorBrandForegroundLinkPressed: "#E8DCCE",
    colorBrandForegroundOnLight: "#251713",
    colorBrandForegroundOnLightHover: "#1B100D",
    colorBrandForegroundOnLightPressed: "#130A08",
    colorBrandBackground: "#D6C3B2",
    colorBrandBackgroundHover: "#E2D2C3",
    colorBrandBackgroundPressed: "#C7B19F",
    colorBrandBackgroundSelected: "#E2D2C3",
    colorBrandBackground2: "#5A372D",
    colorBrandBackground2Hover: "#664035",
    colorBrandBackground2Pressed: "#4B2C23",
    colorBrandBackground3Static: "#D6C3B2",
    colorBrandBackground4Static: "#2F1D18",
    colorBrandStroke1: "#D6C3B2",
    colorBrandStroke2: "#E8DCCE",
    colorNeutralForeground1: "#FFFAF6",
    colorNeutralForeground2: "#E8DCCE",
    colorNeutralForeground3: "#D6C3B2",
    colorNeutralForeground4: "#BFA695",
    colorNeutralBackground1: "#4B2C23",
    colorNeutralBackground2: "#40261F",
    colorNeutralBackground3: "#362019",
    colorNeutralBackground4: "#301D18",
    colorNeutralBackground5: "#251713",
    colorNeutralStroke1: "#8E7364",
    colorNeutralStroke2: "#B39A8B",
    colorNeutralStrokeAccessible: "#E8DCCE",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(214, 195, 178, 0.18)",
    colorSubtleBackgroundPressed: "rgba(214, 195, 178, 0.26)",
    colorSubtleBackgroundLightAlphaHover: "rgba(214, 195, 178, 0.18)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(214, 195, 178, 0.26)",
    colorTransparentStroke: "rgba(214, 195, 178, 0.24)",
    colorStrokeFocus1: "#251713",
    colorStrokeFocus2: "#D6C3B2"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
