import React from "react";
import ReactDOM from "react-dom/client";
import { FluentProvider } from "@fluentui/react-components";
import { BrowserRouter } from "react-router-dom";
import "@fontsource-variable/jetbrains-mono";
import "@chinese-fonts/maple-mono-cn/dist/MapleMono-CN-Regular/result.css";
import "@chinese-fonts/maple-mono-cn/dist/MapleMono-CN-SemiBold/result.css";

import App from "./App";
import { AppearanceProvider, useAppearance } from "./appearance";
import { LoadingStatusProvider } from "./loading-status";
import { SessionProvider } from "./session";
import { getAnicargoTheme } from "./theme";
import { UiPreferencesProvider, useUiPreferences } from "./ui-preferences";
import "./styles.css";

function AppRoot() {
  const { resolvedAppearance } = useAppearance();
  const { uiScaleProfile } = useUiPreferences();
  const fluentTheme = getAnicargoTheme(resolvedAppearance, uiScaleProfile.fontScale);

  return (
    <FluentProvider theme={fluentTheme} style={{ minHeight: "100vh" }}>
      <LoadingStatusProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </LoadingStatusProvider>
    </FluentProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <SessionProvider>
      <AppearanceProvider>
        <UiPreferencesProvider>
          <AppRoot />
        </UiPreferencesProvider>
      </AppearanceProvider>
    </SessionProvider>
  </React.StrictMode>
);
