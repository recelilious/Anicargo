import { webDarkTheme, webLightTheme } from "@fluentui/react-components";

const fontFamilyBase = "\"JetBrains Mono\", monospace";

type FluentTheme = typeof webLightTheme;

function createLightTheme(): FluentTheme {
  return {
    ...webLightTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#5D4F57",
    colorBrandForeground2: "#7B6971",
    colorBrandForeground2Hover: "#52454D",
    colorBrandForeground2Pressed: "#43383E",
    colorBrandForegroundLink: "#5D4F57",
    colorBrandForegroundLinkHover: "#4E4249",
    colorBrandForegroundLinkPressed: "#43383E",
    colorBrandForegroundOnLight: "#5D4F57",
    colorBrandForegroundOnLightHover: "#4E4249",
    colorBrandForegroundOnLightPressed: "#43383E",
    colorBrandBackground: "#5D4F57",
    colorBrandBackgroundHover: "#50444B",
    colorBrandBackgroundPressed: "#43383E",
    colorBrandBackgroundSelected: "#50444B",
    colorBrandBackground2: "#FDE7DE",
    colorBrandBackground2Hover: "#F8DACE",
    colorBrandBackground2Pressed: "#F2CCBF",
    colorBrandBackground3Static: "#FAC7B7",
    colorBrandBackground4Static: "#FFE9E1",
    colorBrandStroke1: "#9D8991",
    colorBrandStroke2: "#7B6971",
    colorNeutralForeground1: "#241D22",
    colorNeutralForeground2: "#5D4F57",
    colorNeutralForeground3: "#7B6971",
    colorNeutralForeground4: "#98868E",
    colorNeutralBackground1: "#FFF9F6",
    colorNeutralBackground2: "#F7EEEA",
    colorNeutralBackground3: "#F2E4DF",
    colorNeutralBackground4: "#EAD8D1",
    colorNeutralBackground5: "#E2CBC4",
    colorNeutralStroke1: "#D8C3BC",
    colorNeutralStroke2: "#C8B1AA",
    colorNeutralStrokeAccessible: "#8B7680",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "#F4E6E1",
    colorSubtleBackgroundPressed: "#EEDAD4",
    colorSubtleBackgroundLightAlphaHover: "rgba(93, 79, 87, 0.08)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(93, 79, 87, 0.14)",
    colorTransparentStroke: "rgba(93, 79, 87, 0.18)"
  };
}

function createDarkTheme(): FluentTheme {
  return {
    ...webDarkTheme,
    fontFamilyBase,
    fontFamilyMonospace: fontFamilyBase,
    fontFamilyNumeric: fontFamilyBase,
    colorBrandForeground1: "#FAC7B7",
    colorBrandForeground2: "#FFD8CC",
    colorBrandForeground2Hover: "#FFE4DB",
    colorBrandForeground2Pressed: "#F3C2B2",
    colorBrandForegroundLink: "#FAC7B7",
    colorBrandForegroundLinkHover: "#FFE0D5",
    colorBrandForegroundLinkPressed: "#EFB39E",
    colorBrandForegroundOnLight: "#5D4F57",
    colorBrandForegroundOnLightHover: "#443840",
    colorBrandForegroundOnLightPressed: "#362C32",
    colorBrandBackground: "#FAC7B7",
    colorBrandBackgroundHover: "#F0B7A3",
    colorBrandBackgroundPressed: "#E0A28D",
    colorBrandBackgroundSelected: "#F0B7A3",
    colorBrandBackground2: "#5D4F57",
    colorBrandBackground2Hover: "#6B5C65",
    colorBrandBackground2Pressed: "#4C4047",
    colorBrandBackground3Static: "#7B6971",
    colorBrandBackground4Static: "#2F252B",
    colorBrandStroke1: "#B78F82",
    colorBrandStroke2: "#FAC7B7",
    colorNeutralForeground1: "#FFF1EC",
    colorNeutralForeground2: "#E4C9C0",
    colorNeutralForeground3: "#C4AAA1",
    colorNeutralForeground4: "#9E8881",
    colorNeutralBackground1: "#171215",
    colorNeutralBackground2: "#21191D",
    colorNeutralBackground3: "#2B2127",
    colorNeutralBackground4: "#372A31",
    colorNeutralBackground5: "#45353E",
    colorNeutralStroke1: "#514049",
    colorNeutralStroke2: "#6A5660",
    colorNeutralStrokeAccessible: "#A98F87",
    colorSubtleBackground: "transparent",
    colorSubtleBackgroundHover: "rgba(250, 199, 183, 0.10)",
    colorSubtleBackgroundPressed: "rgba(250, 199, 183, 0.16)",
    colorSubtleBackgroundLightAlphaHover: "rgba(250, 199, 183, 0.10)",
    colorSubtleBackgroundLightAlphaPressed: "rgba(250, 199, 183, 0.16)",
    colorTransparentStroke: "rgba(250, 199, 183, 0.18)"
  };
}

export const anicargoThemes = {
  light: createLightTheme(),
  dark: createDarkTheme()
} as const;

export type ResolvedAppearance = keyof typeof anicargoThemes;
export type ThemePreference = ResolvedAppearance | "system";
