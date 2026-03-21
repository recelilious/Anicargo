import { type FormEvent, useEffect, useState } from "react";
import { Badge, Button, Card, Field, Input, Spinner, Switch, Text, makeStyles } from "@fluentui/react-components";

import { adminLogin, adminLogout, createFansubRule, fetchAdminDashboard, updatePolicy } from "../api";
import { useSession } from "../session";
import type { AdminDashboardResponse } from "../types";

const ADMIN_TOKEN_KEY = "anicargo.admin_token";

const useStyles = makeStyles({
  page: {
    minHeight: "100vh",
    padding: "28px",
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    background: "var(--app-bg)"
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "16px"
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px"
  },
  rules: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "12px"
  },
  hero: {
    background: "var(--app-panel)",
    boxShadow: "var(--app-card-shadow)"
  }
});

export function AdminPage() {
  const styles = useStyles();
  const { deviceId } = useSession();
  const [token, setToken] = useState<string | null>(() => window.localStorage.getItem(ADMIN_TOKEN_KEY));
  const [dashboard, setDashboard] = useState<AdminDashboardResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loginForm, setLoginForm] = useState({ username: "", password: "" });
  const [ruleForm, setRuleForm] = useState({
    fansubName: "",
    localePreference: "zh-Hans",
    priority: 50,
    isBlacklist: false
  });

  useEffect(() => {
    if (!token) {
      return;
    }

    let isMounted = true;
    setIsLoading(true);

    void fetchAdminDashboard(deviceId, token)
      .then((response) => {
        if (isMounted) {
          setDashboard(response);
          setError(null);
        }
      })
      .catch((nextError: Error) => {
        if (isMounted) {
          setError(nextError.message);
          window.localStorage.removeItem(ADMIN_TOKEN_KEY);
          setToken(null);
        }
      })
      .finally(() => {
        if (isMounted) {
          setIsLoading(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [deviceId, token]);

  async function onAdminLogin(event: FormEvent) {
    event.preventDefault();

    try {
      const response = await adminLogin(loginForm.username, loginForm.password);
      window.localStorage.setItem(ADMIN_TOKEN_KEY, response.token);
      setToken(response.token);
      setLoginForm({ username: "", password: "" });
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onPolicySave(event: FormEvent) {
    event.preventDefault();
    if (!token || !dashboard) {
      return;
    }

    const policy = await updatePolicy(deviceId, token, dashboard.policy);
    setDashboard((current) => (current ? { ...current, policy } : current));
  }

  async function onRuleCreate(event: FormEvent) {
    event.preventDefault();
    if (!token) {
      return;
    }

    const nextRule = await createFansubRule(deviceId, token, ruleForm);
    setDashboard((current) =>
      current
        ? {
            ...current,
            fansubRules: [nextRule, ...current.fansubRules]
          }
        : current
    );
    setRuleForm({
      fansubName: "",
      localePreference: "zh-Hans",
      priority: 50,
      isBlacklist: false
    });
  }

  async function onAdminLogout() {
    if (token) {
      await adminLogout(deviceId, token);
    }

    window.localStorage.removeItem(ADMIN_TOKEN_KEY);
    setToken(null);
    setDashboard(null);
  }

  if (!token) {
    return (
      <section className={styles.page}>
        <form onSubmit={(event) => void onAdminLogin(event)}>
          <Card className={`${styles.form} ${styles.hero}`}>
            <Text weight="semibold" size={800}>
              管理登录
            </Text>
            <Field label="管理员用户名">
              <Input
                value={loginForm.username}
                onChange={(_, data) => setLoginForm((current) => ({ ...current, username: data.value }))}
              />
            </Field>
            <Field label="管理员密码">
              <Input
                type="password"
                value={loginForm.password}
                onChange={(_, data) => setLoginForm((current) => ({ ...current, password: data.value }))}
              />
            </Field>
            <Button type="submit" appearance="primary">
              登录
            </Button>
            {error ? <Text>{error}</Text> : null}
          </Card>
        </form>
      </section>
    );
  }

  return (
    <section className={styles.page}>
      <Card className={styles.hero}>
        <Text weight="semibold" size={800}>
          管理面板
        </Text>
        <Button appearance="secondary" onClick={() => void onAdminLogout()}>
          退出
        </Button>
      </Card>

      {isLoading ? <Spinner label="正在加载..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {dashboard ? (
        <>
          <div className={styles.grid}>
            <Card>
              <Text weight="semibold">设备</Text>
              <Text>{dashboard.counts.devices}</Text>
            </Card>
            <Card>
              <Text weight="semibold">账号</Text>
              <Text>{dashboard.counts.users}</Text>
            </Card>
            <Card>
              <Text weight="semibold">订阅</Text>
              <Text>{dashboard.counts.subscriptions}</Text>
            </Card>
            <Card>
              <Text weight="semibold">字幕组规则</Text>
              <Text>{dashboard.counts.fansubRules}</Text>
            </Card>
          </div>

          <form onSubmit={(event) => void onPolicySave(event)}>
            <Card className={styles.form}>
              <Text weight="semibold">下载策略</Text>
              <Field label="订阅阈值">
                <Input
                  type="number"
                  value={String(dashboard.policy.subscriptionThreshold)}
                  onChange={(_, data) =>
                    setDashboard((current) =>
                      current
                        ? {
                            ...current,
                            policy: {
                              ...current.policy,
                              subscriptionThreshold: Number(data.value || 0)
                            }
                          }
                        : current
                    )
                  }
                />
              </Field>
              <Field label="替换窗口（小时）">
                <Input
                  type="number"
                  value={String(dashboard.policy.replacementWindowHours)}
                  onChange={(_, data) =>
                    setDashboard((current) =>
                      current
                        ? {
                            ...current,
                            policy: {
                              ...current.policy,
                              replacementWindowHours: Number(data.value || 0)
                            }
                          }
                        : current
                    )
                  }
                />
              </Field>
              <Switch
                checked={dashboard.policy.preferSameFansub}
                onChange={(_, data) =>
                  setDashboard((current) =>
                    current
                      ? {
                          ...current,
                          policy: {
                            ...current.policy,
                            preferSameFansub: data.checked
                          }
                        }
                      : current
                  )
                }
                label="优先延续上一集字幕组"
              />
              <Button type="submit" appearance="primary">
                保存策略
              </Button>
            </Card>
          </form>

          <form onSubmit={(event) => void onRuleCreate(event)}>
            <Card className={styles.form}>
              <Text weight="semibold">新增字幕组规则</Text>
              <Field label="字幕组">
                <Input
                  value={ruleForm.fansubName}
                  onChange={(_, data) => setRuleForm((current) => ({ ...current, fansubName: data.value }))}
                />
              </Field>
              <Field label="语言偏好">
                <Input
                  value={ruleForm.localePreference}
                  onChange={(_, data) => setRuleForm((current) => ({ ...current, localePreference: data.value }))}
                />
              </Field>
              <Field label="优先级">
                <Input
                  type="number"
                  value={String(ruleForm.priority)}
                  onChange={(_, data) => setRuleForm((current) => ({ ...current, priority: Number(data.value || 0) }))}
                />
              </Field>
              <Switch
                checked={ruleForm.isBlacklist}
                onChange={(_, data) => setRuleForm((current) => ({ ...current, isBlacklist: data.checked }))}
                label="加入黑名单"
              />
              <Button type="submit" appearance="primary">
                保存规则
              </Button>
            </Card>
          </form>

          <div className={styles.rules}>
            {dashboard.fansubRules.map((rule) => (
              <Card key={rule.id}>
                <Text weight="semibold">{rule.fansubName}</Text>
                <Badge appearance={rule.isBlacklist ? "filled" : "outline"}>
                  {rule.isBlacklist ? "黑名单" : "白名单"}
                </Badge>
                <Text>优先级：{rule.priority}</Text>
                <Text>偏好：{rule.localePreference}</Text>
              </Card>
            ))}
          </div>
        </>
      ) : null}
    </section>
  );
}
