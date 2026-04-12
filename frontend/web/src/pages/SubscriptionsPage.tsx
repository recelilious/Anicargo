import { useEffect, useState } from "react";
import { Button, Card, Field, Input, Select, Text, makeStyles } from "@fluentui/react-components";

import { fetchSubscriptions } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence } from "../motion";
import { useSession } from "../session";
import type { SubjectCard as SubjectCardModel } from "../types";

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
    flexDirection: "column",
    alignItems: "flex-start",
    gap: "8px",
  },
  headerSource: {
    color: "var(--app-muted)",
  },
  toolbarRow: {
    display: "flex",
    justifyContent: "space-between",
    gap: "16px",
    alignItems: "flex-start",
    flexWrap: "wrap",
  },
  controls: {
    display: "grid",
    gridTemplateColumns: "minmax(320px, 1.6fr) minmax(220px, 1fr)",
    gap: "16px",
    alignItems: "end",
    width: "min(760px, 100%)",
    minWidth: 0,
    "@media (max-width: 960px)": {
      gridTemplateColumns: "1fr",
    },
  },
  headerStats: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(120px, 1fr))",
    gap: "12px",
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
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(var(--app-subject-card-min-width), 1fr))",
    gap: "16px",
  },
  muted: {
    color: "var(--app-muted)",
  },
  actions: {
    display: "flex",
    justifyContent: "center",
  },
});

export function SubscriptionsPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const [keywordInput, setKeywordInput] = useState("");
  const [keyword, setKeyword] = useState("");
  const [sort, setSort] = useState("updated");
  const [items, setItems] = useState<SubjectCardModel[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [hasNextPage, setHasNextPage] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useLoadingStatus(
    isLoading ? "正在读取订阅..." : isLoadingMore ? "正在加载更多订阅..." : null,
  );

  useEffect(() => {
    let isMounted = true;
    setIsLoading(true);

    const params = new URLSearchParams({
      page: "1",
      pageSize: String(PAGE_SIZE),
      sort,
    });

    if (keyword.trim()) {
      params.set("keyword", keyword.trim());
    }

    void fetchSubscriptions(params, deviceId, userToken)
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
  }, [deviceId, keyword, sort, userToken]);

  async function loadMore() {
    if (isLoadingMore || !hasNextPage) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const params = new URLSearchParams({
        page: String(page + 1),
        pageSize: String(PAGE_SIZE),
        sort,
      });

      if (keyword.trim()) {
        params.set("keyword", keyword.trim());
      }

      const response = await fetchSubscriptions(params, deviceId, userToken);
      setItems((current) => [...current, ...response.items]);
      setTotal(response.total);
      setPage(response.page);
      setHasNextPage(response.hasNextPage);
    } finally {
      setIsLoadingMore(false);
    }
  }

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.surfaceCard} app-motion-surface`}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            我的订阅
          </Text>
          <Text size={300} className={styles.headerSource}>
            当前账号订阅的番剧
          </Text>
        </div>
      </Card>

      <Card className={`${styles.surfaceCard} app-motion-surface`} style={{ ["--motion-delay" as string]: "46ms" }}>
        <div className={styles.toolbarRow}>
          <div className={styles.controls}>
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
                placeholder="番名"
              />
            </Field>

            <Field label="排序">
              <Select value={sort} onChange={(event) => setSort(event.target.value)}>
                <option value="updated">按更新时间</option>
                <option value="rating">按评分</option>
                <option value="title">按标题</option>
              </Select>
            </Field>
          </div>

          <div className={styles.headerStats}>
            <div className={styles.statBox}>
              <Text size={200} className={styles.muted}>
                当前已加载
              </Text>
              <Text weight="semibold">{items.length}</Text>
            </div>
            <div className={styles.statBox}>
              <Text size={200} className={styles.muted}>
                订阅总数
              </Text>
              <Text weight="semibold">{total}</Text>
            </div>
          </div>
        </div>
      </Card>
      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有订阅中的番剧</Text>
        </Card>
      ) : null}

      <div className={styles.grid}>
        {items.map((item, index) => (
          <SubjectCard
            key={item.bangumiSubjectId}
            subject={item}
            metaVariant="catalog"
            motionIndex={index}
          />
        ))}
      </div>

      {hasNextPage ? (
        <div className={styles.actions}>
          <Button appearance="primary" onClick={() => void loadMore()} disabled={isLoadingMore}>
            加载更多
          </Button>
        </div>
      ) : null}
    </MotionPage>
  );
}
