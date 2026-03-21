import {
  ArrowSwapRegular,
  CalendarLtrRegular,
  SearchRegular,
  SettingsRegular
} from "@fluentui/react-icons";
import { Badge, Button, Text, makeStyles, tokens } from "@fluentui/react-components";
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
  const { bootstrap } = useSession();

  return (
    <div className={styles.layout}>
      <aside className={styles.rail}>
        <div className={styles.brand}>
          <Text weight="semibold" size={700}>
            Anicargo
          </Text>
          <Text size={300}>
            面向朋友间私有部署的动漫订阅、下载与播放平台。
          </Text>
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
          <Badge appearance="outline">
            {bootstrap?.viewer.kind === "user" ? "账号订阅" : "设备订阅"}
          </Badge>
          <Text size={300}>
            {bootstrap?.viewer.kind === "user"
              ? `当前账号：${bootstrap.viewer.label}`
              : `当前设备：${bootstrap?.deviceId ?? "初始化中"}`}
          </Text>
          <NavLink to="/settings" className={styles.navLink}>
            <Button appearance="secondary" icon={<ArrowSwapRegular />}>
              切换到设置
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
