import type { ReactNode } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import LogoMark from "../Icons/LogoMark";
import { useSession } from "../session";
import "../Styles/AppShell.css";

interface NavItem {
  label: string;
  path: string;
  minLevel: number;
}

const navItems: NavItem[] = [
  { label: "Library", path: "/library", minLevel: 1 },
  { label: "Collection", path: "/collection", minLevel: 2 },
  { label: "Management", path: "/management", minLevel: 3 },
  { label: "Admin", path: "/admin", minLevel: 3 },
  { label: "Settings", path: "/settings", minLevel: 1 }
];

interface AppShellProps {
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  children: ReactNode;
}

export default function AppShell({ title, subtitle, actions, children }: AppShellProps) {
  const { session, clearSession } = useSession();
  const navigate = useNavigate();
  const roleLevel = session?.roleLevel ?? 0;
  const roleLabel = roleLevel >= 3 ? `Admin · L${roleLevel}` : `User · L${roleLevel}`;

  function handleLogout() {
    clearSession();
    navigate("/", { replace: true });
  }

  return (
    <div className="app-shell">
      <aside className="app-sidebar">
        <div className="app-brand">
          <LogoMark size={32} />
          <span className="app-brand-title">Anicargo</span>
        </div>
        <nav className="app-nav">
          {navItems
            .filter((item) => roleLevel >= item.minLevel)
            .map((item) => (
              <NavLink
                key={item.path}
                to={item.path}
                className={({ isActive }) =>
                  isActive ? "app-nav-link is-active" : "app-nav-link"
                }
              >
                {item.label}
              </NavLink>
            ))}
        </nav>
      </aside>

      <div className="app-main">
        <header className="app-topbar">
          <div>
            <p className="app-kicker">Workspace</p>
            <h1 className="app-title">{title}</h1>
            {subtitle ? <p className="app-subtitle">{subtitle}</p> : null}
          </div>
          <div className="app-topbar-actions">
            {actions}
            <div className="app-user">
              <div className="app-user-meta">
                <span className="app-user-name">{session?.userId ?? "unknown"}</span>
                <span className="app-user-role">{roleLabel}</span>
              </div>
              <button type="button" className="app-btn ghost" onClick={handleLogout}>
                Log out
              </button>
            </div>
          </div>
        </header>

        <main className="app-content">{children}</main>
      </div>
    </div>
  );
}
