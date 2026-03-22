import { useEffect, useRef, useState } from "react";
import { ArrowLeftRegular } from "@fluentui/react-icons";
import { Badge, Button, Card, Spinner, Text, makeStyles, tokens } from "@fluentui/react-components";
import { useLocation, useNavigate, useParams } from "react-router-dom";

import { fetchSubjectDetail, fetchSubjectDownloadStatus, toggleSubscription } from "../api";
import { EpisodeCard } from "../components/EpisodeCard";
import { resolveReturnScrollTop, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { SubjectDetailResponse, SubjectDownloadStatus } from "../types";

const ACTIVE_LIFECYCLES = new Set(["queued", "planning", "searching", "staged", "starting", "downloading"]);

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px"
  },
  hero: {
    position: "relative",
    overflow: "hidden",
    minHeight: "320px",
    padding: "28px",
    color: "#ffffff",
    boxShadow: "var(--app-card-shadow-strong)",
    border: "1px solid var(--app-border)"
  },
  heroBackdrop: {
    position: "absolute",
    inset: 0,
    backgroundSize: "cover",
    backgroundPosition: "center center",
    filter: "blur(14px)",
    transform: "scale(1.06)"
  },
  heroOverlay: {
    position: "absolute",
    inset: 0,
    backgroundColor: "rgba(18, 10, 8, 0.68)"
  },
  heroContent: {
    position: "relative",
    display: "flex",
    flexDirection: "column",
    gap: "16px"
  },
  heroTopRow: {
    display: "flex",
    justifyContent: "flex-start"
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    alignItems: "flex-start"
  },
  subtitle: {
    color: "rgba(255, 245, 238, 0.86)"
  },
  badges: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap"
  },
  buttonRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
    flexWrap: "wrap"
  },
  subscribeButton: {
    alignSelf: "flex-start",
    minWidth: "132px"
  },
  backButton: {
    minWidth: "108px"
  },
  heroInfo: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    marginTop: "2px"
  },
  statusCard: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    padding: "16px 18px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.1)",
    border: "1px solid rgba(255, 244, 236, 0.18)",
    backdropFilter: "blur(10px)"
  },
  statusHeader: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "flex-start",
    flexWrap: "wrap"
  },
  statusGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(140px, 1fr))",
    gap: "12px"
  },
  statusMetric: {
    display: "flex",
    flexDirection: "column",
    gap: "6px"
  },
  noticeCard: {
    padding: "14px 16px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(214, 195, 178, 0.18)",
    border: "1px solid rgba(255, 244, 236, 0.24)",
    display: "flex",
    flexDirection: "column",
    gap: "6px"
  },
  statGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: "12px"
  },
  statCard: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    padding: "14px 16px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.1)",
    border: "1px solid rgba(255, 244, 236, 0.18)",
    backdropFilter: "blur(10px)"
  },
  statLabel: {
    display: "block",
    color: "rgba(255, 245, 238, 0.72)"
  },
  summaryCard: {
    padding: "16px 18px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.08)",
    border: "1px solid rgba(255, 244, 236, 0.16)",
    backdropFilter: "blur(10px)",
    display: "flex",
    flexDirection: "column",
    gap: "8px"
  },
  summaryText: {
    color: "rgba(255, 245, 238, 0.92)",
    lineHeight: "1.6"
  },
  infoGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  },
  infoCard: {
    minWidth: 0,
    padding: "14px 16px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.08)",
    border: "1px solid rgba(255, 244, 236, 0.16)",
    backdropFilter: "blur(10px)",
    display: "flex",
    flexDirection: "column",
    gap: "6px"
  },
  infoValue: {
    display: "block",
    maxWidth: "100%",
    color: "rgba(255, 245, 238, 0.92)",
    lineHeight: "1.5",
    whiteSpace: "normal",
    wordBreak: "normal",
    overflowWrap: "anywhere"
  },
  episodesSection: {
    display: "flex",
    flexDirection: "column",
    gap: "14px"
  },
  episodesGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  }
});

function formatRating(score: number | null) {
  return score == null ? "暂无评分" : score.toFixed(1);
}

function formatBroadcast(detail: SubjectDetailResponse["subject"]) {
  if (detail.broadcastTime) {
    return detail.airDate ? `${detail.airDate} ${detail.broadcastTime}` : detail.broadcastTime;
  }

  return detail.airDate ?? "未知";
}

function formatSubscriptionSource(source: SubjectDetailResponse["subscription"]["source"]) {
  return source.kind === "user" ? "账号订阅" : "设备订阅";
}

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

function formatDownloadLifecycle(status: SubjectDownloadStatus | null) {
  if (!status) {
    return "未进入下载流程";
  }

  switch (status.jobLifecycle) {
    case "queued":
      return "已进入队列";
    case "planning":
      return "正在规划";
    case "searching":
      return "正在搜索资源";
    case "staged":
      return "资源已就绪，等待执行";
    case "starting":
      return "下载引擎启动中";
    case "downloading":
      return "正在下载";
    case "seeding":
      return "已完成，正在做种";
    case "completed":
      return "已完成";
    case "failed":
      return "下载失败";
    default:
      return status.demandState === "threshold_met" ? "等待入队" : "等待订阅阈值";
  }
}

function formatDownloadProgress(status: SubjectDownloadStatus | null) {
  if (!status) {
    return "暂无下载进度";
  }

  const totalBytes = Math.max(status.totalBytes, status.downloadedBytes);
  if (totalBytes > 0) {
    const progress = ((status.downloadedBytes / totalBytes) * 100).toFixed(1);
    return `${formatBytes(status.downloadedBytes)} / ${formatBytes(totalBytes)} (${progress}%)`;
  }

  return formatBytes(status.downloadedBytes);
}

export function SubjectPage() {
  const styles = useStyles();
  const navigate = useNavigate();
  const location = useLocation();
  const { subjectId } = useParams();
  const { deviceId, userToken } = useSession();
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [downloadStatus, setDownloadStatus] = useState<SubjectDownloadStatus | null>(null);
  const [downloadNotice, setDownloadNotice] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubscribing, setIsSubscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const routeState = (location.state as RouteState | null) ?? null;
  const readyMediaCountRef = useRef<number | null>(null);

  useEffect(() => {
    if (!subjectId) {
      return;
    }

    let isMounted = true;
    setIsLoading(true);

    void fetchSubjectDetail(Number(subjectId), deviceId, userToken)
      .then((response) => {
        if (isMounted) {
          setDetail(response);
          setDownloadStatus(response.downloadStatus);
          readyMediaCountRef.current = response.downloadStatus?.readyMediaCount ?? 0;
          setError(null);
        }
      })
      .catch((nextError: Error) => {
        if (isMounted) {
          setError(nextError.message);
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
  }, [deviceId, subjectId, userToken]);

  useEffect(() => {
    if (!subjectId || !detail) {
      return;
    }

    const shouldPoll =
      detail.subscription.isSubscribed ||
      (downloadStatus?.jobLifecycle != null && ACTIVE_LIFECYCLES.has(downloadStatus.jobLifecycle));

    if (!shouldPoll) {
      return;
    }

    let isMounted = true;
    const interval = window.setInterval(() => {
      void fetchSubjectDownloadStatus(Number(subjectId), deviceId, userToken).then((status) => {
        if (!isMounted) {
          return;
        }

        const previousReadyCount = readyMediaCountRef.current ?? 0;
        const nextReadyCount = status?.readyMediaCount ?? 0;
        if (nextReadyCount > previousReadyCount) {
          setDownloadNotice(
            nextReadyCount > 0
              ? `订阅资源已入库，当前已有 ${nextReadyCount} 个可用文件。`
              : "订阅资源状态已更新。"
          );
        }
        readyMediaCountRef.current = nextReadyCount;
        setDownloadStatus(status);
      });
    }, 5000);

    return () => {
      isMounted = false;
      window.clearInterval(interval);
    };
  }, [detail, deviceId, downloadStatus?.jobLifecycle, subjectId, userToken]);

  async function handleToggleSubscription() {
    if (!subjectId) {
      return;
    }

    setIsSubscribing(true);
    try {
      const response = await toggleSubscription(Number(subjectId), deviceId, userToken);
      setDetail((current) =>
        current
          ? {
              ...current,
              subscription: response.subscription
            }
          : current
      );
      const nextStatus = await fetchSubjectDownloadStatus(Number(subjectId), deviceId, userToken);
      setDownloadStatus(nextStatus);
      readyMediaCountRef.current = nextStatus?.readyMediaCount ?? 0;
    } finally {
      setIsSubscribing(false);
    }
  }

  function handleBack() {
    if (!routeState?.fromPath) {
      navigate(-1);
      return;
    }

    const restoreScrollTop = resolveReturnScrollTop(routeState.fromPath);
    navigate(routeState.fromPath, {
      state: typeof restoreScrollTop === "number" ? ({ restoreScrollTop } satisfies RouteState) : undefined
    });
  }

  if (isLoading) {
    return <Spinner label="正在加载条目..." />;
  }

  if (!detail) {
    return <Text>{error ?? "条目不存在。"}</Text>;
  }

  return (
    <section className={styles.page}>
      <Card className={styles.hero}>
        <div
          className={styles.heroBackdrop}
          style={{
            backgroundImage: detail.subject.imageBanner
              ? `url(${detail.subject.imageBanner})`
              : detail.subject.imagePortrait
                ? `url(${detail.subject.imagePortrait})`
                : undefined,
            backgroundColor: "var(--app-fallback-hero)"
          }}
        />
        <div className={styles.heroOverlay} />

        <div className={styles.heroContent}>
          <div className={styles.heroTopRow}>
            <Button appearance="primary" className={styles.backButton} icon={<ArrowLeftRegular />} onClick={handleBack}>
              返回
            </Button>
          </div>

          <div className={styles.titleGroup}>
            <Text weight="semibold" size={900}>
              {detail.subject.titleCn || detail.subject.title}
            </Text>
            <Text className={styles.subtitle}>{detail.subject.title}</Text>
          </div>

          {detail.subject.tags.length > 0 ? (
            <div className={styles.badges}>
              {detail.subject.tags.map((tag) => (
                <Badge key={tag} appearance="filled">
                  {tag}
                </Badge>
              ))}
            </div>
          ) : null}

          <div className={styles.buttonRow}>
            <Button
              appearance="primary"
              className={styles.subscribeButton}
              onClick={handleToggleSubscription}
              disabled={isSubscribing}
            >
              {detail.subscription.isSubscribed ? "取消订阅" : "订阅"}
            </Button>
          </div>

          <div className={styles.heroInfo}>
            {downloadStatus ? (
              <div className={styles.statusCard}>
                <div className={styles.statusHeader}>
                  <div>
                    <Text size={200} className={styles.statLabel}>
                      下载状态
                    </Text>
                    <Text weight="semibold">{formatDownloadLifecycle(downloadStatus)}</Text>
                  </div>
                  <Badge appearance={downloadStatus.readyMediaCount > 0 ? "filled" : "outline"}>
                    {downloadStatus.readyMediaCount > 0 ? "已有可播放资源" : "等待资源"}
                  </Badge>
                </div>

                <div className={styles.statusGrid}>
                  <div className={styles.statusMetric}>
                    <Text size={200} className={styles.statLabel}>
                      当前进度
                    </Text>
                    <Text weight="semibold">{formatDownloadProgress(downloadStatus)}</Text>
                  </div>

                  <div className={styles.statusMetric}>
                    <Text size={200} className={styles.statLabel}>
                      下载速度
                    </Text>
                    <Text weight="semibold">{formatSpeed(downloadStatus.downloadRateBytes)}</Text>
                  </div>

                  <div className={styles.statusMetric}>
                    <Text size={200} className={styles.statLabel}>
                      Peer
                    </Text>
                    <Text weight="semibold">{downloadStatus.peerCount}</Text>
                  </div>

                  <div className={styles.statusMetric}>
                    <Text size={200} className={styles.statLabel}>
                      已入库文件
                    </Text>
                    <Text weight="semibold">{downloadStatus.readyMediaCount}</Text>
                  </div>
                </div>

                {downloadStatus.sourceTitle ? (
                  <Text className={styles.subtitle}>
                    当前资源：{downloadStatus.sourceTitle}
                    {downloadStatus.sourceFansubName ? ` · ${downloadStatus.sourceFansubName}` : ""}
                  </Text>
                ) : null}
              </div>
            ) : null}

            {downloadNotice ? (
              <div className={styles.noticeCard}>
                <Text size={200} className={styles.statLabel}>
                  订阅更新
                </Text>
                <Text weight="semibold">{downloadNotice}</Text>
              </div>
            ) : null}

            <div className={styles.statGrid}>
              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  订阅
                </Text>
                <Text weight="semibold">
                  {detail.subscription.subscriptionCount} / {detail.subscription.threshold}
                </Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  归属
                </Text>
                <Text weight="semibold">{formatSubscriptionSource(detail.subscription.source)}</Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  放送
                </Text>
                <Text weight="semibold">{formatBroadcast(detail.subject)}</Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  评分
                </Text>
                <Text weight="semibold">{formatRating(detail.subject.ratingScore)}</Text>
              </div>
            </div>

            {detail.subject.summary ? (
              <div className={styles.summaryCard}>
                <Text size={200} className={styles.statLabel}>
                  简介
                </Text>
                <Text className={styles.summaryText}>{detail.subject.summary}</Text>
              </div>
            ) : null}

            {detail.subject.infobox.length > 0 ? (
              <div className={styles.infoGrid}>
                {detail.subject.infobox.map((item) => (
                  <div key={`${item.key}-${item.value}`} className={styles.infoCard}>
                    <Text size={200} className={styles.statLabel}>
                      {item.key}
                    </Text>
                    <Text className={styles.infoValue}>{item.value}</Text>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      </Card>

      <section className={styles.episodesSection}>
        <Text weight="semibold" size={700}>
          剧集
        </Text>

        <div className={styles.episodesGrid}>
          {detail.episodes.map((episode) => (
            <EpisodeCard key={episode.bangumiEpisodeId} episode={episode} subjectId={detail.subject.bangumiSubjectId} />
          ))}
        </div>
      </section>
    </section>
  );
}
