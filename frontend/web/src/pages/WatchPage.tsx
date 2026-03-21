import { useEffect, useState } from "react";
import { Card, Spinner, Text, makeStyles } from "@fluentui/react-components";
import { useParams } from "react-router-dom";

import { fetchSubjectDetail } from "../api";
import { useSession } from "../session";
import type { Episode, SubjectDetailResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  player: {
    minHeight: "360px",
    display: "grid",
    placeItems: "center",
    backgroundColor: "var(--app-surface-1)",
    color: "var(--app-text)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  }
});

export function WatchPage() {
  const styles = useStyles();
  const { subjectId, episodeId } = useParams();
  const { deviceId, userToken } = useSession();
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [episode, setEpisode] = useState<Episode | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    if (!subjectId || !episodeId) {
      return;
    }

    let isMounted = true;
    setIsLoading(true);

    void fetchSubjectDetail(Number(subjectId), deviceId, userToken)
      .then((response) => {
        if (!isMounted) {
          return;
        }

        setDetail(response);
        setEpisode(response.episodes.find((item) => item.bangumiEpisodeId === Number(episodeId)) ?? null);
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

  if (isLoading) {
    return <Spinner label="正在准备播放..." />;
  }

  return (
    <section className={styles.page}>
      <Card className={styles.player}>
        <Text weight="semibold" size={800}>
          {episode?.isAvailable ? "播放器占位" : "资源未入库"}
        </Text>
        <Text>{episode?.availabilityNote ?? "播放链路接入后会直接进入按集播放。"}</Text>
      </Card>

      <Card>
        <Text weight="semibold">{detail?.subject.titleCn || detail?.subject.title}</Text>
        <Text>
          第 {episode?.episodeNumber ?? episode?.sort ?? "?"} 集 · {episode?.titleCn || episode?.title || "未命名剧集"}
        </Text>
      </Card>
    </section>
  );
}
