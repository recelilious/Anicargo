import { useEffect, useState } from "react";
import { Card, Spinner, Text, makeStyles } from "@fluentui/react-components";
import { Link, useLocation } from "react-router-dom";

import { fetchPlaybackHistory } from "../api";
import { buildRoutePath, rememberReturnTarget } from "../navigation";
import { useSession } from "../session";
import type { PlaybackHistoryItem } from "../types";

const PAGE_SIZE = 24;

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
  },
  surfaceCard: {
    padding: "20px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
    gap: "12px",
  },
  link: {
    color: "inherit",
    textDecorationLine: "none",
  },
  historyCard: {
    display: "grid",
    gridTemplateColumns: "84px minmax(0, 1fr)",
    gap: "14px",
    padding: "14px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  poster: {
    width: "84px",
    height: "116px",
    borderRadius: "16px",
    backgroundColor: "var(--app-fallback-hero)",
    backgroundSize: "cover",
    backgroundPosition: "center center",
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minWidth: 0,
  },
  title: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitLineClamp: "2",
    WebkitBoxOrient: "vertical",
  },
  muted: {
    color: "var(--app-muted)",
  },
  actions: {
    display: "flex",
    justifyContent: "center",
  },
  loadMoreCard: {
    padding: "14px 16px",
    textAlign: "center",
    cursor: "pointer",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
});

function formatPlayedAt(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

export function HistoryPage() {
  const styles = useStyles();
  const location = useLocation();
  const { deviceId, userToken } = useSession();
  const [items, setItems] = useState<PlaybackHistoryItem[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let isMounted = true;
    setIsLoading(true);

    const params = new URLSearchParams({
      page: "1",
      pageSize: String(PAGE_SIZE),
    });

    void fetchPlaybackHistory(params, deviceId, userToken)
      .then((response) => {
        if (!isMounted) {
          return;
        }

        setItems(response.items);
        setTotal(response.total);
        setPage(response.page);
        setHasNextPage(response.hasNextPage);
        setError(null);
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
  }, [deviceId, userToken]);

  async function loadMore() {
    if (isLoadingMore || !hasNextPage) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const params = new URLSearchParams({
        page: String(page + 1),
        pageSize: String(PAGE_SIZE),
      });

      const response = await fetchPlaybackHistory(params, deviceId, userToken);
      setItems((current) => [...current, ...response.items]);
      setTotal(response.total);
      setPage(response.page);
      setHasNextPage(response.hasNextPage);
    } finally {
      setIsLoadingMore(false);
    }
  }

  function rememberCurrentPosition() {
    const scrollTop = document.getElementById("app-scroll-root")?.scrollTop ?? 0;
    rememberReturnTarget(buildRoutePath(location), scrollTop);
  }

  return (
    <section className={styles.page}>
      <Card className={styles.surfaceCard}>
        <Text weight="semibold" size={800}>
          历史记录
        </Text>
      </Card>

      <Card className={styles.surfaceCard}>
        <Text weight="semibold">已记录</Text>
        <Text>{total}</Text>
      </Card>

      {isLoading ? <Spinner label="正在读取历史记录..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有播放历史</Text>
        </Card>
      ) : null}

      <div className={styles.list}>
        {items.map((item) => (
          <Link
            key={`${item.bangumiSubjectId}-${item.bangumiEpisodeId}-${item.lastPlayedAt}`}
            className={styles.link}
            to={`/watch/${item.bangumiSubjectId}/${item.bangumiEpisodeId}`}
            onClick={rememberCurrentPosition}
          >
            <Card className={styles.historyCard}>
              <div
                className={styles.poster}
                style={{
                  backgroundImage: item.imagePortrait ? `url(${item.imagePortrait})` : undefined,
                }}
              />

              <div className={styles.body}>
                <Text weight="semibold" className={styles.title}>
                  {item.subjectTitleCn || item.subjectTitle}
                </Text>
                <Text className={styles.title}>
                  第 {item.episodeNumber ?? "?"} 集 · {item.episodeTitleCn || item.episodeTitle || "未命名"}
                </Text>
                <Text className={styles.muted}>{item.fileName ?? "资源文件待确认"}</Text>
                <Text className={styles.muted}>{item.sourceFansubName ?? "未标注字幕组"}</Text>
                <Text className={styles.muted}>
                  最近播放 {formatPlayedAt(item.lastPlayedAt)} · {item.playCount} 次
                </Text>
              </div>
            </Card>
          </Link>
        ))}
      </div>

      {hasNextPage ? (
        <div className={styles.actions}>
          <Card className={styles.loadMoreCard} onClick={() => void loadMore()}>
            <Text weight="semibold">{isLoadingMore ? "正在加载..." : "加载更多"}</Text>
          </Card>
        </div>
      ) : null}
    </section>
  );
}
