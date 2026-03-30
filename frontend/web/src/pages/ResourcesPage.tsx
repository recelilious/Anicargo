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
const ACTIVE_DOWNLOAD_REFRESH_MS = 1000;
const RESOURCE_REFRESH_MS = 1000;

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
    gap: "16px",
    alignItems: "flex-start",
    flexWrap: "wrap",
  },
  contentGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(320px, 1fr))",
    gap: "16px",
    flex: "1 1 auto",
    minHeight: 0,
    overflow: "hidden",
    alignItems: "stretch",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "18px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflow: "hidden",
  },
  panelHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "flex-start",
    gap: "12px",
    flexWrap: "wrap",
  },
  panelStats: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(120px, 1fr))",
    gap: "10px",
    width: "min(320px, 100%)",
  },
  statBox: {
    padding: "12px 14px",
    borderRadius: "18px",
    border: "1px solid var(--app-border)",
    backgroundColor: "var(--app-surface-2)",
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minWidth: 0,
  },
  listViewport: {
    flex: "1 1 auto",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    paddingRight: "6px",
  },
  progressGrid: {
    display: "grid",
    gridTemplateColumns: "1fr",
    gridAutoRows: "156px",
    gap: "12px",
  },
  progressCard: {
    height: "100%",
    padding: "16px 18px",
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    minWidth: 0,
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
  titleBlock: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minWidth: 0,
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
  progressText: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  list: {
    display: "grid",
    gridTemplateColumns: "1fr",
    gridAutoRows: "156px",
    gap: "12px",
  },
  itemCard: {
    height: "100%",
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

function formatResourceStatus(value: string) {
  switch (value) {
    case "ready":
      return "已入库";
    case "downloaded":
      return "已下载";
    case "partial":
      return "未完成";
    default:
      return value;
  }
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
      return "已排队";
    case "starting":
      return "启动中";
    case "downloading":
      return "下载中";
    case "seeding":
      return "已完成";
    case "searching":
      return "搜索中";
    case "staged":
      return "待启动";
    default:
      return state;
  }
}

function formatEpisodeNumber(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function describeDownloadSlot(download: ActiveDownload) {
  if (download.isCollection) {
    if (download.episodeIndex != null && download.episodeEndIndex != null) {
      return `合集 ${formatEpisodeNumber(download.episodeIndex)} - ${formatEpisodeNumber(download.episodeEndIndex)}`;
    }

    return "合集";
  }

  if (download.episodeIndex == null) {
    return download.slotKey;
  }

  if (
    download.episodeEndIndex != null &&
    download.episodeEndIndex !== download.episodeIndex
  ) {
    return `第 ${formatEpisodeNumber(download.episodeIndex)} - ${formatEpisodeNumber(download.episodeEndIndex)} 集`;
  }

  return `第 ${formatEpisodeNumber(download.episodeIndex)} 集`;
}

function describeEpisode(item: ResourceLibraryItem) {
  if (item.episodeIndex == null) {
    return item.isCollection ? "合集" : "未映射";
  }

  if (item.episodeEndIndex != null && item.episodeEndIndex !== item.episodeIndex) {
    return `${formatEpisodeNumber(item.episodeIndex)} - ${formatEpisodeNumber(item.episodeEndIndex)}`;
  }

  return `第 ${formatEpisodeNumber(item.episodeIndex)} 集`;
}

function compareDisplayText(left: string, right: string) {
  return left.localeCompare(right, ["zh-Hans-CN", "ja-JP", "en-US"], {
    sensitivity: "base",
    numeric: true,
  });
}

export function ResourcesPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [items, setItems] = useState<ResourceLibraryItem[]>([]);
  const [downloads, setDownloads] = useState<ActiveDownload[]>([]);
  const [total, setTotal] = useState(0);
  const [totalSizeBytes, setTotalSizeBytes] = useState(0);
  const [page, setPage] = useState(1);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const visibleResourcePageSize = Math.min(page * PAGE_SIZE, 96);

  function buildResourceParams(pageSize = visibleResourcePageSize) {
    const params = new URLSearchParams({
      page: "1",
      pageSize: String(pageSize),
    });
    if (keyword.trim()) {
      params.set("keyword", keyword.trim());
    }
    return params;
  }

  useEffect(() => {
    let isMounted = true;
    setIsLoading(true);

    void Promise.all([
      fetchResources(buildResourceParams(), deviceId, userToken),
      fetchActiveDownloads(deviceId, userToken),
    ])
      .then(([resourceResponse, downloadResponse]) => {
        if (!isMounted) {
          return;
        }

        setItems(resourceResponse.items);
        setTotal(resourceResponse.total);
        setTotalSizeBytes(resourceResponse.totalSizeBytes);
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
  }, [deviceId, keyword, userToken, visibleResourcePageSize]);

  useEffect(() => {
    let isMounted = true;
    let timeoutId: number | undefined;

    const poll = async () => {
      try {
        const [downloadResponse, resourceResponse] = await Promise.all([
          fetchActiveDownloads(deviceId, userToken),
          fetchResources(buildResourceParams(), deviceId, userToken),
        ]);
        if (isMounted) {
          setDownloads(downloadResponse.items);
          setItems(resourceResponse.items);
          setTotal(resourceResponse.total);
          setTotalSizeBytes(resourceResponse.totalSizeBytes);
          setHasNextPage(resourceResponse.hasNextPage);
          setError(null);
        }
      } catch (nextError) {
        if (isMounted && nextError instanceof Error) {
          setError(nextError.message);
        }
      } finally {
        if (isMounted) {
          timeoutId = window.setTimeout(
            poll,
            Math.min(ACTIVE_DOWNLOAD_REFRESH_MS, RESOURCE_REFRESH_MS),
          );
        }
      }
    };

    timeoutId = window.setTimeout(
      poll,
      Math.min(ACTIVE_DOWNLOAD_REFRESH_MS, RESOURCE_REFRESH_MS),
    );

    return () => {
      isMounted = false;
      if (timeoutId !== undefined) {
        window.clearTimeout(timeoutId);
      }
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
      setTotalSizeBytes(response.totalSizeBytes);
    } finally {
      setIsLoadingMore(false);
    }
  }

  const sortedDownloads = useMemo(() => {
    const visibleDownloads = downloads.filter((item) =>
      item.state === "staged" ||
      item.state === "queued" ||
      item.state === "searching" ||
      item.state === "starting" ||
      item.state === "downloading"
    );

    return [...visibleDownloads].sort((left, right) => {
      const titleCompare = compareDisplayText(
        left.titleCn || left.title,
        right.titleCn || right.title,
      );
      if (titleCompare !== 0) {
        return titleCompare;
      }

      const leftEpisode = left.episodeIndex ?? Number.POSITIVE_INFINITY;
      const rightEpisode = right.episodeIndex ?? Number.POSITIVE_INFINITY;
      if (leftEpisode !== rightEpisode) {
        return leftEpisode - rightEpisode;
      }

      const fansubCompare = compareDisplayText(
        left.sourceFansubName ?? "",
        right.sourceFansubName ?? "",
      );
      if (fansubCompare !== 0) {
        return fansubCompare;
      }

      return new Date(left.updatedAt).getTime() - new Date(right.updatedAt).getTime();
    });
  }, [downloads]);

  const activeDownloadCount = useMemo(
    () => sortedDownloads.filter((item) => item.state === "downloading" || item.state === "starting").length,
    [sortedDownloads],
  );
  const totalDownloadSpeed = useMemo(
    () => sortedDownloads.reduce((sum, item) => sum + item.downloadRateBytes, 0),
    [sortedDownloads],
  );

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

      {isLoading ? <Spinner label="正在读取资源..." /> : null}
      {error ? <Text>{error}</Text> : null}

      <div className={styles.contentGrid}>
        <section className={styles.panel}>
          <div className={styles.panelHeader}>
            <Text weight="semibold" size={700}>
              下载进度
            </Text>

            <div className={styles.panelStats}>
              <div className={styles.statBox}>
                <Text size={200} className={styles.muted}>
                  下载中
                </Text>
                <Text weight="semibold">{activeDownloadCount}</Text>
              </div>
              <div className={styles.statBox}>
                <Text size={200} className={styles.muted}>
                  下载速度
                </Text>
                <Text weight="semibold">{formatSpeed(totalDownloadSpeed)}</Text>
              </div>
            </div>
          </div>

          <div className={styles.listViewport}>
            <div className={styles.progressGrid}>
              {sortedDownloads.map((download) => {
                const total = Math.max(download.totalBytes, download.downloadedBytes);
                const progressValue = total > 0 ? download.downloadedBytes / total : 0;

                return (
                  <Link
                    key={`${download.bangumiSubjectId}-${download.slotKey}`}
                    className={styles.link}
                    to={`/title/${download.bangumiSubjectId}`}
                  >
                    <Card className={styles.progressCard}>
                      <div className={styles.progressHeader}>
                        <div className={styles.titleBlock}>
                          <Text weight="semibold" className={styles.titleLine}>
                            {describeDownloadSlot(download)}
                          </Text>
                          <Text className={styles.subtitleLine}>
                            {download.titleCn || download.title}
                          </Text>
                        </div>
                        <Text className={styles.muted}>{formatDownloadState(download.state)}</Text>
                      </div>

                      <ProgressBar value={Math.max(0, Math.min(1, progressValue))} />
                      <Text className={styles.progressText}>{formatProgress(download)}</Text>

                      <div className={styles.progressMeta}>
                        <div>
                          <Text size={200} className={styles.muted}>
                            速度
                          </Text>
                          <Text weight="semibold" className={styles.progressText}>
                            {formatSpeed(download.downloadRateBytes)}
                          </Text>
                        </div>
                        <div>
                          <Text size={200} className={styles.muted}>
                            Peer
                          </Text>
                          <Text weight="semibold">{download.peerCount}</Text>
                        </div>
                        <div>
                          <Text size={200} className={styles.muted}>
                            来源
                          </Text>
                          <Text weight="semibold" className={styles.progressText}>
                            {download.sourceFansubName ?? "AnimeGarden"}
                          </Text>
                        </div>
                      </div>
                    </Card>
                  </Link>
                );
              })}
            </div>
          </div>
        </section>

        <section className={styles.panel}>
          <div className={styles.panelHeader}>
            <Text weight="semibold" size={700}>
              已下载资源
            </Text>

            <div className={styles.panelStats}>
              <div className={styles.statBox}>
                <Text size={200} className={styles.muted}>
                  资源总数
                </Text>
                <Text weight="semibold">{total}</Text>
              </div>
              <div className={styles.statBox}>
                <Text size={200} className={styles.muted}>
                  占用空间
                </Text>
                <Text weight="semibold">{formatBytes(totalSizeBytes)}</Text>
              </div>
            </div>
          </div>

          <div className={styles.listViewport}>
            <div className={styles.list}>
              {items.map((item) => (
                <Link key={item.id} className={styles.link} to={`/title/${item.bangumiSubjectId}`}>
                  <Card className={styles.itemCard}>
                    <Text weight="semibold" className={styles.titleLine}>
                      {item.fileName}
                    </Text>
                    <Text className={styles.subtitleLine}>{item.sourceTitle}</Text>
                    <Text className={styles.titleLine}>
                      {formatResourceStatus(item.status)} · {describeEpisode(item)}
                    </Text>
                    <Text className={styles.titleLine}>{formatBytes(item.sizeBytes)}</Text>
                    <Text className={styles.subtitleLine}>
                      {(item.sourceFansubName ?? "AnimeGarden")} · Bangumi {item.bangumiSubjectId}
                    </Text>
                  </Card>
                </Link>
              ))}
            </div>
          </div>

          {hasNextPage ? (
            <div className={styles.actions}>
              <Button appearance="primary" onClick={() => void loadMore()} disabled={isLoadingMore}>
                {isLoadingMore ? "正在加载..." : "加载更多"}
              </Button>
            </div>
          ) : null}
        </section>
      </div>
    </section>
  );
}
