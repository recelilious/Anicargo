import { useEffect, useRef, useState } from "react";
import { Card, Spinner, Text, makeStyles } from "@fluentui/react-components";
import { useParams } from "react-router-dom";

import {
  buildApiUrl,
  fetchEpisodePlayback,
  fetchSubjectDetail,
  recordPlaybackHistory
} from "../api";
import { useSession } from "../session";
import type { Episode, EpisodePlaybackResponse, SubjectDetailResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  playerCard: {
    padding: "16px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  playerSurface: {
    minHeight: "420px",
    display: "grid",
    placeItems: "center",
    borderRadius: "18px",
    overflow: "hidden",
    backgroundColor: "rgba(6, 8, 12, 0.92)"
  },
  video: {
    width: "100%",
    height: "100%",
    display: "block",
    backgroundColor: "rgba(6, 8, 12, 0.96)"
  },
  playerFallback: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    alignItems: "center",
    justifyContent: "center",
    padding: "32px 20px",
    textAlign: "center"
  },
  infoCard: {
    padding: "18px 20px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  metaGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px",
    marginTop: "14px"
  },
  metaItem: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    padding: "14px 16px",
    borderRadius: "16px",
    backgroundColor: "var(--app-surface-2)",
    border: "1px solid var(--app-border)"
  },
  muted: {
    color: "var(--app-muted)"
  }
});

export function WatchPage() {
  const styles = useStyles();
  const { subjectId, episodeId } = useParams();
  const { deviceId, userToken } = useSession();
  const hasRecordedPlaybackRef = useRef(false);
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [episode, setEpisode] = useState<Episode | null>(null);
  const [playback, setPlayback] = useState<EpisodePlaybackResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
      fetchEpisodePlayback(numericSubjectId, numericEpisodeId, deviceId, userToken)
    ])
      .then(([detailResponse, playbackResponse]) => {
        if (!isMounted) {
          return;
        }

        setDetail(detailResponse);
        setEpisode(
          detailResponse.episodes.find((item) => item.bangumiEpisodeId === numericEpisodeId) ?? null
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
          mediaInventoryId: playback.media.mediaInventoryId
        },
        deviceId,
        userToken
      );
    } catch (recordError) {
      hasRecordedPlaybackRef.current = false;
      console.warn("Failed to record playback history", recordError);
    }
  }

  if (isLoading) {
    return <Spinner label="正在准备播放..." />;
  }

  const streamUrl = playback?.media ? buildApiUrl(playback.media.streamUrl) : null;

  return (
    <section className={styles.page}>
      <Card className={styles.playerCard}>
        <div className={styles.playerSurface}>
          {streamUrl ? (
            <video
              className={styles.video}
              controls
              onPlay={() => void handlePlaybackStart()}
              preload="metadata"
              src={streamUrl}
              playsInline
            />
          ) : (
            <div className={styles.playerFallback}>
              <Text weight="semibold" size={700}>
                {error ? "播放信息获取失败" : playback?.note ?? "当前没有可播放资源"}
              </Text>
              <Text className={styles.muted}>
                {error ?? episode?.availabilityNote ?? "资源准备完成后会在这里直接播放。"}
              </Text>
            </div>
          )}
        </div>
      </Card>

      <Card className={styles.infoCard}>
        <Text weight="semibold" size={700}>
          {detail?.subject.titleCn || detail?.subject.title || "剧集播放"}
        </Text>
        <Text>
          第 {episode?.episodeNumber ?? episode?.sort ?? "?"} 集 ·{" "}
          {episode?.titleCn || episode?.title || "未命名剧集"}
        </Text>

        <div className={styles.metaGrid}>
          <div className={styles.metaItem}>
            <Text size={200} className={styles.muted}>
              播放状态
            </Text>
            <Text weight="semibold">{playback?.note ?? "未获取"}</Text>
          </div>

          <div className={styles.metaItem}>
            <Text size={200} className={styles.muted}>
              文件
            </Text>
            <Text weight="semibold">{playback?.media?.fileName ?? "暂无"}</Text>
          </div>

          <div className={styles.metaItem}>
            <Text size={200} className={styles.muted}>
              来源
            </Text>
            <Text weight="semibold">{playback?.media?.sourceTitle ?? "暂无"}</Text>
          </div>

          <div className={styles.metaItem}>
            <Text size={200} className={styles.muted}>
              字幕组
            </Text>
            <Text weight="semibold">{playback?.media?.sourceFansubName ?? "未标注"}</Text>
          </div>
        </div>
      </Card>
    </section>
  );
}
