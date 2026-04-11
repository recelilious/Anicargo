import { type FormEvent, useState } from "react";
import {
  Button,
  Card,
  Field,
  Input,
  Radio,
  RadioGroup,
  Switch,
  Text,
  makeStyles
} from "@fluentui/react-components";

import { useAppearance } from "../appearance";
import { MotionPage, MotionPresence } from "../motion";
import { useSession } from "../session";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  cards: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))",
    gap: "16px"
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px"
  },
  muted: {
    color: "var(--app-muted)"
  },
  card: {
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  }
});

export function SettingsPage() {
  const styles = useStyles();
  const {
    adminUsername,
    bootstrap,
    deepNightMode,
    displayName,
    isAdmin,
    isGuestViewer,
    loginAdmin,
    loginAccount,
    logoutAdmin,
    logoutAccount,
    registerAccount,
    setDeepNightMode,
    systemTimeZone
  } = useSession();
  const { themePreference, resolvedAppearance, setThemePreference } = useAppearance();
  const [registerForm, setRegisterForm] = useState({ username: "", password: "" });
  const [loginForm, setLoginForm] = useState({ username: "", password: "" });
  const [adminForm, setAdminForm] = useState({ username: "", password: "" });
  const [error, setError] = useState<string | null>(null);
  const [adminError, setAdminError] = useState<string | null>(null);

  async function onRegisterSubmit(event: FormEvent) {
    event.preventDefault();

    try {
      await registerAccount(registerForm.username, registerForm.password);
      setRegisterForm({ username: "", password: "" });
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onLoginSubmit(event: FormEvent) {
    event.preventDefault();

    try {
      await loginAccount(loginForm.username, loginForm.password);
      setLoginForm({ username: "", password: "" });
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onAdminLoginSubmit(event: FormEvent) {
    event.preventDefault();

    try {
      await loginAdmin(adminForm.username, adminForm.password);
      setAdminForm({ username: "", password: "" });
      setAdminError(null);
    } catch (nextError) {
      setAdminError((nextError as Error).message);
    }
  }

  return (
    <MotionPage className={styles.page}>
      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      <div className={styles.cards}>
        <Card className={`${styles.card} app-motion-surface`}>
          <Text weight="semibold">身份</Text>
          <Text>{isGuestViewer ? `游客：${displayName}` : `账号：${displayName}`}</Text>
          <Text className={styles.muted}>{bootstrap?.deviceId}</Text>
          {!isGuestViewer ? (
            <Button appearance="secondary" onClick={() => void logoutAccount()}>
              退出账号
            </Button>
          ) : null}
        </Card>

        <Card className={`${styles.card} app-motion-surface`} style={{ ["--motion-delay" as string]: "44ms" }}>
          <Text weight="semibold">外观</Text>
          <RadioGroup
            value={themePreference}
            onChange={(_, data) => setThemePreference(data.value as "system" | "light" | "dark")}
          >
            <Radio value="system" label="跟随系统" />
            <Radio value="light" label="浅色" />
            <Radio value="dark" label="深色" />
          </RadioGroup>
          <Text size={300} className={styles.muted}>
            当前生效：{resolvedAppearance === "dark" ? "深色" : "浅色"}
          </Text>
        </Card>

        <Card className={`${styles.card} app-motion-surface`} style={{ ["--motion-delay" as string]: "88ms" }}>
          <Text weight="semibold">时间显示</Text>
          <Switch
            checked={deepNightMode}
            label={deepNightMode ? "深夜模式已开启" : "深夜模式已关闭"}
            onChange={(_, data) => setDeepNightMode(Boolean(data.checked))}
          />
          <Text size={300} className={styles.muted}>
            当前时区：{systemTimeZone}
          </Text>
          <Text size={300} className={styles.muted}>
            开启后，凌晨 06:00 之前会按前一日显示为 24+ 小时制。
          </Text>
        </Card>

        <Card className={`${styles.card} app-motion-surface`} style={{ ["--motion-delay" as string]: "132ms" }}>
          <Text weight="semibold">管理</Text>
          {isAdmin ? (
            <>
              <Text>{adminUsername ? `管理员：${adminUsername}` : "管理员已登录"}</Text>
              <Button appearance="secondary" onClick={() => void logoutAdmin()}>
                退出管理员
              </Button>
            </>
          ) : (
            <form className={styles.form} onSubmit={(event) => void onAdminLoginSubmit(event)}>
              <Field label="管理员用户名">
                <Input
                  value={adminForm.username}
                  onChange={(_, data) =>
                    setAdminForm((current) => ({ ...current, username: data.value }))
                  }
                />
              </Field>
              <Field label="管理员密码">
                <Input
                  type="password"
                  value={adminForm.password}
                  onChange={(_, data) =>
                    setAdminForm((current) => ({ ...current, password: data.value }))
                  }
                />
              </Field>
              <Button type="submit" appearance="secondary">
                登录管理员
              </Button>
              <MotionPresence show={Boolean(adminError)} mode="soft">
                {adminError ? <Text>{adminError}</Text> : null}
              </MotionPresence>
            </form>
          )}
        </Card>
      </div>

      <MotionPresence show={isGuestViewer}>
        <div className={styles.cards}>
          <form onSubmit={(event) => void onRegisterSubmit(event)}>
            <Card className={`${styles.card} ${styles.form} app-motion-surface`}>
              <Text weight="semibold">注册账号</Text>
              <Field label="用户名">
                <Input
                  value={registerForm.username}
                  onChange={(_, data) =>
                    setRegisterForm((current) => ({ ...current, username: data.value }))
                  }
                />
              </Field>
              <Field label="密码">
                <Input
                  type="password"
                  value={registerForm.password}
                  onChange={(_, data) =>
                    setRegisterForm((current) => ({ ...current, password: data.value }))
                  }
                />
              </Field>
              <Button type="submit" appearance="primary">
                注册
              </Button>
            </Card>
          </form>

          <form onSubmit={(event) => void onLoginSubmit(event)}>
            <Card className={`${styles.card} ${styles.form} app-motion-surface`} style={{ ["--motion-delay" as string]: "44ms" }}>
              <Text weight="semibold">登录账号</Text>
              <Field label="用户名">
                <Input
                  value={loginForm.username}
                  onChange={(_, data) =>
                    setLoginForm((current) => ({ ...current, username: data.value }))
                  }
                />
              </Field>
              <Field label="密码">
                <Input
                  type="password"
                  value={loginForm.password}
                  onChange={(_, data) =>
                    setLoginForm((current) => ({ ...current, password: data.value }))
                  }
                />
              </Field>
              <Button type="submit" appearance="primary">
                登录
              </Button>
            </Card>
          </form>
        </div>
      </MotionPresence>
    </MotionPage>
  );
}
