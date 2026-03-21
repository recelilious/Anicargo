import { type FormEvent, useState } from "react";
import { Button, Card, Field, Input, Switch, Text, makeStyles } from "@fluentui/react-components";

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
  const { bootstrap, displayName, isGuestViewer, registerAccount, loginAccount, logoutAccount } = useSession();
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
          默认模式下不需要登录，当前设备本身就可以订阅。只有在你需要跨设备同步订阅时，才需要注册并登录账号。
        </Text>
      </Card>

      {error ? <Text>{error}</Text> : null}

      <div className={styles.cards}>
        <Card>
          <Text weight="semibold">当前身份</Text>
          <Text>{isGuestViewer ? `游客：${displayName}` : `账号：${displayName}`}</Text>
          <Text>{isGuestViewer ? "当前订阅保存在本机设备中。" : "当前订阅会跟随账号同步。"}</Text>
          <Text size={300}>{bootstrap?.deviceId}</Text>
          {!isGuestViewer ? (
            <Button appearance="secondary" onClick={() => void logoutAccount()}>
              退出账号
            </Button>
          ) : null}
        </Card>

        <Card>
          <Text weight="semibold">网页设置</Text>
          <Switch defaultChecked label="优先显示中文标题" />
          <Switch defaultChecked label="进入首页时聚焦到今天" />
          <Text size={300}>后续这里会继续补充播放、布局和通知相关的偏好设置。</Text>
        </Card>
      </div>

      {isGuestViewer ? (
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
                注册并切换到账号模式
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
                登录并同步账号订阅
              </Button>
            </Card>
          </form>
        </div>
      ) : null}
    </section>
  );
}
