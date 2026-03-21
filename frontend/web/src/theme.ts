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
    colorBrandForeground2: "#705247",
    colorBrandForeground2Hover: "#5E4339",
    colorBrandForeground2Pressed: "#4B342B",
    colorBrandForegroundLink: "#4B2C23",
    colorBrandForegroundLinkHover: "#5E4339",
    colorBrandForegroundLinkPressed: "#3A221B",
    colorBrandForegroundOnLight: "#4B2C23",
    colorBrandForegroundOnLightHover: "#5E4339",
    colorBrandForegroundOnLightPressed: "#3A221B",
    colorBrandBackground: "#4B2C23",
    colorBrandBackgroundHover: "#5E392F",
    colorBrandBackgroundPressed: "#392119",
    colorBrandBackgroundSelected: "#5E392F",
    colorBrandBackground2: "#E8DACE",
    colorBrandBackground2Hover: "#DFCDBE",
    colorBrandBackground2Pressed: "#D4BFAD",
    colorBrandBackground3Static: "#D6C3B2",
    colorBrandBackground4Static: "#F2E7DD",
    colorBrandStroke1: "#9D8577",
    colorBrandStroke2: "#705247",
    colorNeutralForeground1: "#241511",
    colorNeutralForeground2: "#4B2C23",
    colorNeutralForeground3: "#7A655A",
    colorNeutralForeground4: "#9A857A",
    colorNeutralBackground1: "#FCF7F2",
    colorNeutralBackground2: "#F2E9E1",
    colorNeutralBackground3: "#ECE0D5",
    colorNeutralBackground4: "#E3D3C6",
    colorNeutralBackground5: "#D8C5B4",
    colorNeutralStroke1: "#D0C0B3",
    colorNeutralStroke2: "#C0AC9C",
    colorNeutralStrokeAccessible: "#876C60",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "#F0E4DB",
    colorSubtleBackgroundPressed: "#E7D7CB",
    colorSubtleBackgroundLightAlphaHover: "rgba(75, 44, 35, 0.08)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(75, 44, 35, 0.14)",
    colorTransparentStroke: "rgba(75, 44, 35, 0.18)"
  };
}

function createDarkTheme(): FluentTheme {
  return {
    ...webDarkTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#D6C3B2",
    colorBrandForeground2: "#E7D8CA",
    colorBrandForeground2Hover: "#F2E6DB",
    colorBrandForeground2Pressed: "#D0B9A6",
    colorBrandForegroundLink: "#D6C3B2",
    colorBrandForegroundLinkHover: "#F2E6DB",
    colorBrandForegroundLinkPressed: "#CCB4A0",
    colorBrandForegroundOnLight: "#4B2C23",
    colorBrandForegroundOnLightHover: "#5E392F",
    colorBrandForegroundOnLightPressed: "#3A221B",
    colorBrandBackground: "#D6C3B2",
    colorBrandBackgroundHover: "#C9B29F",
    colorBrandBackgroundPressed: "#B89E89",
    colorBrandBackgroundSelected: "#C9B29F",
    colorBrandBackground2: "#4B2C23",
    colorBrandBackground2Hover: "#603A2F",
    colorBrandBackground2Pressed: "#392119",
    colorBrandBackground3Static: "#6E5044",
    colorBrandBackground4Static: "#221512",
    colorBrandStroke1: "#A48574",
    colorBrandStroke2: "#D6C3B2",
    colorNeutralForeground1: "#F4E9DE",
    colorNeutralForeground2: "#DCCBBB",
    colorNeutralForeground3: "#B8A291",
    colorNeutralForeground4: "#90786C",
    colorNeutralBackground1: "#140D0B",
    colorNeutralBackground2: "#1B120F",
    colorNeutralBackground3: "#261915",
    colorNeutralBackground4: "#32211B",
    colorNeutralBackground5: "#412B23",
    colorNeutralStroke1: "#50372E",
    colorNeutralStroke2: "#6A4C40",
    colorNeutralStrokeAccessible: "#B69D8D",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(214, 195, 178, 0.10)",
    colorSubtleBackgroundPressed: "rgba(214, 195, 178, 0.16)",
    colorSubtleBackgroundLightAlphaHover: "rgba(214, 195, 178, 0.10)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(214, 195, 178, 0.16)",
    colorTransparentStroke: "rgba(214, 195, 178, 0.18)"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
