import { useEffect, useMemo, useState } from "react";
import {
  BookmarkRegular,
  BoxRegular,
  CalendarLtrRegular,
  HistoryRegular,
  SearchRegular,
  SettingsRegular
} from "@fluentui/react-icons";
import { Badge, Button, Text, makeStyles, tokens } from "@fluentui/react-components";
import { NavLink, Outlet, useLocation } from "react-router-dom";

import { fetchCatalogManifest } from "../api";
import type { RouteState } from "../navigation";
import { useSession } from "../session";
import { BrandLogo } from "./BrandLogo";

const useStyles = makeStyles({
  layout: {
    height: "100vh",
    display: "grid",
    gridTemplateColumns: "220px 1fr",
    backgroundColor: "var(--app-bg)",
    overflow: "hidden"
  },
  rail: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    height: "100vh",
    padding: "22px 14px",
    borderRight: `1px solid ${tokens.colorNeutralStroke2}`,
    backgroundColor: "var(--app-rail)",
    overflow: "hidden"
  },
  brand: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    padding: "2px 6px 0"
  },
  brandLogo: {
    width: "36px",
    height: "44px",
    flexShrink: 0,
    color: "var(--app-text)"
  },
  brandTitle: {
    minWidth: 0
  },
  profileCard: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    padding: "14px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "var(--app-panel)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  profileMeta: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    minWidth: 0
  },
  profileSubtitle: {
    color: "var(--app-muted)"
  },
  nav: {
    display: "flex",
    flexDirection: "column",
    gap: "8px"
  },
  navLink: {
    textDecorationLine: "none"
  },
  navButton: {
    width: "100%",
    justifyContent: "flex-start",
    borderRadius: tokens.borderRadiusLarge
  },
  active: {
    backgroundColor: "var(--app-selected-bg)",
    color: "var(--app-selected-fg)"
  },
  footer: {
    marginTop: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    padding: "0 6px"
  },
  adminHint: {
    color: "var(--app-muted)"
  },
  content: {
    minWidth: 0,
    height: "100vh",
    padding: "24px 28px 40px",
    overflowY: "auto",
    overflowX: "hidden"
  }
});

type NavItem = {
  to: string;
  label: string;
  icon: typeof SearchRegular;
};

export function AppShell() {
  const styles = useStyles();
  const location = useLocation();
  const { deviceId, displayName, userToken, viewerModeLabel, viewerSubline } = useSession();
  const [catalogManifest, setCatalogManifest] = useState({
    previewAvailable: false
  });

  useEffect(() => {
    let cancelled = false;

    void fetchCatalogManifest(deviceId, userToken)
      .then((response) => {
        if (!cancelled) {
          setCatalogManifest({ previewAvailable: response.previewAvailable });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setCatalogManifest({ previewAvailable: false });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [deviceId, userToken]);

  useEffect(() => {
    const scrollRoot = document.getElementById("app-scroll-root");
    if (!scrollRoot) {
      return;
    }

    const restoreScrollTop = (location.state as RouteState | null)?.restoreScrollTop;
    scrollRoot.scrollTo({
      top: typeof restoreScrollTop === "number" ? restoreScrollTop : 0,
      behavior: "auto"
    });
  }, [location.key, location.state]);

  const navItems = useMemo<NavItem[]>(() => {
    const items: NavItem[] = [
      { to: "/search", label: "搜索", icon: SearchRegular },
      { to: "/", label: "新番时间表", icon: CalendarLtrRegular }
    ];

    if (catalogManifest.previewAvailable) {
      items.push({ to: "/preview", label: "新季度前瞻", icon: CalendarLtrRegular });
    }

    items.push(
      { to: "/subscriptions", label: "订阅", icon: BookmarkRegular },
      { to: "/resources", label: "资源", icon: BoxRegular },
      { to: "/history", label: "历史记录", icon: HistoryRegular },
      { to: "/settings", label: "设置", icon: SettingsRegular }
    );

    return items;
  }, [catalogManifest.previewAvailable]);

  return (
    <div className={styles.layout}>
      <aside className={styles.rail}>
        <div className={styles.brand}>
          <BrandLogo className={styles.brandLogo} aria-hidden="true" />
          <Text weight="semibold" size={700} className={styles.brandTitle}>
            Anicargo
          </Text>
        </div>

        <div className={styles.profileCard}>
          <div className={styles.profileMeta}>
            <Text weight="semibold">{displayName}</Text>
            <Text size={200} className={styles.profileSubtitle}>
              {viewerSubline}
            </Text>
          </div>
          <Badge appearance="tint">{viewerModeLabel}</Badge>
        </div>

        <nav className={styles.nav}>
          {navItems.map((item) => (
            <NavLink key={item.to} to={item.to} end={item.to === "/"} className={styles.navLink}>
              {({ isActive }) => (
                <Button
                  appearance={isActive ? "secondary" : "subtle"}
                  className={`${styles.navButton} ${isActive ? styles.active : ""}`.trim()}
                  icon={<item.icon />}
                >
                  {item.label}
                </Button>
              )}
            </NavLink>
          ))}
        </nav>

        <div className={styles.footer}>
          <Text size={200} className={styles.adminHint}>
            管理入口：/admin
          </Text>
        </div>
      </aside>

      <main id="app-scroll-root" className={styles.content}>
        <Outlet />
      </main>
    </div>
  );
}
