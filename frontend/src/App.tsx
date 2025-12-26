import { BrowserRouter, Navigate, NavLink, Route, Routes } from "react-router-dom";
import { resolveApiUrl } from "./api";
import LoginPage from "./pages/LoginPage";
import LibraryPage from "./pages/LibraryPage";
import PlayerPage from "./pages/PlayerPage";
import UserManagementPage from "./pages/UserManagementPage";
import { SessionProvider, useSession } from "./session";

function RequireAuth({ children }: { children: JSX.Element }) {
  const { session } = useSession();
  if (!session) {
    return <Navigate to="/login" replace />;
  }
  return children;
}

function Shell() {
  const { session, clearSession } = useSession();
  const profileLabel = session
    ? `${session.userId} (${session.role})`
    : "Not signed in";

  return (
    <div className="app">
      <header className="hero">
        <div>
          <p className="eyebrow">Self-hosted anime vault</p>
          <h1>Anicargo</h1>
          <p className="subtitle">
            Stream your curated library from anywhere. Auth-first, HLS-ready.
          </p>
        </div>
        <nav className="nav">
          <NavLink to="/library">Library</NavLink>
          <NavLink to="/player">Player</NavLink>
          <NavLink to="/users">Users</NavLink>
        </nav>
        <div className="profile">
          <span className="chip">{profileLabel}</span>
          {session ? (
            <button className="ghost" onClick={clearSession} type="button">
              Sign out
            </button>
          ) : (
            <NavLink className="ghost" to="/login">
              Sign in
            </NavLink>
          )}
        </div>
      </header>

      <main className="page">
        <Routes>
          <Route path="/" element={<Navigate to="/library" replace />} />
          <Route path="/login" element={<LoginPage />} />
          <Route
            path="/library"
            element={
              <RequireAuth>
                <LibraryPage />
              </RequireAuth>
            }
          />
          <Route
            path="/player"
            element={
              <RequireAuth>
                <PlayerPage />
              </RequireAuth>
            }
          />
          <Route
            path="/player/:id"
            element={
              <RequireAuth>
                <PlayerPage />
              </RequireAuth>
            }
          />
          <Route
            path="/users"
            element={
              <RequireAuth>
                <UserManagementPage />
              </RequireAuth>
            }
          />
          <Route path="*" element={<Navigate to="/library" replace />} />
        </Routes>
      </main>

      <footer className="footer">
        <span>API base: {resolveApiUrl("/")}</span>
        <span>VITE_API_BASE overrides default routing.</span>
      </footer>
    </div>
  );
}

export default function App() {
  return (
    <SessionProvider>
      <BrowserRouter>
        <Shell />
      </BrowserRouter>
    </SessionProvider>
  );
}
