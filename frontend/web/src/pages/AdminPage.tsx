import { type FormEvent, useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  Field,
  Input,
  Spinner,
  Switch,
  Text,
  makeStyles
} from "@fluentui/react-components";

import {
  activateAdminDownload,
  adminLogin,
  adminLogout,
  createFansubRule,
  fetchAdminDashboard,
  fetchAdminDownloadCandidates,
  fetchAdminDownloadExecutions,
  fetchAdminDownloads,
  fetchAdminExecutionEvents,
  fetchAdminRuntime,
  forceAdminDownload,
  updatePolicy
} from "../api";
import { useSession } from "../session";
import type {
  AdminDashboardResponse,
  AdminRuntimeResponse,
  DownloadExecution,
  DownloadExecutionEvent,
  DownloadJob,
  ResourceCandidate
} from "../types";

const ADMIN_TOKEN_KEY = "anicargo.admin_token";

const useStyles = makeStyles({
  page: {
    minHeight: "100vh",
    padding: "28px",
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    backgroundColor: "var(--app-bg)"
  },
  header: {
    padding: "18px 20px",
    display: "flex",
    justifyContent: "space-between",
    gap: "16px",
    alignItems: "center",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  },
  layout: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1.1fr) minmax(360px, 0.9fr)",
    gap: "18px",
    alignItems: "start"
  },
  column: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  panel: {
    padding: "18px 20px",
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px"
  },
  downloadsList: {
    display: "flex",
    flexDirection: "column",
    gap: "10px"
  },
  downloadCard: {
    padding: "14px 16px",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    backgroundColor: "var(--app-surface-2)",
    border: "1px solid var(--app-border)"
  },
  activeDownloadCard: {
    outline: "2px solid var(--app-selected-fg)"
  },
  cardRow: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "center",
    flexWrap: "wrap"
  },
  compactGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: "10px"
  },
  muted: {
    color: "var(--app-muted)"
  },
  actions: {
    display: "flex",
    gap: "10px",
    flexWrap: "wrap"
  },
  stack: {
    display: "flex",
    flexDirection: "column",
    gap: "10px"
  }
});

function formatBytes(value: number) {
  if (!value) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size >= 10 || unitIndex === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unitIndex]}`;
}

function formatSpeed(value: number) {
  return value > 0 ? `${formatBytes(value)}/s` : "0 B/s";
}

function formatExecutionProgress(execution: DownloadExecution | null) {
  if (!execution) {
    return "暂无执行";
  }

  const totalBytes = Math.max(execution.sourceSizeBytes, execution.downloadedBytes);
  if (totalBytes > 0) {
    const progress = ((execution.downloadedBytes / totalBytes) * 100).toFixed(1);
    return `${formatBytes(execution.downloadedBytes)} / ${formatBytes(totalBytes)} (${progress}%)`;
  }

  return formatBytes(execution.downloadedBytes);
}

export function AdminPage() {
  const styles = useStyles();
  const { deviceId } = useSession();
  const [token, setToken] = useState<string | null>(() => window.localStorage.getItem(ADMIN_TOKEN_KEY));
  const [dashboard, setDashboard] = useState<AdminDashboardResponse | null>(null);
  const [runtime, setRuntime] = useState<AdminRuntimeResponse | null>(null);
  const [downloads, setDownloads] = useState<DownloadJob[]>([]);
  const [selectedJobId, setSelectedJobId] = useState<number | null>(null);
  const [candidates, setCandidates] = useState<ResourceCandidate[]>([]);
  const [executions, setExecutions] = useState<DownloadExecution[]>([]);
  const [events, setEvents] = useState<DownloadExecutionEvent[]>([]);
  const [selectedExecutionId, setSelectedExecutionId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loginForm, setLoginForm] = useState({ username: "", password: "" });
  const [forceSubjectId, setForceSubjectId] = useState("");
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

    const adminToken = token;
    let isMounted = true;

    async function loadAdminState() {
      setIsLoading(true);
      try {
        const [dashboardResponse, runtimeResponse, downloadsResponse] = await Promise.all([
          fetchAdminDashboard(deviceId, adminToken),
          fetchAdminRuntime(deviceId, adminToken),
          fetchAdminDownloads(deviceId, adminToken)
        ]);

        if (!isMounted) {
          return;
        }

        setDashboard(dashboardResponse);
        setRuntime(runtimeResponse);
        setDownloads(downloadsResponse.items);
        setSelectedJobId((current) => current ?? downloadsResponse.items[0]?.id ?? null);
        setError(null);
      } catch (nextError) {
        if (!isMounted) {
          return;
        }

        setError((nextError as Error).message);
        window.localStorage.removeItem(ADMIN_TOKEN_KEY);
        setToken(null);
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    }

    void loadAdminState();
    const interval = window.setInterval(() => {
      void loadAdminState();
    }, 5000);

    return () => {
      isMounted = false;
      window.clearInterval(interval);
    };
  }, [deviceId, token]);

  useEffect(() => {
    if (!token || !selectedJobId) {
      setCandidates([]);
      setExecutions([]);
      setEvents([]);
      setSelectedExecutionId(null);
      return;
    }

    let isMounted = true;
    void Promise.all([
      fetchAdminDownloadCandidates(deviceId, token, selectedJobId),
      fetchAdminDownloadExecutions(deviceId, token, selectedJobId)
    ]).then(([candidateResponse, executionResponse]) => {
      if (!isMounted) {
        return;
      }

      setCandidates(candidateResponse.items);
      setExecutions(executionResponse.items);
      setSelectedExecutionId((current) => current ?? executionResponse.items[0]?.id ?? null);
    });

    return () => {
      isMounted = false;
    };
  }, [deviceId, selectedJobId, token]);

  useEffect(() => {
    if (!token || !selectedExecutionId) {
      setEvents([]);
      return;
    }

    let isMounted = true;
    void fetchAdminExecutionEvents(deviceId, token, selectedExecutionId).then((response) => {
      if (isMounted) {
        setEvents(response.items);
      }
    });

    return () => {
      isMounted = false;
    };
  }, [deviceId, selectedExecutionId, token]);

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

  async function onForceDownload() {
    if (!token || !forceSubjectId.trim()) {
      return;
    }

    await forceAdminDownload(deviceId, token, Number(forceSubjectId));
    const downloadsResponse = await fetchAdminDownloads(deviceId, token);
    setDownloads(downloadsResponse.items);
    setSelectedJobId(downloadsResponse.items[0]?.id ?? null);
    setForceSubjectId("");
  }

  async function onActivateDownload(jobId: number) {
    if (!token) {
      return;
    }

    await activateAdminDownload(deviceId, token, jobId);
    const [executionResponse, downloadsResponse] = await Promise.all([
      fetchAdminDownloadExecutions(deviceId, token, jobId),
      fetchAdminDownloads(deviceId, token)
    ]);
    setExecutions(executionResponse.items);
    setSelectedExecutionId(executionResponse.items[0]?.id ?? null);
    setDownloads(downloadsResponse.items);
  }

  async function onAdminLogout() {
    if (token) {
      await adminLogout(deviceId, token);
    }

    window.localStorage.removeItem(ADMIN_TOKEN_KEY);
    setToken(null);
    setDashboard(null);
    setRuntime(null);
    setDownloads([]);
    setCandidates([]);
    setExecutions([]);
    setEvents([]);
  }

  if (!token) {
    return (
      <section className={styles.page}>
        <form onSubmit={(event) => void onAdminLogin(event)}>
          <Card className={styles.panel}>
            <Text weight="semibold" size={800}>
              管理员登录
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

  const selectedJob = downloads.find((job) => job.id === selectedJobId) ?? null;
  const selectedExecution = executions.find((execution) => execution.id === selectedExecutionId) ?? executions[0] ?? null;

  return (
    <section className={styles.page}>
      <Card className={styles.header}>
        <div>
          <Text weight="semibold" size={800}>
            管理面板
          </Text>
          <Text className={styles.muted}>下载、策略、资源与运行态统一管理。</Text>
        </div>

        <div className={styles.actions}>
          <Button appearance="secondary" onClick={() => void onAdminLogout()}>
            退出
          </Button>
        </div>
      </Card>

      {isLoading ? <Spinner label="正在同步管理状态..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {runtime ? (
        <div className={styles.grid}>
          <Card className={styles.panel}>
            <Text weight="semibold">服务地址</Text>
            <Text>{runtime.serverAddress}</Text>
            <Text className={styles.muted}>运行 {runtime.uptimeLabel}</Text>
          </Card>
          <Card className={styles.panel}>
            <Text weight="semibold">活跃下载</Text>
            <Text>{runtime.runtime.activeExecutions}</Text>
            <Text className={styles.muted}>{formatSpeed(runtime.runtime.downloadRateBytes)}</Text>
          </Card>
          <Card className={styles.panel}>
            <Text weight="semibold">下载任务</Text>
            <Text>{runtime.runtime.openDownloadJobs}</Text>
            <Text className={styles.muted}>已选资源 {runtime.runtime.jobsWithSelection}</Text>
          </Card>
          <Card className={styles.panel}>
            <Text weight="semibold">HTTP 请求</Text>
            <Text>{runtime.http.totalRequests}</Text>
            <Text className={styles.muted}>失败 {runtime.http.failedRequests}</Text>
          </Card>
        </div>
      ) : null}

      <div className={styles.layout}>
        <div className={styles.column}>
          <Card className={styles.panel}>
            <Text weight="semibold">手动触发下载</Text>
            <div className={styles.actions}>
              <Field label="Bangumi Subject ID" style={{ flex: 1 }}>
                <Input value={forceSubjectId} onChange={(_, data) => setForceSubjectId(data.value)} />
              </Field>
              <Button appearance="primary" onClick={() => void onForceDownload()}>
                强制触发
              </Button>
            </div>
          </Card>

          <Card className={styles.panel}>
            <Text weight="semibold">下载队列</Text>
            <div className={styles.downloadsList}>
              {downloads.map((job) => (
                <Card
                  key={job.id}
                  className={`${styles.downloadCard} ${selectedJobId === job.id ? styles.activeDownloadCard : ""}`}
                  onClick={() => setSelectedJobId(job.id)}
                >
                  <div className={styles.cardRow}>
                    <Text weight="semibold">Bangumi {job.bangumiSubjectId}</Text>
                    <Badge appearance="outline">{job.lifecycle}</Badge>
                  </div>
                  <Text className={styles.muted}>
                    {job.triggerKind} · {job.requestedBy}
                  </Text>
                  <Text>
                    订阅 {job.subscriptionCount} / {job.thresholdSnapshot}
                  </Text>
                  <Text className={styles.muted}>搜索状态：{job.searchStatus}</Text>
                  <div className={styles.actions}>
                    <Button appearance="secondary" onClick={() => void onActivateDownload(job.id)}>
                      执行所选资源
                    </Button>
                  </div>
                </Card>
              ))}
            </div>
          </Card>

          {selectedJob ? (
            <>
              <Card className={styles.panel}>
                <Text weight="semibold">候选资源</Text>
                <div className={styles.stack}>
                  {candidates.map((candidate) => (
                    <Card key={candidate.id} className={styles.downloadCard}>
                      <div className={styles.cardRow}>
                        <Text weight="semibold">{candidate.fansubName ?? candidate.publisherName}</Text>
                        <Badge appearance={candidate.rejectedReason ? "outline" : "filled"}>{candidate.slotKey}</Badge>
                      </div>
                      <Text>{candidate.title}</Text>
                      <Text className={styles.muted}>
                        分数 {candidate.score.toFixed(1)} · {candidate.resolution ?? "未知分辨率"}
                      </Text>
                      {candidate.rejectedReason ? <Text>{candidate.rejectedReason}</Text> : null}
                    </Card>
                  ))}
                </div>
              </Card>

              <Card className={styles.panel}>
                <Text weight="semibold">执行实例</Text>
                <div className={styles.stack}>
                  {executions.map((execution) => (
                    <Card
                      key={execution.id}
                      className={`${styles.downloadCard} ${selectedExecutionId === execution.id ? styles.activeDownloadCard : ""}`}
                      onClick={() => setSelectedExecutionId(execution.id)}
                    >
                      <div className={styles.cardRow}>
                        <Text weight="semibold">{execution.sourceFansubName ?? "未标注字幕组"}</Text>
                        <Badge appearance="outline">{execution.state}</Badge>
                      </div>
                      <Text>{execution.sourceTitle}</Text>
                      <Text className={styles.muted}>{formatExecutionProgress(execution)}</Text>
                      <Text className={styles.muted}>
                        {formatSpeed(execution.downloadRateBytes)} · Peer {execution.peerCount}
                      </Text>
                    </Card>
                  ))}
                </div>
              </Card>
            </>
          ) : null}
        </div>

        <div className={styles.column}>
          {selectedExecution ? (
            <Card className={styles.panel}>
              <Text weight="semibold">当前执行详情</Text>
              <div className={styles.compactGrid}>
                <Card className={styles.downloadCard}>
                  <Text weight="semibold">进度</Text>
                  <Text>{formatExecutionProgress(selectedExecution)}</Text>
                </Card>
                <Card className={styles.downloadCard}>
                  <Text weight="semibold">下载速度</Text>
                  <Text>{formatSpeed(selectedExecution.downloadRateBytes)}</Text>
                </Card>
                <Card className={styles.downloadCard}>
                  <Text weight="semibold">上传速度</Text>
                  <Text>{formatSpeed(selectedExecution.uploadRateBytes)}</Text>
                </Card>
                <Card className={styles.downloadCard}>
                  <Text weight="semibold">Peer</Text>
                  <Text>{selectedExecution.peerCount}</Text>
                </Card>
              </div>
            </Card>
          ) : null}

          <Card className={styles.panel}>
            <Text weight="semibold">执行事件</Text>
            <div className={styles.stack}>
              {events.map((event) => (
                <Card key={event.id} className={styles.downloadCard}>
                  <div className={styles.cardRow}>
                    <Text weight="semibold">{event.eventKind}</Text>
                    <Badge appearance={event.level === "error" ? "filled" : "outline"}>{event.level}</Badge>
                  </div>
                  <Text>{event.message}</Text>
                  <Text className={styles.muted}>{event.createdAt}</Text>
                </Card>
              ))}
              {events.length === 0 ? <Text className={styles.muted}>当前没有执行事件。</Text> : null}
            </div>
          </Card>

          {dashboard ? (
            <>
              <form onSubmit={(event) => void onPolicySave(event)}>
                <Card className={styles.panel}>
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
                <Card className={styles.panel}>
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
            </>
          ) : null}
        </div>
      </div>
    </section>
  );
}
