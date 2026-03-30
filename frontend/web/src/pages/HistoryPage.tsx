import { useEffect, useState } from "react";
import { Card, Text, makeStyles } from "@fluentui/react-components";
import { Link, useLocation } from "react-router-dom";

import { fetchPlaybackHistory } from "../api";
import { CardCoverFallback } from "../components/CardCoverFallback";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence, motionDelayStyle } from "../motion";
import { buildRoutePath, rememberReturnTarget, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { PlaybackHistoryItem } from "../types";

const PAGE_SIZE = 24;

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
  },
  surfaceCard: {
    padding: "20px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  headerRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: "16px",
    flexWrap: "wrap",
  },
  statBox: {
    padding: "12px 14px",
    minWidth: "132px",
    borderRadius: "18px",
    border: "1px solid var(--app-border)",
    backgroundColor: "var(--app-surface-2)",
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  historyPanel: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "18px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    flex: "1 1 auto",
    minHeight: 0,
    overflow: "hidden",
  },
  listViewport: {
    flex: "1 1 auto",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    paddingRight: "6px",
  },
  list: {
    display: "grid",
    gridTemplateColumns: "1fr",
    gridAutoRows: "148px",
    gap: "12px",
  },
  link: {
    color: "inherit",
    textDecorationLine: "none",
  },
  historyCard: {
    height: "100%",
    display: "grid",
    gridTemplateColumns: "92px minmax(0, 1fr)",
    gap: "14px",
    padding: "14px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  poster: {
    position: "relative",
    width: "92px",
    height: "100%",
    borderRadius: "16px",
    backgroundColor: "var(--app-fallback-hero)",
    backgroundSize: "cover",
    backgroundPosition: "center center",
    overflow: "hidden",
  },
  body: {
    display: "flex",
    flexDirection: "column",
    justifyContent: "space-between",
    gap: "10px",
    minWidth: 0,
  },
  content: {
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
  singleLine: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  muted: {
    color: "var(--app-muted)",
  },
  metaRow: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: "10px",
    alignItems: "end",
  },
  metaCell: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minWidth: 0,
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

function HistoryPoster({ imagePortrait }: { imagePortrait: string | null }) {
  const styles = useStyles();
  const [hasPosterError, setHasPosterError] = useState(false);
  const shouldShowPoster = Boolean(imagePortrait) && !hasPosterError;

  useEffect(() => {
    setHasPosterError(false);
  }, [imagePortrait]);

  return (
    <div className={styles.poster}>
      {!shouldShowPoster ? <CardCoverFallback logoWidth="54%" logoMaxWidth={60} /> : null}
      {shouldShowPoster ? (
        <img
          src={imagePortrait ?? undefined}
          alt=""
          loading="lazy"
          style={{
            width: "100%",
            height: "100%",
            objectFit: "cover",
            display: "block",
          }}
          onError={() => setHasPosterError(true)}
        />
      ) : null}
    </div>
  );
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
  useLoadingStatus(
    isLoading ? "正在读取历史记录..." : isLoadingMore ? "正在加载更多历史记录..." : null,
  );

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

  const routeState: RouteState = {
    fromPath: buildRoutePath(location),
  };

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.surfaceCard} app-motion-surface`}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            历史记录
          </Text>

          <div className={styles.statBox}>
            <Text size={200} className={styles.muted}>
              已记录
            </Text>
            <Text weight="semibold">{total}</Text>
          </div>
        </div>
      </Card>
      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有播放历史</Text>
        </Card>
      ) : null}

      <section className={`${styles.historyPanel} app-motion-surface`} style={{ ["--motion-delay" as string]: "48ms" }}>
        <div className={styles.listViewport}>
          <div className={styles.list}>
            {items.map((item, index) => (
              <Link
                key={`${item.bangumiSubjectId}-${item.bangumiEpisodeId}-${item.lastPlayedAt}`}
                className={styles.link}
                to={`/watch/${item.bangumiSubjectId}/${item.bangumiEpisodeId}`}
                state={routeState}
                onClick={rememberCurrentPosition}
                style={motionDelayStyle(index, 30, 90)}
              >
                <Card className={styles.historyCard}>
                  <HistoryPoster imagePortrait={item.imagePortrait} />

                  <div className={styles.body}>
                    <div className={styles.content}>
                      <Text weight="semibold" className={styles.title}>
                        {item.subjectTitleCn || item.subjectTitle}
                      </Text>
                      <Text className={styles.title}>
                        第 {item.episodeNumber ?? "?"} 集 · {item.episodeTitleCn || item.episodeTitle || "未命名"}
                      </Text>
                    </div>

                    <div className={styles.metaRow}>
                      <div className={styles.metaCell}>
                        <Text size={200} className={styles.muted}>
                          最近播放
                        </Text>
                        <Text className={styles.singleLine}>{formatPlayedAt(item.lastPlayedAt)}</Text>
                      </div>
                      <div className={styles.metaCell}>
                        <Text size={200} className={styles.muted}>
                          播放次数
                        </Text>
                        <Text className={styles.singleLine}>{item.playCount} 次</Text>
                      </div>
                    </div>
                  </div>
                </Card>
              </Link>
            ))}
          </div>
        </div>

        {hasNextPage ? (
          <div className={styles.actions}>
            <Card className={styles.loadMoreCard} onClick={() => void loadMore()}>
              <Text weight="semibold">加载更多</Text>
            </Card>
          </div>
        ) : null}
      </section>
    </MotionPage>
  );
}
