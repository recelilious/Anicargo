import {
  ArrowSwapRegular,
  CalendarLtrRegular,
  SearchRegular,
  SettingsRegular
} from "@fluentui/react-icons";
import { Avatar, Badge, Button, Text, makeStyles, tokens } from "@fluentui/react-components";
import { NavLink, Outlet } from "react-router-dom";

import { useSession } from "../session";

const useStyles = makeStyles({
  layout: {
    minHeight: "100vh",
    display: "grid",
    gridTemplateColumns: "248px 1fr",
    backgroundColor: tokens.colorNeutralBackground2
  },
  rail: {
    display: "flex",
    flexDirection: "column",
    gap: "20px",
    padding: "28px 20px",
    borderRight: `1px solid ${tokens.colorNeutralStroke2}`,
    background: "linear-gradient(180deg, #f7fbff 0%, #eef6ff 100%)"
  },
  profileCard: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "16px",
    borderRadius: tokens.borderRadiusXLarge,
    background: "linear-gradient(135deg, rgba(15, 108, 189, 0.12) 0%, rgba(255, 185, 95, 0.20) 100%)",
    border: `1px solid ${tokens.colorNeutralStroke2}`
  },
  profileRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px"
  },
  profileMeta: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    minWidth: 0
  },
  profileSubtitle: {
    color: tokens.colorNeutralForeground3
  },
  brand: {
    display: "flex",
    flexDirection: "column",
    gap: "6px"
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
    justifyContent: "flex-start"
  },
  active: {
    backgroundColor: "#d6ebff",
    color: "#0f6cbd"
  },
  footer: {
    marginTop: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "12px"
  },
  adminHint: {
    color: tokens.colorNeutralForeground3
  },
  content: {
    padding: "24px 28px 40px"
  }
});

const navItems = [
  { to: "/", label: "新番时间表", icon: CalendarLtrRegular },
  { to: "/search", label: "搜索", icon: SearchRegular },
  { to: "/settings", label: "设置", icon: SettingsRegular }
] as const;

export function AppShell() {
  const styles = useStyles();
  const { displayName, viewerModeLabel, viewerSubline } = useSession();

  return (
    <div className={styles.layout}>
      <aside className={styles.rail}>
        <div className={styles.profileCard}>
          <div className={styles.profileRow}>
            <Avatar name={displayName} color="colorful" size={48} />
            <div className={styles.profileMeta}>
              <Text weight="semibold">{displayName}</Text>
              <Text size={200} className={styles.profileSubtitle}>
                {viewerSubline}
              </Text>
            </div>
          </div>
          <Badge appearance="tint">{viewerModeLabel}</Badge>
        </div>

        <div className={styles.brand}>
          <Text weight="semibold" size={700}>
            Anicargo
          </Text>
          <Text size={300}>面向朋友间私有部署的动漫订阅、下载与播放平台。</Text>
        </div>

        <nav className={styles.nav}>
          {navItems.map((item) => (
            <NavLink key={item.to} to={item.to} end className={styles.navLink}>
              {({ isActive }) => (
                <Button
                  appearance={isActive ? "secondary" : "subtle"}
                  className={`${styles.navButton} ${isActive ? styles.active : ""}`}
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
            管理员请使用 `/admin` 独立入口登录。
          </Text>
          <NavLink to="/settings" className={styles.navLink}>
            <Button appearance="secondary" icon={<ArrowSwapRegular />}>
              打开设置
            </Button>
          </NavLink>
        </div>
      </aside>

      <main className={styles.content}>
        <Outlet />
      </main>
    </div>
  );
}
