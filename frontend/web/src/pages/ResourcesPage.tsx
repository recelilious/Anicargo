import { useEffect, useMemo, useState } from "react";
import { Button, Card, Field, Input, Spinner, Text, makeStyles } from "@fluentui/react-components";

import { fetchResources } from "../api";
import { useSession } from "../session";
import type { ResourceLibraryItem } from "../types";

const PAGE_SIZE = 24;

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  surfaceCard: {
    padding: "20px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  headerRow: {
    display: "flex",
    justifyContent: "space-between",
    gap: "16px",
    alignItems: "flex-end",
    flexWrap: "wrap"
  },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px"
  },
  list: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))",
    gap: "12px"
  },
  itemCard: {
    padding: "16px 18px",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  muted: {
    color: "var(--app-muted)"
  },
  actions: {
    display: "flex",
    justifyContent: "center"
  }
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
      pageSize: String(PAGE_SIZE)
    });
    if (keyword.trim()) {
      params.set("keyword", keyword.trim());
    }

    void fetchResources(params, deviceId, userToken)
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
  }, [deviceId, keyword, userToken]);

  async function loadMore() {
    if (isLoadingMore || !hasNextPage) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const params = new URLSearchParams({
        page: String(page + 1),
        pageSize: String(PAGE_SIZE)
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
          <div>
            <Text weight="semibold" size={800}>
              资源库
            </Text>
            <Text className={styles.muted}>查看当前后端已经索引完成的媒体文件。</Text>
          </div>

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
          <Text weight="semibold">已加载资源</Text>
          <Text>{items.length}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">命中总数</Text>
          <Text>{total}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前可播</Text>
          <Text>{playableCount}</Text>
        </Card>
      </div>

      {isLoading ? <Spinner label="正在读取资源库..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有命中的资源</Text>
          <Text className={styles.muted}>等下载执行完成并索引之后，资源会出现在这里。</Text>
        </Card>
      ) : null}

      <div className={styles.list}>
        {items.map((item) => (
          <Card key={item.id} className={styles.itemCard}>
            <Text weight="semibold">{item.fileName}</Text>
            <Text className={styles.muted}>Bangumi {item.bangumiSubjectId}</Text>
            <Text>{describeEpisode(item)}</Text>
            <Text>{formatBytes(item.sizeBytes)}</Text>
            <Text className={styles.muted}>{item.sourceFansubName ?? item.sourceTitle}</Text>
          </Card>
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
