import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import PageWelcome from "./Pages/PageWelcome";
import PageLogin from "./Pages/PageLogin";
import PageLibrary from "./Pages/PageLibrary";
import PageSignup from "./Pages/PageSignup";
import PageSettings from "./Pages/PageSettings";
import PagePlayer from "./Pages/PagePlayer";
import PageCollection from "./Pages/PageCollection";
import PageManagement from "./Pages/PageManagement";
import PageAdmin from "./Pages/PageAdmin";
import PageNotFound from "./Pages/PageNotFound";
import { SessionProvider } from "./session";
import RequireRole from "./Components/RequireRole";

export default function App() {
  return (
    <SessionProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<PageWelcome />} />
          <Route path="/login" element={<PageLogin />} />
          <Route path="/signin" element={<Navigate to="/login" replace />} />
          <Route path="/signup" element={<PageSignup />} />
          <Route
            path="/library"
            element={(
              <RequireRole minLevel={1}>
                <PageLibrary />
              </RequireRole>
            )}
          />
          <Route
            path="/settings"
            element={(
              <RequireRole minLevel={1}>
                <PageSettings />
              </RequireRole>
            )}
          />
          <Route
            path="/player/:id"
            element={(
              <RequireRole minLevel={1}>
                <PagePlayer />
              </RequireRole>
            )}
          />
          <Route
            path="/player"
            element={(
              <RequireRole minLevel={1}>
                <PagePlayer />
              </RequireRole>
            )}
          />
          <Route
            path="/collection"
            element={(
              <RequireRole minLevel={2}>
                <PageCollection />
              </RequireRole>
            )}
          />
          <Route
            path="/management"
            element={(
              <RequireRole minLevel={3}>
                <PageManagement />
              </RequireRole>
            )}
          />
          <Route
            path="/admin"
            element={(
              <RequireRole minLevel={3}>
                <PageAdmin />
              </RequireRole>
            )}
          />
          <Route path="*" element={<PageNotFound />} />
        </Routes>
      </BrowserRouter>
    </SessionProvider>
  );
}
