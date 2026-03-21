import { startTransition, useDeferredValue, useEffect, useState } from "react";
import {
  Card,
  Field,
  Input,
  Select,
  Spinner,
  Text,
  makeStyles
} from "@fluentui/react-components";

import { searchSubjects } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { SearchResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  searchBar: {
    padding: "24px"
  },
  filters: {
    display: "grid",
    gridTemplateColumns: "2fr 1fr 1fr",
    gap: "12px",
    alignItems: "end"
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
  const [response, setResponse] = useState<SearchResponse>({ items: [], facets: { years: [], tags: [] } });
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const deferredKeyword = useDeferredValue(keyword);

  useEffect(() => {
    if (deferredKeyword.trim().length < 2) {
      startTransition(() => {
        setResponse({ items: [], facets: { years: [], tags: [] } });
      });
      return;
    }

    const params = new URLSearchParams({ keyword: deferredKeyword.trim() });
    if (selectedYear) {
      params.set("year", selectedYear);
    }
    if (selectedTag) {
      params.set("tag", selectedTag);
    }

    let isMounted = true;
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
  }, [deferredKeyword, selectedYear, selectedTag, deviceId, userToken]);

  return (
    <section className={styles.page}>
      <Card className={styles.searchBar}>
        <Text weight="semibold" size={800}>
          搜索番剧
        </Text>
        <div className={styles.filters}>
          <Field label="搜索关键词">
            <Input
              value={keyword}
              onChange={(_, data) => setKeyword(data.value)}
              placeholder="输入番名、别名或关键词"
            />
          </Field>

          <Field label="年份筛选">
            <Select value={selectedYear} onChange={(event) => setSelectedYear(event.target.value)}>
              <option value="">全部年份</option>
              {response.facets.years.map((year) => (
                <option key={year} value={String(year)}>
                  {year}
                </option>
              ))}
            </Select>
          </Field>

          <Field label="Tag 筛选">
            <Select value={selectedTag} onChange={(event) => setSelectedTag(event.target.value)}>
              <option value="">全部标签</option>
              {response.facets.tags.map((tag) => (
                <option key={tag} value={tag}>
                  {tag}
                </option>
              ))}
            </Select>
          </Field>
        </div>
      </Card>

      {isLoading ? <Spinner label="正在搜索 Bangumi..." /> : null}
      {error ? <Text>{error}</Text> : null}

      <div className={styles.grid}>
        {response.items.map((subject) => (
          <SubjectCard key={subject.bangumiSubjectId} subject={subject} />
        ))}
      </div>
    </section>
  );
}
