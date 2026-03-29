import { useEffect, useState } from "react";
import { ArrowLeftRegular } from "@fluentui/react-icons";
import { Badge, Button, Card, Spinner, Text, makeStyles, tokens } from "@fluentui/react-components";
import { useLocation, useNavigate, useParams } from "react-router-dom";

import { fetchSubjectDetail, toggleSubscription } from "../api";
import { EpisodeCard } from "../components/EpisodeCard";
import { resolveReturnScrollTop, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { SubjectDetailResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px",
  },
  hero: {
    position: "relative",
    overflow: "hidden",
    minHeight: "320px",
    padding: "28px",
    color: "#ffffff",
    boxShadow: "var(--app-card-shadow-strong)",
    border: "1px solid var(--app-border)",
  },
  heroBackdrop: {
    position: "absolute",
    inset: 0,
    backgroundSize: "cover",
    backgroundPosition: "center center",
    filter: "blur(14px)",
    transform: "scale(1.06)",
  },
  heroOverlay: {
    position: "absolute",
    inset: 0,
    backgroundColor: "rgba(18, 10, 8, 0.68)",
  },
  heroContent: {
    position: "relative",
    display: "flex",
    flexDirection: "column",
    gap: "16px",
  },
  heroTopRow: {
    display: "flex",
    justifyContent: "flex-start",
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    alignItems: "flex-start",
  },
  subtitle: {
    color: "rgba(255, 245, 238, 0.86)",
  },
  badges: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap",
  },
  buttonRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
    flexWrap: "wrap",
  },
  subscribeButton: {
    alignSelf: "flex-start",
    minWidth: "132px",
  },
  disabledSubscribeButton: {
    alignSelf: "flex-start",
    minWidth: "132px",
    opacity: 0.72,
    cursor: "not-allowed",
  },
  backButton: {
    minWidth: "108px",
  },
  heroInfo: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    marginTop: "2px",
  },
  statGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
    gap: "12px",
  },
  statCard: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    padding: "14px 16px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.1)",
    border: "1px solid rgba(255, 244, 236, 0.18)",
    backdropFilter: "blur(10px)",
  },
  statLabel: {
    display: "block",
    color: "rgba(255, 245, 238, 0.72)",
  },
  summaryCard: {
    padding: "16px 18px",
    borderRadius: tokens.borderRadiusXLarge,
    backgroundColor: "rgba(255, 248, 241, 0.08)",
    border: "1px solid rgba(255, 244, 236, 0.16)",
    backdropFilter: "blur(10px)",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
  },
  summaryText: {
    color: "rgba(255, 245, 238, 0.92)",
    lineHeight: "1.6",
  },
  infoGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px",
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
    gap: "6px",
  },
  infoValue: {
    display: "block",
    maxWidth: "100%",
    color: "rgba(255, 245, 238, 0.92)",
    lineHeight: "1.5",
    whiteSpace: "normal",
    wordBreak: "normal",
    overflowWrap: "anywhere",
  },
  episodesSection: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
  },
  episodesGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px",
  },
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

function resolveSubscriptionAction(detail: SubjectDetailResponse) {
  if (detail.subject.releaseStatus === "upcoming") {
    return {
      label: "未播出",
      disabled: true,
    };
  }

  return {
    label: detail.subscription.isSubscribed ? "取消订阅" : "订阅",
    disabled: false,
  };
}

export function SubjectPage() {
  const styles = useStyles();
  const navigate = useNavigate();
  const location = useLocation();
  const { subjectId } = useParams();
  const { deviceId, userToken } = useSession();
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubscribing, setIsSubscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const routeState = (location.state as RouteState | null) ?? null;

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

  async function handleToggleSubscription() {
    if (!subjectId || !detail) {
      return;
    }

    const previousSubscription = detail.subscription;
    const nextIsSubscribed = !previousSubscription.isSubscribed;
    const optimisticCount = Math.max(
      0,
      previousSubscription.subscriptionCount + (nextIsSubscribed ? 1 : -1),
    );

    setDetail((current) =>
      current
        ? {
            ...current,
            subscription: {
              ...current.subscription,
              isSubscribed: nextIsSubscribed,
              subscriptionCount: optimisticCount,
            },
          }
        : current,
    );
    setIsSubscribing(true);
    setError(null);

    try {
      const response = await toggleSubscription(Number(subjectId), deviceId, userToken);
      setDetail((current) =>
        current
          ? {
              ...current,
              subscription: response.subscription,
            }
          : current,
      );
    } catch (nextError) {
      setDetail((current) =>
        current
          ? {
              ...current,
              subscription: previousSubscription,
            }
          : current,
      );
      setError((nextError as Error).message);
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
      state: typeof restoreScrollTop === "number" ? ({ restoreScrollTop } satisfies RouteState) : undefined,
    });
  }

  if (isLoading) {
    return <Spinner label="正在加载条目..." />;
  }

  if (!detail) {
    return <Text>{error ?? "条目不存在。"}</Text>;
  }

  const subscriptionAction = resolveSubscriptionAction(detail);

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
            backgroundColor: "var(--app-fallback-hero)",
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
              appearance={subscriptionAction.disabled ? "secondary" : "primary"}
              className={subscriptionAction.disabled ? styles.disabledSubscribeButton : styles.subscribeButton}
              onClick={subscriptionAction.disabled ? undefined : handleToggleSubscription}
              disabled={subscriptionAction.disabled || isSubscribing}
            >
              {subscriptionAction.label}
            </Button>
          </div>

          <div className={styles.heroInfo}>
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
                    <Text weight="semibold" className={styles.infoValue}>
                      {item.value}
                    </Text>
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        </div>
      </Card>

      {error ? <Text>{error}</Text> : null}

      <div className={styles.episodesSection}>
        <Text weight="semibold" size={700}>
          剧集
        </Text>

        <div className={styles.episodesGrid}>
          {detail.episodes.map((episode) => (
            <EpisodeCard key={episode.bangumiEpisodeId} subjectId={detail.subject.bangumiSubjectId} episode={episode} />
          ))}
        </div>
      </div>
    </section>
  );
}
