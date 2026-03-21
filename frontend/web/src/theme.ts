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
    colorBrandForeground1: "#F4E6D9",
    colorBrandForeground2: "#FFF2E7",
    colorBrandForeground2Hover: "#FFF7F1",
    colorBrandForeground2Pressed: "#E1CDBB",
    colorBrandForegroundLink: "#F4E6D9",
    colorBrandForegroundLinkHover: "#FFF7F1",
    colorBrandForegroundLinkPressed: "#E1CDBB",
    colorBrandForegroundOnLight: "#2B1712",
    colorBrandForegroundOnLightHover: "#22120E",
    colorBrandForegroundOnLightPressed: "#190D0A",
    colorBrandBackground: "#6A4337",
    colorBrandBackgroundHover: "#7A5042",
    colorBrandBackgroundPressed: "#5A382D",
    colorBrandBackgroundSelected: "#7A5042",
    colorBrandBackground2: "#5A382D",
    colorBrandBackground2Hover: "#6A4337",
    colorBrandBackground2Pressed: "#4B2C23",
    colorBrandBackground3Static: "#7A5042",
    colorBrandBackground4Static: "#241612",
    colorBrandStroke1: "#B29587",
    colorBrandStroke2: "#D6C3B2",
    colorNeutralForeground1: "#FFF2E7",
    colorNeutralForeground2: "#F2DEC9",
    colorNeutralForeground3: "#D6C3B2",
    colorNeutralForeground4: "#B89F8F",
    colorNeutralBackground1: "#4B2C23",
    colorNeutralBackground2: "#5A382D",
    colorNeutralBackground3: "#3F261F",
    colorNeutralBackground4: "#2F1C16",
    colorNeutralBackground5: "#22130F",
    colorNeutralStroke1: "#8A6D5F",
    colorNeutralStroke2: "#B29587",
    colorNeutralStrokeAccessible: "#E5D1C1",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(214, 195, 178, 0.12)",
    colorSubtleBackgroundPressed: "rgba(214, 195, 178, 0.18)",
    colorSubtleBackgroundLightAlphaHover: "rgba(214, 195, 178, 0.12)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(214, 195, 178, 0.18)",
    colorTransparentStroke: "rgba(214, 195, 178, 0.18)",
    colorStrokeFocus1: "#120B09",
    colorStrokeFocus2: "#D6C3B2"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
