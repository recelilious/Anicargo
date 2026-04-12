import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowLeftRegular } from "@fluentui/react-icons";
import { Button, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";

import {
  buildApiUrl,
  recordPlaybackHistory,
} from "../api";
import { AnicargoPlayer } from "../components/AnicargoPlayer";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, motionDelayStyle } from "../motion";
import { resolveReturnScrollTop, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { Episode, EpisodePlaybackResponse, SubjectDetailResponse } from "../types";
import {
  fetchEpisodePlaybackCached,
  fetchSubjectDetailCached,
  getCachedEpisodePlayback,
  getCachedSubjectDetail,
  revalidateEpisodePlayback,
  revalidateSubjectDetail,
} from "../view-cache";

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
    padding: "0",
    display: "flex",
    alignItems: "stretch",
    justifyContent: "stretch",
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
    alignItems: "stretch",
    justifyContent: "stretch",
  },
  playerFrame: {
    width: "100%",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: "#070a10",
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
    alignItems: "center",
  },
  episodeList: {
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    display: "grid",
    gridAutoRows: "88px",
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
    justifyContent: "center",
    gap: "6px",
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
  episodeTitleWrap: {
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    gap: "6px",
  },
  singleLine: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  episodeTitle: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    lineHeight: "1.45",
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
  const numericSubjectId = subjectId ? Number(subjectId) : null;
  const numericEpisodeId = episodeId ? Number(episodeId) : null;
  const cachedDetail =
    numericSubjectId != null ? getCachedSubjectDetail(numericSubjectId, deviceId, userToken) : null;
  const cachedPlayback =
    numericSubjectId != null && numericEpisodeId != null
      ? getCachedEpisodePlayback(numericSubjectId, numericEpisodeId, deviceId, userToken)
      : null;
  const hasRecordedPlaybackRef = useRef(false);
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(cachedDetail);
  const [episode, setEpisode] = useState<Episode | null>(
    cachedDetail?.episodes.find((item) => item.bangumiEpisodeId === numericEpisodeId) ?? null,
  );
  const [playback, setPlayback] = useState<EpisodePlaybackResponse | null>(cachedPlayback);
  const [isLoading, setIsLoading] = useState(cachedDetail == null || cachedPlayback == null);
  const [error, setError] = useState<string | null>(null);
  useLoadingStatus(isLoading ? "正在准备播放..." : null);
  const routeState = (location.state as RouteState | null) ?? null;

  useEffect(() => {
    if (numericSubjectId == null || numericEpisodeId == null) {
      return;
    }

    let cancelled = false;
    const nextCachedDetail = getCachedSubjectDetail(numericSubjectId, deviceId, userToken);
    const nextCachedPlayback = getCachedEpisodePlayback(
      numericSubjectId,
      numericEpisodeId,
      deviceId,
      userToken,
    );

    setDetail(nextCachedDetail);
    setEpisode(
      nextCachedDetail?.episodes.find((item) => item.bangumiEpisodeId === numericEpisodeId) ?? null,
    );
    setPlayback(nextCachedPlayback);
    setIsLoading(nextCachedDetail == null || nextCachedPlayback == null);
    setError(null);
    hasRecordedPlaybackRef.current = false;

    const detailRequest = nextCachedDetail
      ? revalidateSubjectDetail(numericSubjectId, deviceId, userToken)
      : fetchSubjectDetailCached(numericSubjectId, deviceId, userToken);
    const playbackRequest = nextCachedPlayback
      ? revalidateEpisodePlayback(numericSubjectId, numericEpisodeId, deviceId, userToken)
      : fetchEpisodePlaybackCached(numericSubjectId, numericEpisodeId, deviceId, userToken);

    void Promise.all([detailRequest, playbackRequest])
      .then(([detailResponse, playbackResponse]) => {
        if (cancelled) {
          return;
        }

        setDetail(detailResponse);
        setEpisode(
          detailResponse.episodes.find((item) => item.bangumiEpisodeId === numericEpisodeId) ?? null,
        );
        setPlayback(playbackResponse);
      })
      .catch((requestError: Error) => {
        if (cancelled) {
          return;
        }

        setError(requestError.message);
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [numericSubjectId, numericEpisodeId, deviceId, userToken]);

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
  const subtitleTracks = useMemo(
    () =>
      playback?.media?.subtitleTracks.map((track) => ({
        id: track.id,
        label: track.label,
        language: track.language,
        url: buildApiUrl(track.url),
      })) ?? [],
    [playback?.media?.subtitleTracks],
  );
  const pageTitle = detail?.subject.titleCn || detail?.subject.title || "播放";
  const episodeLabel = formatEpisodeLabel(episode);
  const episodeTitle = episode?.titleCn || episode?.title || "未命名剧集";
  const fansubLabel = playback?.media?.sourceFansubName ?? "来源未知";
  const updatedLabel = formatUpdatedAt(playback?.media?.updatedAt);

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
                  subtitleTracks={subtitleTracks}
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
                      <div className={styles.episodeTitleWrap}>
                        <Text weight="semibold" className={styles.singleLine}>
                          第 {item.episodeNumber ?? item.sort} 集
                        </Text>
                        <Text className={styles.episodeTitle}>{item.titleCn || item.title || "未命名剧集"}</Text>
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
