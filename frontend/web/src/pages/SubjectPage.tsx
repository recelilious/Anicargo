import { useEffect, useState } from "react";
import { ArrowLeftRegular } from "@fluentui/react-icons";
import { Badge, Button, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { useLocation, useNavigate, useParams } from "react-router-dom";

import { toggleSubscription } from "../api";
import { EpisodeCard } from "../components/EpisodeCard";
import { SubjectCard } from "../components/SubjectCard";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence } from "../motion";
import { buildRoutePath, consumeReturnTarget, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { SubjectCard as SubjectCardModel, SubjectDetailResponse } from "../types";
import {
  fetchSubjectDetailCached,
  getCachedSubjectDetail,
  getCachedSubjectPreview,
  revalidateSubjectDetail,
  subjectCardFromDetail,
  subjectDetailPreviewFromCard,
} from "../view-cache";

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
  relatedSection: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
  },
  relatedGrid: {
    display: "grid",
    gridTemplateColumns:
      "repeat(auto-fill, minmax(var(--app-subject-card-fixed-width), var(--app-subject-card-fixed-width)))",
    gap: "12px",
    justifyContent: "flex-start",
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

function formatThemeList(values: string[]) {
  return values
    .map((value) => value.trim())
    .filter(Boolean)
    .join(" / ");
}

function shouldHideInfoboxItem(key: string) {
  const normalized = key.trim().toLowerCase();
  return (
    normalized === "\u4e2d\u6587\u540d" ||
    normalized === "\u4e0a\u6620\u5e74\u5ea6"
  );
}

export function SubjectPage() {
  const styles = useStyles();
  const navigate = useNavigate();
  const location = useLocation();
  const { subjectId } = useParams();
  const { deviceId, userToken } = useSession();
  const numericSubjectId = subjectId ? Number(subjectId) : null;
  const [preview, setPreview] = useState<SubjectCardModel | null>(() =>
    numericSubjectId != null ? getCachedSubjectPreview(numericSubjectId) : null,
  );
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(() =>
    numericSubjectId != null ? getCachedSubjectDetail(numericSubjectId, deviceId, userToken) : null,
  );
  const [isLoading, setIsLoading] = useState(() =>
    numericSubjectId != null ? getCachedSubjectDetail(numericSubjectId, deviceId, userToken) == null : false,
  );
  const [isSubscribing, setIsSubscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useLoadingStatus(isLoading ? "正在加载条目..." : isSubscribing ? "正在更新订阅..." : null);
  const currentPath = buildRoutePath(location);

  useEffect(() => {
    if (numericSubjectId == null) {
      return;
    }

    let cancelled = false;
    const cachedPreview = getCachedSubjectPreview(numericSubjectId);
    const cachedDetail = getCachedSubjectDetail(numericSubjectId, deviceId, userToken);

    setPreview(cachedPreview);
    setDetail(cachedDetail);
    setError(null);
    setIsLoading(cachedDetail == null);

    const request = cachedDetail
      ? revalidateSubjectDetail(numericSubjectId, deviceId, userToken)
      : fetchSubjectDetailCached(numericSubjectId, deviceId, userToken);

    void request
      .then((response) => {
        if (!cancelled) {
          setDetail(response);
          setPreview(subjectCardFromDetail(response.subject));
          setError(null);
        }
      })
      .catch((nextError: Error) => {
        if (!cancelled) {
          setError(nextError.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [deviceId, numericSubjectId, userToken]);

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
    const target = consumeReturnTarget(currentPath);
    if (!target) {
      navigate(-1);
      return;
    }

    navigate(target.fromPath, {
      replace: true,
      state: { restoreScrollTop: target.scrollTop } satisfies RouteState,
    });
  }

  const displaySubject = detail?.subject ?? (preview ? subjectDetailPreviewFromCard(preview) : null);

  if (!displaySubject) {
    return <Text>{error ?? "Loading..."}</Text>;
  }

  if (!detail && !preview && !isLoading) {
    return <Text>{error ?? "条目不存在。"}</Text>;
  }

  const subscriptionAction = detail
    ? resolveSubscriptionAction(detail)
    : {
        label: "Loading...",
        disabled: true,
      };
  const visibleInfobox = detail
    ? detail.subject.infobox.filter((item) => !shouldHideInfoboxItem(item.key))
    : [];
  const openingThemes = detail ? formatThemeList(detail.subject.openingThemes) : "";
  const endingThemes = detail ? formatThemeList(detail.subject.endingThemes) : "";

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.hero} app-motion-surface`}>
        <div
          className={styles.heroBackdrop}
          style={{
            backgroundImage: displaySubject.imageBanner
              ? `url(${displaySubject.imageBanner})`
              : displaySubject.imagePortrait
                ? `url(${displaySubject.imagePortrait})`
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
              {displaySubject.titleCn || displaySubject.title}
            </Text>
            <Text className={styles.subtitle}>{displaySubject.title}</Text>
          </div>

          {displaySubject.tags.length > 0 ? (
            <div className={styles.badges}>
              {displaySubject.tags.map((tag) => (
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
                  {detail ? `${detail.subscription.subscriptionCount} / ${detail.subscription.threshold}` : "-- / --"}
                </Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  归属
                </Text>
                <Text weight="semibold">
                  {detail ? formatSubscriptionSource(detail.subscription.source) : "--"}
                </Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  放送
                </Text>
                <Text weight="semibold">{formatBroadcast(displaySubject)}</Text>
              </div>

              <div className={styles.statCard}>
                <Text size={200} className={styles.statLabel}>
                  评分
                </Text>
                <Text weight="semibold">{formatRating(displaySubject.ratingScore)}</Text>
              </div>
            </div>

            {displaySubject.summary ? (
              <div className={styles.summaryCard}>
                <Text size={200} className={styles.statLabel}>
                  简介
                </Text>
                <Text className={styles.summaryText}>{displaySubject.summary}</Text>
              </div>
            ) : null}

            {openingThemes || endingThemes || visibleInfobox.length > 0 ? (
              <div className={styles.infoGrid}>
                {openingThemes ? (
                  <div className={styles.infoCard}>
                    <Text size={200} className={styles.statLabel}>
                      {"\u7247\u5934\u66f2"}
                    </Text>
                    <Text weight="semibold" className={styles.infoValue}>
                      {openingThemes}
                    </Text>
                  </div>
                ) : null}

                {endingThemes ? (
                  <div className={styles.infoCard}>
                    <Text size={200} className={styles.statLabel}>
                      {"\u7247\u5c3e\u66f2"}
                    </Text>
                    <Text weight="semibold" className={styles.infoValue}>
                      {endingThemes}
                    </Text>
                  </div>
                ) : null}

                {visibleInfobox.map((item) => (
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

      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      <div className={`${styles.episodesSection} app-motion-surface`} style={{ ["--motion-delay" as string]: "56ms" }}>
        <Text weight="semibold" size={700}>
          剧集
        </Text>

        <div className={styles.episodesGrid}>
          {detail ? (
            detail.episodes.map((episode, index) => (
              <EpisodeCard
                key={episode.bangumiEpisodeId}
                subjectId={detail.subject.bangumiSubjectId}
                episode={episode}
                motionIndex={index}
              />
            ))
          ) : (
            <Text>Loading episodes...</Text>
          )}
        </div>
      </div>
      {detail && detail.subject.relatedSubjects.length > 0 ? (
        <div
          className={`${styles.relatedSection} app-motion-surface`}
          style={{ ["--motion-delay" as string]: "84ms" }}
        >
          <Text weight="semibold" size={700}>
            {"\u5173\u8054\u52a8\u753b"}
          </Text>

          <div className={styles.relatedGrid}>
            {detail.subject.relatedSubjects.map((subject, index) => (
              <SubjectCard
                key={`${subject.catalogLabel ?? "related"}-${subject.bangumiSubjectId}`}
                subject={subject}
                metaVariant="related"
                motionIndex={index}
              />
            ))}
          </div>
        </div>
      ) : null}
    </MotionPage>
  );
}
