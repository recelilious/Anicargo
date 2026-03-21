import { startTransition, useDeferredValue, useEffect, useState } from "react";
import { Button, Card, Field, Input, Select, Spinner, Text, makeStyles, tokens } from "@fluentui/react-components";

import { searchSubjects } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { SearchResponse } from "../types";

const EMPTY_RESPONSE: SearchResponse = {
  items: [],
  facets: { years: [], tags: [] },
  total: 0,
  page: 1,
  pageSize: 20,
  hasNextPage: false
};

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  searchBar: {
    padding: "24px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  filters: {
    display: "grid",
    gridTemplateColumns: "minmax(260px, 2fr) repeat(2, minmax(140px, 1fr))",
    gap: "12px",
    alignItems: "end"
  },
  summary: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    padding: "12px 16px",
    backgroundColor: "var(--app-panel)",
    border: "1px solid var(--app-border)",
    borderRadius: tokens.borderRadiusLarge
  },
  pager: {
    display: "flex",
    alignItems: "center",
    gap: "8px"
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px"
  }
});

export function SearchPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const [keyword, setKeyword] = useState("");
  const [selectedYear, setSelectedYear] = useState("");
  const [selectedTag, setSelectedTag] = useState("");
  const [response, setResponse] = useState<SearchResponse>(EMPTY_RESPONSE);
  const [page, setPage] = useState(1);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const deferredKeyword = useDeferredValue(keyword);

  useEffect(() => {
    let isMounted = true;
    const params = new URLSearchParams({
      keyword: deferredKeyword.trim(),
      sort: "rating",
      page: String(page),
      pageSize: "20"
    });

    if (selectedYear) {
      params.set("year", selectedYear);
    }
    if (selectedTag) {
      params.set("tag", selectedTag);
    }

    setIsLoading(true);

    void searchSubjects(params, deviceId, userToken)
      .then((nextResponse) => {
        if (!isMounted) {
          return;
        }

        startTransition(() => {
          setResponse(nextResponse);
          setError(null);
        });
      })
      .catch((nextError: Error) => {
        if (isMounted) {
          setError(nextError.message);
          setResponse(EMPTY_RESPONSE);
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
  }, [deferredKeyword, selectedYear, selectedTag, page, deviceId, userToken]);

  function updateKeyword(value: string) {
    setKeyword(value);
    setPage(1);
  }

  function updateYear(value: string) {
    setSelectedYear(value);
    setPage(1);
  }

  function updateTag(value: string) {
    setSelectedTag(value);
    setPage(1);
  }

  const totalPages = Math.max(1, Math.ceil(response.total / response.pageSize));

  return (
    <section className={styles.page}>
      <Card className={styles.searchBar}>
        <Text weight="semibold" size={800}>
          搜索
        </Text>
        <div className={styles.filters}>
          <Field label="关键词">
            <Input value={keyword} onChange={(_, data) => updateKeyword(data.value)} placeholder="番名 / 别名 / 关键词" />
          </Field>

          <Field label="年份">
            <Select value={selectedYear} onChange={(event) => updateYear(event.target.value)}>
              <option value="">全部</option>
              {response.facets.years.map((year) => (
                <option key={year} value={String(year)}>
                  {year}
                </option>
              ))}
            </Select>
          </Field>

          <Field label="标签">
            <Select value={selectedTag} onChange={(event) => updateTag(event.target.value)}>
              <option value="">全部</option>
              {response.facets.tags.map((tag) => (
                <option key={tag} value={tag}>
                  {tag}
                </option>
              ))}
            </Select>
          </Field>
        </div>
      </Card>

      <div className={styles.summary}>
        <Text size={300}>
          第 {response.page} 页 / 共 {totalPages} 页
        </Text>
        <div className={styles.pager}>
          <Text size={300}>共 {response.total} 条</Text>
          <Button appearance="secondary" disabled={page <= 1 || isLoading} onClick={() => setPage((current) => Math.max(1, current - 1))}>
            上一页
          </Button>
          <Button appearance="secondary" disabled={!response.hasNextPage || isLoading} onClick={() => setPage((current) => current + 1)}>
            下一页
          </Button>
        </div>
      </div>

      {isLoading ? <Spinner label="正在同步 Bangumi 条目..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {!isLoading && !error && response.items.length === 0 ? <Text>没有匹配的条目。</Text> : null}

      <div className={styles.grid}>
        {response.items.map((subject) => (
          <SubjectCard key={subject.bangumiSubjectId} subject={subject} />
        ))}
      </div>
    </section>
  );
}
