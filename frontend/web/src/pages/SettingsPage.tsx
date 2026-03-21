import { FormEvent, useState } from "react";
import {
  Button,
  Card,
  Field,
  Input,
  Switch,
  Text,
  makeStyles
} from "@fluentui/react-components";

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
  }
});

export function SettingsPage() {
  const styles = useStyles();
  const { bootstrap, registerAccount, loginAccount, logoutAccount } = useSession();
  const [registerForm, setRegisterForm] = useState({ username: "", password: "" });
  const [loginForm, setLoginForm] = useState({ username: "", password: "" });
  const [error, setError] = useState<string | null>(null);

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
    <section className={styles.page}>
      <Card>
        <Text weight="semibold" size={800}>
          用户设置
        </Text>
        <Text>
          默认模式下不要求登录，设备本身就可以订阅。需要跨设备同步时，再注册并登录账号。
        </Text>
      </Card>

      {error ? <Text>{error}</Text> : null}

      <div className={styles.cards}>
        <Card>
          <Text weight="semibold">当前身份</Text>
          <Text>{bootstrap?.viewer.kind === "user" ? `账号：${bootstrap.viewer.label}` : "匿名设备模式"}</Text>
          <Text>{bootstrap?.deviceId}</Text>
          {bootstrap?.viewer.kind === "user" ? (
            <Button appearance="secondary" onClick={() => void logoutAccount()}>
              退出账号
            </Button>
          ) : null}
        </Card>

        <Card>
          <Text weight="semibold">网页设置</Text>
          <Switch defaultChecked label="默认显示中文标题" />
          <Switch defaultChecked label="默认展示今日时间表" />
          <Text size={300}>更多布局、语言和播放偏好后续会继续补。</Text>
        </Card>
      </div>

      {bootstrap?.viewer.kind !== "user" ? (
        <div className={styles.cards}>
          <form onSubmit={(event) => void onRegisterSubmit(event)}>
            <Card className={styles.form}>
              <Text weight="semibold">注册账号</Text>
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
                注册并切换到账号
              </Button>
            </Card>
          </form>

          <form onSubmit={(event) => void onLoginSubmit(event)}>
            <Card className={styles.form}>
              <Text weight="semibold">登录账号</Text>
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
                登录并使用账号订阅
              </Button>
            </Card>
          </form>
        </div>
      ) : null}
    </section>
  );
}
