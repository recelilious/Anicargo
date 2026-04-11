import { type FormEvent, useEffect, useState } from "react";
import { Navigate } from "react-router-dom";
import {
  Badge,
  Button,
  Card,
  Field,
  Input,
  Switch,
  Text,
  makeStyles,
} from "@fluentui/react-components";

import {
  activateAdminDownload,
  createFansubRule,
  fetchAdminDashboard,
  fetchAdminDownloadCandidates,
  fetchAdminDownloadExecutions,
  fetchAdminDownloads,
  fetchAdminExecutionEvents,
  fetchAdminRuntime,
  forceAdminDownload,
  updatePolicy,
} from "../api";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence, motionDelayStyle } from "../motion";
import { useSession } from "../session";
import type {
  AdminDashboardResponse,
  AdminRuntimeResponse,
  DownloadExecution,
  DownloadExecutionEvent,
  DownloadJob,
  ResourceCandidate,
} from "../types";

const useStyles = makeStyles({
  page: {
    minHeight: "100%",
    display: "flex",
    flexDirection: "column",
    gap: "18px",
  },
  header: {
    padding: "20px 22px",
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: "16px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px",
  },
  layout: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1.1fr) minmax(340px, 0.9fr)",
    gap: "18px",
    alignItems: "start",
  },
  column: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    minWidth: 0,
  },
  panel: {
    padding: "18px 20px",
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    minWidth: 0,
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  downloadsList: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
  },
  downloadCard: {
    padding: "14px 16px",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    backgroundColor: "var(--app-surface-2)",
    border: "1px solid var(--app-border)",
    minWidth: 0,
  },
  activeDownloadCard: {
    outline: "2px solid var(--app-selected-fg)",
  },
  cardRow: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "center",
    flexWrap: "wrap",
  },
  compactGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: "10px",
  },
  muted: {
    color: "var(--app-muted)",
  },
  actions: {
    display: "flex",
    gap: "10px",
    flexWrap: "wrap",
  },
  stack: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    minWidth: 0,
  },
  summaryValue: {
    fontSize: "22px",
    fontWeight: 700,
    lineHeight: 1.2,
  },
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
    return "No active execution";
  }

  const totalBytes = Math.max(execution.sourceSizeBytes, execution.downloadedBytes);
  if (totalBytes > 0) {
    const progress = ((execution.downloadedBytes / totalBytes) * 100).toFixed(1);
    return `${formatBytes(execution.downloadedBytes)} / ${formatBytes(totalBytes)} (${progress}%)`;
  }

  return formatBytes(execution.downloadedBytes);
}

export function ManagementPage() {
  const styles = useStyles();
  const { deviceId, userToken, displayName, isAdmin, logoutAccount } = useSession();

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
  const [forceSubjectId, setForceSubjectId] = useState("");
  const [ruleForm, setRuleForm] = useState({
    fansubName: "",
    localePreference: "zh-Hans",
    priority: 50,
    isBlacklist: false,
  });

  useLoadingStatus(isLoading ? "正在同步管理状态..." : null);

  useEffect(() => {
    if (!userToken) {
      return;
    }

    const token = userToken;
    let disposed = false;

    async function loadAdminState() {
      setIsLoading(true);

      try {
        const [dashboardResponse, runtimeResponse, downloadsResponse] = await Promise.all([
          fetchAdminDashboard(deviceId, token),
          fetchAdminRuntime(deviceId, token),
          fetchAdminDownloads(deviceId, token),
        ]);

        if (disposed) {
          return;
        }

        setDashboard(dashboardResponse);
        setRuntime(runtimeResponse);
        setDownloads(downloadsResponse.items);
        setSelectedJobId((current) =>
          downloadsResponse.items.some((job) => job.id === current) ? current : (downloadsResponse.items[0]?.id ?? null),
        );
        setError(null);
      } catch (nextError) {
        if (!disposed) {
          setError((nextError as Error).message);
        }
      } finally {
        if (!disposed) {
          setIsLoading(false);
        }
      }
    }

    void loadAdminState();
    const interval = window.setInterval(() => {
      void loadAdminState();
    }, 5000);

    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [deviceId, userToken]);

  useEffect(() => {
    if (!userToken || !selectedJobId) {
      setCandidates([]);
      setExecutions([]);
      setEvents([]);
      setSelectedExecutionId(null);
      return;
    }

    const token = userToken;
    const jobId = selectedJobId;
    let disposed = false;

    async function loadJobDetails() {
      try {
        const [candidateResponse, executionResponse] = await Promise.all([
          fetchAdminDownloadCandidates(deviceId, token, jobId),
          fetchAdminDownloadExecutions(deviceId, token, jobId),
        ]);

        if (disposed) {
          return;
        }

        setCandidates(candidateResponse.items);
        setExecutions(executionResponse.items);
        setSelectedExecutionId((current) =>
          executionResponse.items.some((execution) => execution.id === current)
            ? current
            : (executionResponse.items[0]?.id ?? null),
        );
      } catch (nextError) {
        if (!disposed) {
          setError((nextError as Error).message);
        }
      }
    }

    void loadJobDetails();

    return () => {
      disposed = true;
    };
  }, [deviceId, selectedJobId, userToken]);

  useEffect(() => {
    if (!userToken || !selectedExecutionId) {
      setEvents([]);
      return;
    }

    const token = userToken;
    const executionId = selectedExecutionId;
    let disposed = false;

    async function loadEvents() {
      try {
        const response = await fetchAdminExecutionEvents(deviceId, token, executionId);
        if (!disposed) {
          setEvents(response.items);
        }
      } catch (nextError) {
        if (!disposed) {
          setError((nextError as Error).message);
        }
      }
    }

    void loadEvents();

    return () => {
      disposed = true;
    };
  }, [deviceId, selectedExecutionId, userToken]);

  async function onPolicySave(event: FormEvent) {
    event.preventDefault();
    if (!userToken || !dashboard) {
      return;
    }

    const token = userToken;
    try {
      const policy = await updatePolicy(deviceId, token, dashboard.policy);
      setDashboard((current) => (current ? { ...current, policy } : current));
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onRuleCreate(event: FormEvent) {
    event.preventDefault();
    if (!userToken) {
      return;
    }

    const token = userToken;
    try {
      const nextRule = await createFansubRule(deviceId, token, ruleForm);
      setDashboard((current) =>
        current
          ? {
              ...current,
              fansubRules: [nextRule, ...current.fansubRules],
              counts: {
                ...current.counts,
                fansubRules: current.counts.fansubRules + 1,
              },
            }
          : current,
      );
      setRuleForm({
        fansubName: "",
        localePreference: "zh-Hans",
        priority: 50,
        isBlacklist: false,
      });
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onForceDownload() {
    if (!userToken || !forceSubjectId.trim()) {
      return;
    }

    const token = userToken;
    try {
      await forceAdminDownload(deviceId, token, Number(forceSubjectId));
      const downloadsResponse = await fetchAdminDownloads(deviceId, token);
      setDownloads(downloadsResponse.items);
      setSelectedJobId(downloadsResponse.items[0]?.id ?? null);
      setForceSubjectId("");
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onActivateDownload(jobId: number) {
    if (!userToken) {
      return;
    }

    const token = userToken;
    try {
      await activateAdminDownload(deviceId, token, jobId);
      const [executionResponse, downloadsResponse] = await Promise.all([
        fetchAdminDownloadExecutions(deviceId, token, jobId),
        fetchAdminDownloads(deviceId, token),
      ]);
      setExecutions(executionResponse.items);
      setSelectedExecutionId(executionResponse.items[0]?.id ?? null);
      setDownloads(downloadsResponse.items);
      setError(null);
    } catch (nextError) {
      setError((nextError as Error).message);
    }
  }

  async function onAccountLogout() {
    setDashboard(null);
    setRuntime(null);
    setDownloads([]);
    setCandidates([]);
    setExecutions([]);
    setEvents([]);
    setSelectedJobId(null);
    setSelectedExecutionId(null);
    await logoutAccount();
  }

  if (!isAdmin || !userToken) {
    return <Navigate to="/settings" replace />;
  }

  const selectedJob = downloads.find((job) => job.id === selectedJobId) ?? null;
  const selectedExecution = executions.find((execution) => execution.id === selectedExecutionId) ?? executions[0] ?? null;
  const currentAdminLabel = dashboard?.adminUsername ?? displayName;

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.header} app-motion-surface`}>
        <div>
          <Text weight="semibold" size={800}>
            管理
          </Text>
          <Text className={styles.muted}>管理员会话下统一查看运行状态、下载队列、策略与字幕组规则。</Text>
        </div>

        <div className={styles.actions}>
          <Text className={styles.muted}>当前管理员 {currentAdminLabel}</Text>
          <Button appearance="secondary" onClick={() => void onAccountLogout()}>
            退出管理员
          </Button>
        </div>
      </Card>

      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      <div className={styles.grid}>
        {runtime ? (
          <>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "36ms" }}>
              <Text weight="semibold">服务地址</Text>
              <Text className={styles.summaryValue}>{runtime.serverAddress}</Text>
              <Text className={styles.muted}>运行 {runtime.uptimeLabel}</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "72ms" }}>
              <Text weight="semibold">活动下载</Text>
              <Text className={styles.summaryValue}>{runtime.runtime.activeExecutions}</Text>
              <Text className={styles.muted}>{formatSpeed(runtime.runtime.downloadRateBytes)}</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "108ms" }}>
              <Text weight="semibold">下载任务</Text>
              <Text className={styles.summaryValue}>{runtime.runtime.openDownloadJobs}</Text>
              <Text className={styles.muted}>已选资源 {runtime.runtime.jobsWithSelection}</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "144ms" }}>
              <Text weight="semibold">HTTP 请求</Text>
              <Text className={styles.summaryValue}>{runtime.http.totalRequests}</Text>
              <Text className={styles.muted}>失败 {runtime.http.failedRequests}</Text>
            </Card>
          </>
        ) : null}

        {dashboard ? (
          <>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "180ms" }}>
              <Text weight="semibold">设备</Text>
              <Text className={styles.summaryValue}>{dashboard.counts.devices}</Text>
              <Text className={styles.muted}>已记录设备数</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "216ms" }}>
              <Text weight="semibold">用户</Text>
              <Text className={styles.summaryValue}>{dashboard.counts.users}</Text>
              <Text className={styles.muted}>注册用户数</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "252ms" }}>
              <Text weight="semibold">订阅</Text>
              <Text className={styles.summaryValue}>{dashboard.counts.subscriptions}</Text>
              <Text className={styles.muted}>当前订阅总数</Text>
            </Card>
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "288ms" }}>
              <Text weight="semibold">字幕组规则</Text>
              <Text className={styles.summaryValue}>{dashboard.counts.fansubRules}</Text>
              <Text className={styles.muted}>已登记规则数</Text>
            </Card>
          </>
        ) : null}
      </div>

      <div className={styles.layout}>
        <div className={styles.column}>
          <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "64ms" }}>
            <Text weight="semibold">手动触发下载</Text>
            <div className={styles.actions}>
              <Field label="Bangumi Subject ID" style={{ flex: 1 }}>
                <Input value={forceSubjectId} onChange={(_, data) => setForceSubjectId(data.value)} />
              </Field>
              <Button appearance="primary" onClick={() => void onForceDownload()}>
                立即触发
              </Button>
            </div>
          </Card>

          <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "96ms" }}>
            <Text weight="semibold">下载队列</Text>
            <div className={styles.downloadsList}>
              {downloads.map((job, index) => (
                <Card
                  key={job.id}
                  className={`${styles.downloadCard} ${selectedJobId === job.id ? styles.activeDownloadCard : ""} app-motion-item`}
                  style={motionDelayStyle(index, 24, 120)}
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
                  <Text className={styles.muted}>搜索状态 {job.searchStatus}</Text>
                  <div className={styles.actions}>
                    <Button appearance="secondary" onClick={() => void onActivateDownload(job.id)}>
                      执行已选资源
                    </Button>
                  </div>
                </Card>
              ))}
              {downloads.length === 0 ? <Text className={styles.muted}>当前没有开放的下载任务。</Text> : null}
            </div>
          </Card>

          {selectedJob ? (
            <>
              <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "128ms" }}>
                <Text weight="semibold">候选资源</Text>
                <div className={styles.stack}>
                  {candidates.map((candidate, index) => (
                    <Card key={candidate.id} className={`${styles.downloadCard} app-motion-item`} style={motionDelayStyle(index, 24, 140)}>
                      <div className={styles.cardRow}>
                        <Text weight="semibold">{candidate.fansubName ?? candidate.publisherName}</Text>
                        <Badge appearance={candidate.rejectedReason ? "outline" : "filled"}>{candidate.slotKey}</Badge>
                      </div>
                      <Text>{candidate.title}</Text>
                      <Text className={styles.muted}>
                        分数 {candidate.score.toFixed(1)} · {candidate.resolution ?? "Unknown"}
                      </Text>
                      {candidate.rejectedReason ? <Text>{candidate.rejectedReason}</Text> : null}
                    </Card>
                  ))}
                  {candidates.length === 0 ? <Text className={styles.muted}>当前任务还没有候选资源。</Text> : null}
                </div>
              </Card>

              <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "160ms" }}>
                <Text weight="semibold">执行实例</Text>
                <div className={styles.stack}>
                  {executions.map((execution, index) => (
                    <Card
                      key={execution.id}
                      className={`${styles.downloadCard} ${selectedExecutionId === execution.id ? styles.activeDownloadCard : ""} app-motion-item`}
                      style={motionDelayStyle(index, 24, 160)}
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
                  {executions.length === 0 ? <Text className={styles.muted}>当前任务还没有执行实例。</Text> : null}
                </div>
              </Card>
            </>
          ) : null}
        </div>

        <div className={styles.column}>
          {selectedExecution ? (
            <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "84ms" }}>
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

          <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "118ms" }}>
            <Text weight="semibold">执行事件</Text>
            <div className={styles.stack}>
              {events.map((event, index) => (
                <Card key={event.id} className={`${styles.downloadCard} app-motion-item`} style={motionDelayStyle(index, 20, 150)}>
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
                <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "152ms" }}>
                  <Text weight="semibold">下载策略</Text>
                  <div className={styles.form}>
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
                                    subscriptionThreshold: Number(data.value || 0),
                                  },
                                }
                              : current,
                          )
                        }
                      />
                    </Field>
                    <Field label="换源窗口（小时）">
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
                                    replacementWindowHours: Number(data.value || 0),
                                  },
                                }
                              : current,
                          )
                        }
                      />
                    </Field>
                    <Field label="最大同时下载数">
                      <Input
                        type="number"
                        value={String(dashboard.policy.maxConcurrentDownloads)}
                        onChange={(_, data) =>
                          setDashboard((current) =>
                            current
                              ? {
                                  ...current,
                                  policy: {
                                    ...current.policy,
                                    maxConcurrentDownloads: Math.max(1, Number(data.value || 0)),
                                  },
                                }
                              : current,
                          )
                        }
                      />
                    </Field>
                    <Field label="上行限速（MB/s，0 为不限速）">
                      <Input
                        type="number"
                        value={String(dashboard.policy.uploadLimitMb)}
                        onChange={(_, data) =>
                          setDashboard((current) =>
                            current
                              ? {
                                  ...current,
                                  policy: {
                                    ...current.policy,
                                    uploadLimitMb: Math.max(0, Number(data.value || 0)),
                                  },
                                }
                              : current,
                          )
                        }
                      />
                    </Field>
                    <Field label="下行限速（MB/s，0 为不限速）">
                      <Input
                        type="number"
                        value={String(dashboard.policy.downloadLimitMb)}
                        onChange={(_, data) =>
                          setDashboard((current) =>
                            current
                              ? {
                                  ...current,
                                  policy: {
                                    ...current.policy,
                                    downloadLimitMb: Math.max(0, Number(data.value || 0)),
                                  },
                                }
                              : current,
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
                                  preferSameFansub: data.checked,
                                },
                              }
                            : current,
                        )
                      }
                      label="优先延续上一集字幕组"
                    />
                    <Button type="submit" appearance="primary">
                      保存策略
                    </Button>
                  </div>
                </Card>
              </form>

              <form onSubmit={(event) => void onRuleCreate(event)}>
                <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "186ms" }}>
                  <Text weight="semibold">新增字幕组规则</Text>
                  <div className={styles.form}>
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
                  </div>
                </Card>
              </form>

              <Card className={`${styles.panel} app-motion-surface`} style={{ ["--motion-delay" as string]: "220ms" }}>
                <Text weight="semibold">现有字幕组规则</Text>
                <div className={styles.stack}>
                  {dashboard.fansubRules.map((rule, index) => (
                    <Card key={rule.id} className={`${styles.downloadCard} app-motion-item`} style={motionDelayStyle(index, 20, 180)}>
                      <div className={styles.cardRow}>
                        <Text weight="semibold">{rule.fansubName}</Text>
                        <Badge appearance={rule.isBlacklist ? "filled" : "outline"}>
                          {rule.isBlacklist ? "黑名单" : `P${rule.priority}`}
                        </Badge>
                      </div>
                      <Text className={styles.muted}>{rule.localePreference}</Text>
                    </Card>
                  ))}
                  {dashboard.fansubRules.length === 0 ? <Text className={styles.muted}>当前还没有字幕组规则。</Text> : null}
                </div>
              </Card>
            </>
          ) : null}
        </div>
      </div>
    </MotionPage>
  );
}
