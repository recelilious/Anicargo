import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  Field,
  Input,
  ProgressBar,
  Spinner,
  Text,
  makeStyles,
} from "@fluentui/react-components";
import { Link } from "react-router-dom";

import { fetchActiveDownloads, fetchResources } from "../api";
import { useSession } from "../session";
import type { ActiveDownload, ResourceLibraryItem } from "../types";

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
  headerRow: {
    display: "flex",
    justifyContent: "space-between",
    gap: "16px",
    alignItems: "flex-end",
    flexWrap: "wrap",
  },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px",
  },
  progressGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))",
    gap: "12px",
  },
  progressCard: {
    padding: "16px 18px",
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  progressHeader: {
    display: "flex",
    justifyContent: "space-between",
    gap: "12px",
    alignItems: "flex-start",
  },
  progressMeta: {
    display: "grid",
    gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
    gap: "8px",
  },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "12px",
  },
  itemCard: {
    padding: "16px 18px",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  link: {
    color: "inherit",
    textDecorationLine: "none",
  },
  muted: {
    color: "var(--app-muted)",
  },
  actions: {
    display: "flex",
    justifyContent: "center",
  },
});

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

function formatProgress(download: ActiveDownload) {
  const total = Math.max(download.totalBytes, download.downloadedBytes);
  if (!total) {
    return "0 B";
  }

  const progress = ((download.downloadedBytes / total) * 100).toFixed(1);
  return `${formatBytes(download.downloadedBytes)} / ${formatBytes(total)} (${progress}%)`;
}

function formatDownloadState(state: string) {
  switch (state) {
    case "queued":
      return "已进入队列";
    case "starting":
      return "启动中";
    case "downloading":
      return "下载中";
    case "seeding":
      return "已完成";
    case "searching":
      return "搜索中";
    default:
      return state;
  }
}

function describeEpisode(item: ResourceLibraryItem) {
  if (item.episodeIndex == null) {
    return item.isCollection ? "合集" : "未映射";
  }

  if (item.episodeEndIndex != null && item.episodeEndIndex !== item.episodeIndex) {
    return `${item.episodeIndex} - ${item.episodeEndIndex}`;
  }

  return `第 ${item.episodeIndex} 集`;
}

export function ResourcesPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [items, setItems] = useState<ResourceLibraryItem[]>([]);
  const [downloads, setDownloads] = useState<ActiveDownload[]>([]);
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
    if (keyword.trim()) {
      params.set("keyword", keyword.trim());
    }

    void Promise.all([
      fetchResources(params, deviceId, userToken),
      fetchActiveDownloads(deviceId, userToken),
    ])
      .then(([resourceResponse, downloadResponse]) => {
        if (!isMounted) {
          return;
        }

        setItems(resourceResponse.items);
        setTotal(resourceResponse.total);
        setPage(resourceResponse.page);
        setHasNextPage(resourceResponse.hasNextPage);
        setDownloads(downloadResponse.items);
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
  }, [deviceId, keyword, userToken]);

  useEffect(() => {
    let isMounted = true;
    const interval = window.setInterval(() => {
      void fetchActiveDownloads(deviceId, userToken).then((response) => {
        if (isMounted) {
          setDownloads(response.items);
        }
      });
    }, 5000);

    return () => {
      isMounted = false;
      window.clearInterval(interval);
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
      if (keyword.trim()) {
        params.set("keyword", keyword.trim());
      }

      const response = await fetchResources(params, deviceId, userToken);
      setItems((current) => [...current, ...response.items]);
      setPage(response.page);
      setHasNextPage(response.hasNextPage);
      setTotal(response.total);
    } finally {
      setIsLoadingMore(false);
    }
  }

  const playableCount = useMemo(() => items.filter((item) => item.status === "ready").length, [items]);

  return (
    <section className={styles.page}>
      <Card className={styles.surfaceCard}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            资源
          </Text>

          <div style={{ minWidth: "min(360px, 100%)" }}>
            <Field label="搜索">
              <Input
                value={keywordInput}
                onChange={(_, data) => setKeywordInput(data.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    setKeyword(keywordInput);
                  }
                }}
                contentAfter={
                  <Button appearance="subtle" onClick={() => setKeyword(keywordInput)}>
                    应用
                  </Button>
                }
                placeholder="番名 / Bangumi ID / 文件名"
              />
            </Field>
          </div>
        </div>
      </Card>

      <div className={styles.stats}>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">进行中的下载</Text>
          <Text>{downloads.length}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">资源总数</Text>
          <Text>{total}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前可播</Text>
          <Text>{playableCount}</Text>
        </Card>
      </div>

      {downloads.length > 0 ? (
        <div className={styles.progressGrid}>
          {downloads.map((download) => {
            const total = Math.max(download.totalBytes, download.downloadedBytes);
            const progressValue = total > 0 ? download.downloadedBytes / total : 0;

            return (
              <Link key={`${download.bangumiSubjectId}-${download.slotKey}`} className={styles.link} to={`/title/${download.bangumiSubjectId}`}>
                <Card className={styles.progressCard}>
                  <div className={styles.progressHeader}>
                    <div>
                      <Text weight="semibold">{download.titleCn || download.title}</Text>
                      <Text className={styles.muted}>{download.sourceFansubName ?? download.sourceTitle}</Text>
                    </div>
                    <Text className={styles.muted}>{formatDownloadState(download.state)}</Text>
                  </div>

                  <ProgressBar value={Math.max(0, Math.min(1, progressValue))} />
                  <Text>{formatProgress(download)}</Text>

                  <div className={styles.progressMeta}>
                    <div>
                      <Text size={200} className={styles.muted}>
                        速度
                      </Text>
                      <Text weight="semibold">{formatSpeed(download.downloadRateBytes)}</Text>
                    </div>
                    <div>
                      <Text size={200} className={styles.muted}>
                        Peer
                      </Text>
                      <Text weight="semibold">{download.peerCount}</Text>
                    </div>
                    <div>
                      <Text size={200} className={styles.muted}>
                        片段
                      </Text>
                      <Text weight="semibold">{download.episodeIndex ?? "?"}</Text>
                    </div>
                  </div>
                </Card>
              </Link>
            );
          })}
        </div>
      ) : null}

      {isLoading ? <Spinner label="正在读取资源..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有命中的资源</Text>
        </Card>
      ) : null}

      <div className={styles.list}>
        {items.map((item) => (
          <Link key={item.id} className={styles.link} to={`/title/${item.bangumiSubjectId}`}>
            <Card className={styles.itemCard}>
              <Text weight="semibold">{item.fileName}</Text>
              <Text className={styles.muted}>Bangumi {item.bangumiSubjectId}</Text>
              <Text>{describeEpisode(item)}</Text>
              <Text>{formatBytes(item.sizeBytes)}</Text>
              <Text className={styles.muted}>{item.sourceFansubName ?? item.sourceTitle}</Text>
            </Card>
          </Link>
        ))}
      </div>

      {hasNextPage ? (
        <div className={styles.actions}>
          <Button appearance="primary" onClick={() => void loadMore()} disabled={isLoadingMore}>
            {isLoadingMore ? "正在加载..." : "加载更多"}
          </Button>
        </div>
      ) : null}
    </section>
  );
}
