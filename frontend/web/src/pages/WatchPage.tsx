import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeftRegular } from "@fluentui/react-icons";
import { Badge, Button, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";

import {
  buildApiUrl,
  fetchEpisodePlayback,
  fetchSubjectDetail,
  recordPlaybackHistory,
} from "../api";
import { AnicargoPlayer } from "../components/AnicargoPlayer";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, motionDelayStyle } from "../motion";
import { resolveReturnScrollTop, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { Episode, EpisodePlaybackResponse, SubjectDetailResponse } from "../types";

const useStyles = makeStyles({
  page: {
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    gap: "18px",
  },
  topBar: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    gap: "14px",
    alignItems: "center",
    padding: "18px 20px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  backButton: {
    minWidth: "108px",
  },
  headingGroup: {
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    gap: "6px",
  },
  titleLine: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  subtitleLine: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    color: "var(--app-muted)",
  },
  metaRow: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: "10px",
    minWidth: 0,
  },
  metaBadge: {
    padding: "10px 14px",
    borderRadius: tokens.borderRadiusXLarge,
    border: "1px solid var(--app-border)",
    backgroundColor: "var(--app-surface-2)",
    maxWidth: "280px",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  contentGrid: {
    minHeight: 0,
    height: "100%",
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) clamp(280px, 24vw, 360px)",
    gap: "18px",
    overflow: "hidden",
    "@media (max-width: 1120px)": {
      gridTemplateColumns: "minmax(0, 1fr) 300px",
    },
    "@media (max-width: 940px)": {
      gridTemplateColumns: "1fr",
      gridTemplateRows: "minmax(0, 1fr) minmax(220px, 34vh)",
    },
  },
  playerPanel: {
    minWidth: 0,
    minHeight: 0,
    padding: "14px",
    display: "flex",
    alignItems: "stretch",
    justifyContent: "center",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    overflow: "hidden",
  },
  playerViewport: {
    width: "100%",
    height: "100%",
    minHeight: 0,
    display: "flex",
    alignItems: "flex-start",
    justifyContent: "center",
  },
  playerFrame: {
    width: "100%",
    maxWidth: "100%",
    maxHeight: "100%",
    aspectRatio: "16 / 9",
    borderRadius: "22px",
    overflow: "hidden",
    backgroundColor: "#070a10",
    border: "1px solid rgba(255, 255, 255, 0.06)",
    boxShadow: "0 20px 44px rgba(0, 0, 0, 0.28)",
  },
  fallbackSurface: {
    width: "100%",
    height: "100%",
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    alignItems: "center",
    justifyContent: "center",
    padding: "30px",
    color: "#eef3f8",
    textAlign: "center",
  },
  sidebar: {
    minWidth: 0,
    minHeight: 0,
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    gap: "12px",
    padding: "18px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    overflow: "hidden",
  },
  sidebarHeader: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "center",
  },
  episodeList: {
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    display: "grid",
    gridAutoRows: "112px",
    gap: "10px",
    paddingRight: "6px",
  },
  episodeLink: {
    color: "inherit",
    textDecorationLine: "none",
  },
  episodeCard: {
    height: "100%",
    padding: "14px 16px",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    justifyContent: "space-between",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    transitionDuration: "180ms",
    transitionProperty: "transform, border-color, background-color",
    transitionTimingFunction: "ease",
    minWidth: 0,
    ":hover": {
      transform: "translateY(-2px)",
      border: "1px solid var(--app-border-strong)",
      backgroundColor: "var(--app-surface-2)",
    },
  },
  episodeCardActive: {
    backgroundColor: "var(--app-selected-bg)",
    border: "1px solid var(--app-border-strong)",
  },
  episodeTop: {
    display: "flex",
    justifyContent: "space-between",
    gap: "10px",
    alignItems: "flex-start",
  },
  episodeTitleWrap: {
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  episodeTitle: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitLineClamp: "2",
    WebkitBoxOrient: "vertical",
    lineHeight: "1.45",
  },
  episodeMeta: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minWidth: 0,
  },
  singleLine: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  emptyState: {
    minHeight: 0,
    height: "100%",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    textAlign: "center",
    color: "var(--app-muted)",
    padding: "18px",
    border: "1px dashed var(--app-border)",
    borderRadius: "18px",
    backgroundColor: "var(--app-surface-2)",
  },
  muted: {
    color: "var(--app-muted)",
  },
});

function formatEpisodeLabel(episode: Episode | null) {
  if (!episode) {
    return "剧集";
  }

  return `第 ${episode.episodeNumber ?? episode.sort} 集`;
}

function formatUpdatedAt(value: string | null | undefined) {
  if (!value) {
    return "更新时间未知";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function WatchPage() {
  const styles = useStyles();
  const navigate = useNavigate();
  const location = useLocation();
  const { subjectId, episodeId } = useParams();
  const { deviceId, userToken } = useSession();
  const hasRecordedPlaybackRef = useRef(false);
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [episode, setEpisode] = useState<Episode | null>(null);
  const [playback, setPlayback] = useState<EpisodePlaybackResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  useLoadingStatus(isLoading ? "正在准备播放..." : null);
  const routeState = (location.state as RouteState | null) ?? null;

  useEffect(() => {
    if (!subjectId || !episodeId) {
      return;
    }

    let isMounted = true;
    setIsLoading(true);
    setError(null);
    hasRecordedPlaybackRef.current = false;

    const numericSubjectId = Number(subjectId);
    const numericEpisodeId = Number(episodeId);

    void Promise.all([
      fetchSubjectDetail(numericSubjectId, deviceId, userToken),
      fetchEpisodePlayback(numericSubjectId, numericEpisodeId, deviceId, userToken),
    ])
      .then(([detailResponse, playbackResponse]) => {
        if (!isMounted) {
          return;
        }

        setDetail(detailResponse);
        setEpisode(
          detailResponse.episodes.find((item) => item.bangumiEpisodeId === numericEpisodeId) ?? null,
        );
        setPlayback(playbackResponse);
      })
      .catch((requestError: Error) => {
        if (!isMounted) {
          return;
        }

        setError(requestError.message);
      })
      .finally(() => {
        if (isMounted) {
          setIsLoading(false);
        }
      });

    return () => {
      isMounted = false;
    };
  }, [subjectId, episodeId, deviceId, userToken]);

  async function handlePlaybackStart() {
    if (!subjectId || !episodeId || !playback?.media || hasRecordedPlaybackRef.current) {
      return;
    }

    hasRecordedPlaybackRef.current = true;

    try {
      await recordPlaybackHistory(
        {
          bangumiSubjectId: Number(subjectId),
          bangumiEpisodeId: Number(episodeId),
          mediaInventoryId: playback.media.mediaInventoryId,
        },
        deviceId,
        userToken,
      );
    } catch (recordError) {
      hasRecordedPlaybackRef.current = false;
      console.warn("Failed to record playback history", recordError);
    }
  }

  const fallbackBackPath = subjectId ? `/title/${subjectId}` : "/";
  const backPath = routeState?.fromPath ?? fallbackBackPath;

  function handleBack() {
    const restoreScrollTop = resolveReturnScrollTop(backPath);
    navigate(backPath, {
      state: typeof restoreScrollTop === "number" ? ({ restoreScrollTop } satisfies RouteState) : undefined,
    });
  }

  const watchRouteState = useMemo<RouteState>(
    () => ({
      fromPath: backPath,
    }),
    [backPath],
  );

  const visibleEpisodes = useMemo(
    () => (detail?.episodes ?? []).filter((item) => item.isAvailable),
    [detail],
  );

  const streamUrl = playback?.media ? buildApiUrl(playback.media.streamUrl) : null;
  const posterUrl = detail?.subject.imageBanner ?? detail?.subject.imagePortrait ?? null;
  const pageTitle = detail?.subject.titleCn || detail?.subject.title || "播放";
  const episodeLabel = formatEpisodeLabel(episode);
  const episodeTitle = episode?.titleCn || episode?.title || "未命名剧集";
  const fansubLabel = playback?.media?.sourceFansubName ?? "来源未知";
  const updatedLabel = formatUpdatedAt(playback?.media?.updatedAt);

  if (isLoading) {
    return null;
  }

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.topBar} app-motion-surface`}>
        <Button appearance="primary" className={styles.backButton} icon={<ArrowLeftRegular />} onClick={handleBack}>
          返回
        </Button>

        <div className={styles.headingGroup}>
          <Text weight="semibold" size={800} className={styles.titleLine}>
            {pageTitle} · {episodeLabel}
          </Text>
          <Text className={styles.subtitleLine}>{episodeTitle}</Text>
        </div>

        <div className={styles.metaRow}>
          <Text className={styles.metaBadge}>{fansubLabel}</Text>
          <Text className={styles.metaBadge}>{updatedLabel}</Text>
        </div>
      </Card>

      <div className={styles.contentGrid}>
        <Card className={`${styles.playerPanel} app-motion-surface`} style={{ ["--motion-delay" as string]: "48ms" }}>
          <div className={styles.playerViewport}>
            <div className={styles.playerFrame}>
              {streamUrl ? (
                <AnicargoPlayer
                  streamUrl={streamUrl}
                  posterUrl={posterUrl}
                  onPlaybackStart={() => void handlePlaybackStart()}
                />
              ) : (
                <div className={styles.fallbackSurface}>
                  <Text weight="semibold" size={700}>
                    {error ? "播放信息获取失败" : playback?.note ?? "当前没有可播放资源"}
                  </Text>
                  <Text>{error ?? episode?.availabilityNote ?? "资源准备完成后会在这里直接播放。"}</Text>
                </div>
              )}
            </div>
          </div>
        </Card>

        <aside className={`${styles.sidebar} app-motion-surface`} style={{ ["--motion-delay" as string]: "88ms" }}>
          <div className={styles.sidebarHeader}>
            <Text weight="semibold" size={700}>
              剧集
            </Text>
            <Text className={styles.muted}>{visibleEpisodes.length} 集可播放</Text>
          </div>

          {visibleEpisodes.length > 0 ? (
            <div className={styles.episodeList}>
              {visibleEpisodes.map((item, index) => {
                const isCurrentEpisode = item.bangumiEpisodeId === Number(episodeId);

                return (
                  <Link
                    key={item.bangumiEpisodeId}
                    to={`/watch/${subjectId}/${item.bangumiEpisodeId}`}
                    state={watchRouteState}
                    className={styles.episodeLink}
                    style={motionDelayStyle(index, 26, 120)}
                  >
                    <Card
                      className={`${styles.episodeCard} ${isCurrentEpisode ? styles.episodeCardActive : ""}`.trim()}
                    >
                      <div className={styles.episodeTop}>
                        <div className={styles.episodeTitleWrap}>
                          <Text weight="semibold" className={styles.singleLine}>
                            第 {item.episodeNumber ?? item.sort} 集
                          </Text>
                          <Text className={styles.episodeTitle}>{item.titleCn || item.title || "未命名剧集"}</Text>
                        </div>
                        <Badge appearance="filled">可播放</Badge>
                      </div>

                      <div className={styles.episodeMeta}>
                        <Text className={`${styles.singleLine} ${styles.muted}`.trim()}>
                          {item.availabilityNote ?? "已入库，可直接播放"}
                        </Text>
                        {item.airdate ? (
                          <Text className={`${styles.singleLine} ${styles.muted}`.trim()}>{item.airdate}</Text>
                        ) : null}
                      </div>
                    </Card>
                  </Link>
                );
              })}
            </div>
          ) : (
            <div className={styles.emptyState}>
              <Text>当前还没有可播放的剧集。</Text>
            </div>
          )}
        </aside>
      </div>
    </MotionPage>
  );
}
