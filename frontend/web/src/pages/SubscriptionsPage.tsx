import { useEffect, useState } from "react";
import { Button, Card, Field, Input, Select, Spinner, Text, makeStyles } from "@fluentui/react-components";

import { fetchSubscriptions } from "../api";
import { SubjectCard } from "../components/SubjectCard";
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
    justifyContent: "space-between",
    gap: "16px",
    alignItems: "flex-end",
    flexWrap: "wrap",
  },
  controls: {
    display: "grid",
    gridTemplateColumns: "minmax(220px, 1fr) 180px",
    gap: "12px",
    alignItems: "end",
    width: "min(560px, 100%)",
  },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
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
    <section className={styles.page}>
      <Card className={styles.surfaceCard}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            我的订阅
          </Text>

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
        </div>
      </Card>

      <div className={styles.stats}>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前已加载</Text>
          <Text>{items.length}</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">订阅总数</Text>
          <Text>{total}</Text>
        </Card>
      </div>

      {isLoading ? <Spinner label="正在读取订阅..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {!isLoading && items.length === 0 ? (
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">当前没有订阅中的番剧</Text>
        </Card>
      ) : null}

      <div className={styles.grid}>
        {items.map((item) => (
          <SubjectCard key={item.bangumiSubjectId} subject={item} metaVariant="catalog" />
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
