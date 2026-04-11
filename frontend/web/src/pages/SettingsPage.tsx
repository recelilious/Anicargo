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
  makeStyles,
} from "@fluentui/react-components";

import { useAppearance } from "../appearance";
import { MotionPage, MotionPresence } from "../motion";
import { useSession } from "../session";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    minHeight: "100%",
  },
  layout: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: "16px",
    alignItems: "start",
    "@media (max-width: 1100px)": {
      gridTemplateColumns: "1fr",
    },
  },
  column: {
    display: "flex",
    flexDirection: "column",
    gap: "16px",
    minWidth: 0,
  },
  card: {
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    padding: "20px",
    display: "flex",
    flexDirection: "column",
    gap: "14px",
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  identityGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
    gap: "10px",
    "@media (max-width: 520px)": {
      gridTemplateColumns: "1fr",
    },
  },
  identityCell: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    padding: "12px 14px",
    borderRadius: "14px",
    backgroundColor: "var(--app-surface-2)",
    border: "1px solid var(--app-border)",
  },
  muted: {
    color: "var(--app-muted)",
  },
  hashValue: {
    wordBreak: "break-all",
  },
});

function getIdentityLabel(isGuestViewer: boolean, isAdmin: boolean) {
  if (isGuestViewer) {
    return "游客设备";
  }

  if (isAdmin) {
    return "管理员账号";
  }

  return "普通账号";
}

export function SettingsPage() {
  const styles = useStyles();
  const {
    bootstrap,
    deepNightMode,
    displayName,
    isAdmin,
    isGuestViewer,
    loginAccount,
    logoutAccount,
    registerAccount,
    setDeepNightMode,
    systemTimeZone,
  } = useSession();
  const { themePreference, resolvedAppearance, setThemePreference } = useAppearance();
  const [registerForm, setRegisterForm] = useState({ username: "", password: "" });
  const [loginForm, setLoginForm] = useState({ username: "", password: "" });
  const [error, setError] = useState<string | null>(null);

  const identityLabel = getIdentityLabel(isGuestViewer, isAdmin);
  const identityHash = bootstrap?.deviceId ?? "—";

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

  return (
    <MotionPage className={styles.page}>
      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      <div className={styles.layout}>
        <div className={styles.column}>
          <Card className={`${styles.card} app-motion-surface`}>
            <div className={styles.titleGroup}>
              <Text weight="semibold">身份</Text>
              <Text size={300} className={styles.muted}>
                当前设备与账号身份信息会显示在这里。
              </Text>
            </div>

            <div className={styles.identityGrid}>
              <div className={styles.identityCell}>
                <Text size={200} className={styles.muted}>
                  当前身份
                </Text>
                <Text weight="semibold">{identityLabel}</Text>
              </div>

              <div className={styles.identityCell}>
                <Text size={200} className={styles.muted}>
                  名称
                </Text>
                <Text weight="semibold">{displayName}</Text>
              </div>

              <div className={styles.identityCell}>
                <Text size={200} className={styles.muted}>
                  哈希码
                </Text>
                <Text weight="semibold" className={styles.hashValue}>
                  {identityHash}
                </Text>
              </div>
            </div>

            <Text size={300} className={styles.muted}>
              {isGuestViewer
                ? "当前处于设备订阅模式。"
                : isAdmin
                  ? "当前账号已启用管理员权限，左侧会显示管理栏目。"
                  : "当前账号已登录，可在不同设备间同步账号订阅。 "}
            </Text>

            {!isGuestViewer ? (
              <Button appearance="secondary" onClick={() => void logoutAccount()}>
                退出当前账号
              </Button>
            ) : null}
          </Card>

          <MotionPresence show={isGuestViewer}>
            <>
              <form onSubmit={(event) => void onLoginSubmit(event)}>
                <Card className={`${styles.card} ${styles.form} app-motion-surface`} style={{ ["--motion-delay" as string]: "44ms" }}>
                  <div className={styles.titleGroup}>
                    <Text weight="semibold">登录账号</Text>
                    <Text size={300} className={styles.muted}>
                      使用已有账号登录，登录后将切换到账号订阅模式。
                    </Text>
                  </div>

                  <Field label="用户名">
                    <Input
                      value={loginForm.username}
                      onChange={(_, data) => setLoginForm((current) => ({ ...current, username: data.value }))}
                    />
                  </Field>

                  <Field label="密码">
                    <Input
                      type="password"
                      value={loginForm.password}
                      onChange={(_, data) => setLoginForm((current) => ({ ...current, password: data.value }))}
                    />
                  </Field>

                  <Button type="submit" appearance="primary">
                    登录
                  </Button>
                </Card>
              </form>

              <form onSubmit={(event) => void onRegisterSubmit(event)}>
                <Card className={`${styles.card} ${styles.form} app-motion-surface`} style={{ ["--motion-delay" as string]: "88ms" }}>
                  <div className={styles.titleGroup}>
                    <Text weight="semibold">注册账号</Text>
                    <Text size={300} className={styles.muted}>
                      注册后会立即切换到新账号，并使用账号身份保存订阅。
                    </Text>
                  </div>

                  <Field label="用户名">
                    <Input
                      value={registerForm.username}
                      onChange={(_, data) => setRegisterForm((current) => ({ ...current, username: data.value }))}
                    />
                  </Field>

                  <Field label="密码">
                    <Input
                      type="password"
                      value={registerForm.password}
                      onChange={(_, data) => setRegisterForm((current) => ({ ...current, password: data.value }))}
                    />
                  </Field>

                  <Button type="submit" appearance="primary">
                    注册
                  </Button>
                </Card>
              </form>
            </>
          </MotionPresence>
        </div>

        <div className={styles.column}>
          <Card className={`${styles.card} app-motion-surface`} style={{ ["--motion-delay" as string]: "132ms" }}>
            <div className={styles.titleGroup}>
              <Text weight="semibold">外观</Text>
              <Text size={300} className={styles.muted}>
                切换当前页面的明暗主题与跟随系统模式。
              </Text>
            </div>

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

          <Card className={`${styles.card} app-motion-surface`} style={{ ["--motion-delay" as string]: "176ms" }}>
            <div className={styles.titleGroup}>
              <Text weight="semibold">时间显示</Text>
              <Text size={300} className={styles.muted}>
                控制时区展示方式与深夜模式的日期归属逻辑。
              </Text>
            </div>

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
        </div>
      </div>
    </MotionPage>
  );
}
