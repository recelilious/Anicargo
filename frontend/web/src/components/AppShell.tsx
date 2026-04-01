import { useEffect, useMemo, useState } from "react";
import {
  BookmarkRegular,
  BoxRegular,
  CalendarLtrRegular,
  HistoryRegular,
  SearchRegular,
  SettingsRegular
} from "@fluentui/react-icons";
import { Button, Text, makeStyles, tokens } from "@fluentui/react-components";
import { NavLink, useLocation, useOutlet } from "react-router-dom";

import { fetchCatalogManifest } from "../api";
import { RoutedMotionOutlet } from "../motion";
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
  brandDivider: {
    width: "100%",
    height: "1px",
    backgroundColor: "var(--app-border)",
    marginTop: "-2px",
    marginBottom: "4px"
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
    display: "none"
  },
  adminHint: {
    display: "none"
  },
  content: {
    minWidth: 0,
    height: "100vh",
    padding: "24px 28px 40px",
    overflow: "hidden",
    display: "flex",
    flexDirection: "column",
  },
  scrollViewport: {
    flex: "1 1 auto",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
  },
  containedViewport: {
    flex: "1 1 auto",
    minHeight: 0,
    overflow: "hidden",
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
  const outlet = useOutlet();
  const { deviceId, userToken } = useSession();
  const [catalogManifest, setCatalogManifest] = useState({
    previewAvailable: false
  });
  const usesContainedScroll =
    location.pathname.startsWith("/resources") || location.pathname.startsWith("/history");

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
        <div className={`${styles.brand} app-motion-surface`} style={{ ["--motion-delay" as string]: "0ms" }}>
          <BrandLogo className={styles.brandLogo} aria-hidden="true" />
          <Text weight="semibold" size={700} className={styles.brandTitle}>
            Anicargo
          </Text>
        </div>
        <div className={`${styles.brandDivider} app-motion-surface`} style={{ ["--motion-delay" as string]: "42ms" }} />

        <nav className={styles.nav}>
          {navItems.map((item, index) => (
            <NavLink key={item.to} to={item.to} end={item.to === "/"} className={styles.navLink}>
              {({ isActive }) => (
                <Button
                  appearance={isActive ? "secondary" : "subtle"}
                  className={`${styles.navButton} ${isActive ? styles.active : ""} app-motion-surface`.trim()}
                  style={{ ["--motion-delay" as string]: `${90 + index * 34}ms` }}
                  icon={<item.icon />}
                >
                  {item.label}
                </Button>
              )}
            </NavLink>
          ))}
        </nav>
      </aside>

      <main
        className={styles.content}
      >
        <div
          id="app-scroll-root"
          className={usesContainedScroll ? styles.containedViewport : styles.scrollViewport}
        >
          <RoutedMotionOutlet routeKey={location.key} outlet={outlet} />
        </div>
      </main>
    </div>
  );
}
