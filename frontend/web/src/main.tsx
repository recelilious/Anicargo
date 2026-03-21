import React from "react";
import ReactDOM from "react-dom/client";
import { FluentProvider } from "@fluentui/react-components";
import { BrowserRouter } from "react-router-dom";

import App from "./App";
import { AppearanceProvider, useAppearance } from "./appearance";
import { SessionProvider } from "./session";
import "./styles.css";

function AppRoot() {
  const { fluentTheme } = useAppearance();

  return (
    <FluentProvider theme={fluentTheme} style={{ minHeight: "100vh" }}>
      <SessionProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </SessionProvider>
    </FluentProvider>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AppearanceProvider>
      <AppRoot />
    </AppearanceProvider>
  </React.StrictMode>
);
