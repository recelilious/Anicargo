import { useEffect, useState } from "react";
import { Badge, Button, Card, Spinner, Text, makeStyles } from "@fluentui/react-components";
import { useParams } from "react-router-dom";

import { fetchSubjectDetail, toggleSubscription } from "../api";
import { EpisodeCard } from "../components/EpisodeCard";
import { useSession } from "../session";
import type { SubjectDetailResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px"
  },
  hero: {
    position: "relative",
    overflow: "hidden",
    minHeight: "280px",
    padding: "28px",
    display: "grid",
    alignContent: "end",
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
    backgroundColor: "rgba(18, 10, 8, 0.64)"
  },
  heroContent: {
    position: "relative",
    display: "flex",
    flexDirection: "column",
    gap: "14px"
  },
  badges: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap"
  },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px"
  },
  infoGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  },
  episodes: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
    gap: "12px"
  },
  surfaceCard: {
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  }
});

function formatRating(score: number | null) {
  return score == null ? "暂无评分" : score.toFixed(1);
}

export function SubjectPage() {
  const styles = useStyles();
  const { subjectId } = useParams();
  const { deviceId, userToken } = useSession();
  const [detail, setDetail] = useState<SubjectDetailResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSubscribing, setIsSubscribing] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
  }, [subjectId, deviceId, userToken]);

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
    } finally {
      setIsSubscribing(false);
    }
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
            backgroundImage: detail.subject.imageBanner ? `url(${detail.subject.imageBanner})` : undefined,
            backgroundColor: "var(--app-fallback-hero)"
          }}
        />
        <div className={styles.heroOverlay} />

        <div className={styles.heroContent}>
          <Text weight="semibold" size={900}>
            {detail.subject.titleCn || detail.subject.title}
          </Text>
          <Text>{detail.subject.title}</Text>
          <div className={styles.badges}>
            {detail.subject.tags.map((tag) => (
              <Badge key={tag} appearance="filled">
                {tag}
              </Badge>
            ))}
          </div>
          <Button appearance="primary" onClick={handleToggleSubscription} disabled={isSubscribing}>
            {detail.subscription.isSubscribed ? "取消订阅" : "订阅"}
          </Button>
        </div>
      </Card>

      <div className={styles.stats}>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">订阅</Text>
          <Text>
            {detail.subscription.subscriptionCount} / {detail.subscription.threshold}
          </Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">归属</Text>
          <Text>{detail.subscription.source.kind === "user" ? "账号订阅" : "设备订阅"}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">放送</Text>
          <Text>{detail.subject.airDate ?? "未知"}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">评分</Text>
          <Text>{formatRating(detail.subject.ratingScore)}</Text>
        </Card>
      </div>

      {detail.subject.summary ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold" size={700}>
            简介
          </Text>
          <Text>{detail.subject.summary}</Text>
        </Card>
      ) : null}

      {detail.subject.infobox.length > 0 ? (
        <div className={styles.infoGrid}>
          {detail.subject.infobox.map((item) => (
            <Card key={`${item.key}-${item.value}`} className={styles.surfaceCard}>
              <Text weight="semibold">{item.key}</Text>
              <Text>{item.value}</Text>
            </Card>
          ))}
        </div>
      ) : null}

      <div className={styles.episodes}>
        {detail.episodes.map((episode) => (
          <EpisodeCard key={episode.bangumiEpisodeId} episode={episode} subjectId={detail.subject.bangumiSubjectId} />
        ))}
      </div>
    </section>
  );
}
