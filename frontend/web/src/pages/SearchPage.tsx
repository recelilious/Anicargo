import { startTransition, useEffect, useRef, useState } from "react";
import {
  Button,
  Card,
  Field,
  Input,
  Select,
  Spinner,
  Text,
  makeStyles,
  tokens,
} from "@fluentui/react-components";

import { searchSubjects } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { SearchResponse, SubjectCard as SubjectCardModel } from "../types";

type SearchFormState = {
  keyword: string;
  sort: "score" | "rank" | "heat" | "match";
  year: string;
  season: "" | "winter" | "spring" | "summer" | "fall";
  tagInput: string;
  metaTagInput: string;
  startDate: string;
  endDate: string;
  ratingMin: string;
  ratingMax: string;
  ratingCountMin: string;
  ratingCountMax: string;
  rankMin: string;
  rankMax: string;
  nsfwMode: "any" | "safe" | "only";
};

type SearchRequestModel = {
  keyword: string;
  sort: SearchFormState["sort"];
  tags: string[];
  metaTags: string[];
  airDateStart: string | null;
  airDateEnd: string | null;
  ratingMin: string | null;
  ratingMax: string | null;
  ratingCountMin: string | null;
  ratingCountMax: string | null;
  rankMin: string | null;
  rankMax: string | null;
  nsfwMode: SearchFormState["nsfwMode"];
  pageSize: number;
};

type SearchPageCache = {
  form: SearchFormState;
  pageSize: number;
  page: number;
  items: SubjectCardModel[];
  response: SearchResponse;
  activeQuerySignature: string;
  autoLoadUnlocked: boolean;
  showLoadMoreButton: boolean;
};

const EMPTY_RESPONSE: SearchResponse = {
  items: [],
  facets: { years: [], tags: [] },
  total: 0,
  page: 1,
  pageSize: 20,
  hasNextPage: false,
};

const DEFAULT_FORM: SearchFormState = {
  keyword: "",
  sort: "score",
  year: "",
  season: "",
  tagInput: "",
  metaTagInput: "",
  startDate: "",
  endDate: "",
  ratingMin: "",
  ratingMax: "",
  ratingCountMin: "",
  ratingCountMax: "",
  rankMin: "",
  rankMax: "",
  nsfwMode: "safe",
};

const CARD_MIN_WIDTH = 210;
const CARD_GAP = 16;
const LOAD_MORE_DISTANCE = 160;

const searchPageStateCache = new Map<string, SearchPageCache>();

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
    minHeight: "100%",
  },
  searchBar: {
    position: "sticky",
    top: 0,
    zIndex: 5,
    padding: "20px 22px 16px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  headerRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    marginBottom: "14px",
  },
  filterGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
    gap: "12px",
    alignItems: "end",
  },
  keywordField: {
    gridColumn: "span 2",
  },
  footerRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    marginTop: "14px",
    paddingTop: "12px",
    borderTop: "1px solid var(--app-border)",
  },
  actions: {
    display: "flex",
    justifyContent: "flex-end",
  },
  results: {
    display: "flex",
    flexDirection: "column",
    gap: "16px",
    minWidth: 0,
  },
  gridHost: {
    minWidth: 0,
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px",
  },
  loadMoreRow: {
    display: "flex",
    justifyContent: "center",
    paddingBottom: "8px",
  },
  statusRow: {
    display: "flex",
    justifyContent: "center",
    paddingBottom: "8px",
  },
  muted: {
    color: "var(--app-muted)",
  },
  metaText: {
    color: "var(--app-muted)",
    fontVariantNumeric: "tabular-nums",
  },
});

function splitTerms(value: string) {
  return value
    .split(/[,，]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function buildDateRange(form: SearchFormState) {
  if (form.startDate || form.endDate) {
    return {
      airDateStart: form.startDate || null,
      airDateEnd: form.endDate || null,
    };
  }

  if (!form.year) {
    return {
      airDateStart: null,
      airDateEnd: null,
    };
  }

  if (!form.season) {
    return {
      airDateStart: `${form.year}-01-01`,
      airDateEnd: `${form.year}-12-31`,
    };
  }

  switch (form.season) {
    case "winter":
      return { airDateStart: `${form.year}-01-01`, airDateEnd: `${form.year}-03-31` };
    case "spring":
      return { airDateStart: `${form.year}-04-01`, airDateEnd: `${form.year}-06-30` };
    case "summer":
      return { airDateStart: `${form.year}-07-01`, airDateEnd: `${form.year}-09-30` };
    case "fall":
      return { airDateStart: `${form.year}-10-01`, airDateEnd: `${form.year}-12-31` };
    default:
      return { airDateStart: null, airDateEnd: null };
  }
}

function buildRequestModel(form: SearchFormState, pageSize: number): SearchRequestModel {
  const { airDateStart, airDateEnd } = buildDateRange(form);

  return {
    keyword: form.keyword.trim(),
    sort: form.sort,
    tags: splitTerms(form.tagInput),
    metaTags: splitTerms(form.metaTagInput),
    airDateStart,
    airDateEnd,
    ratingMin: form.ratingMin || null,
    ratingMax: form.ratingMax || null,
    ratingCountMin: form.ratingCountMin || null,
    ratingCountMax: form.ratingCountMax || null,
    rankMin: form.rankMin || null,
    rankMax: form.rankMax || null,
    nsfwMode: form.nsfwMode,
    pageSize,
  };
}

function buildSearchParams(request: SearchRequestModel, page: number) {
  const params = new URLSearchParams({
    keyword: request.keyword,
    sort: request.sort,
    page: String(page),
    pageSize: String(request.pageSize),
    nsfwMode: request.nsfwMode,
  });

  for (const tag of request.tags) {
    params.append("tag", tag);
  }

  for (const metaTag of request.metaTags) {
    params.append("metaTag", metaTag);
  }

  if (request.airDateStart) {
    params.set("airDateStart", request.airDateStart);
  }

  if (request.airDateEnd) {
    params.set("airDateEnd", request.airDateEnd);
  }

  if (request.ratingMin) {
    params.set("ratingMin", request.ratingMin);
  }

  if (request.ratingMax) {
    params.set("ratingMax", request.ratingMax);
  }

  if (request.ratingCountMin) {
    params.set("ratingCountMin", request.ratingCountMin);
  }

  if (request.ratingCountMax) {
    params.set("ratingCountMax", request.ratingCountMax);
  }

  if (request.rankMin) {
    params.set("rankMin", request.rankMin);
  }

  if (request.rankMax) {
    params.set("rankMax", request.rankMax);
  }

  return params;
}

function mergeItems(currentItems: SubjectCardModel[], nextItems: SubjectCardModel[]) {
  if (currentItems.length === 0) {
    return nextItems;
  }

  const seen = new Set(currentItems.map((item) => item.bangumiSubjectId));
  const merged = currentItems.slice();

  for (const item of nextItems) {
    if (seen.has(item.bangumiSubjectId)) {
      continue;
    }

    seen.add(item.bangumiSubjectId);
    merged.push(item);
  }

  return merged;
}

function useDebouncedValue<T>(value: T, delayMs: number) {
  const [debounced, setDebounced] = useState(value);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebounced(value);
    }, delayMs);

    return () => {
      window.clearTimeout(timer);
    };
  }, [delayMs, value]);

  return debounced;
}

function createSearchCacheKey(deviceId: string, userToken: string | null) {
  return `${deviceId}:${userToken ?? "guest"}`;
}

export function SearchPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const cacheKey = createSearchCacheKey(deviceId, userToken);
  const cachedState = searchPageStateCache.get(cacheKey);
  const gridHostRef = useRef<HTMLDivElement | null>(null);
  const loadLockRef = useRef(false);
  const [form, setForm] = useState<SearchFormState>(() => cachedState?.form ?? DEFAULT_FORM);
  const [pageSize, setPageSize] = useState(() => cachedState?.pageSize ?? 20);
  const [page, setPage] = useState(() => cachedState?.page ?? 1);
  const [items, setItems] = useState<SubjectCardModel[]>(() => cachedState?.items ?? []);
  const [response, setResponse] = useState<SearchResponse>(() => cachedState?.response ?? EMPTY_RESPONSE);
  const [isInitialLoading, setIsInitialLoading] = useState(() => cachedState == null);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [autoLoadUnlocked, setAutoLoadUnlocked] = useState(
    () => cachedState?.autoLoadUnlocked ?? false,
  );
  const [showLoadMoreButton, setShowLoadMoreButton] = useState(
    () => cachedState?.showLoadMoreButton ?? false,
  );

  const debouncedKeyword = useDebouncedValue(form.keyword, 280);
  const debouncedTagInput = useDebouncedValue(form.tagInput, 280);
  const debouncedMetaTagInput = useDebouncedValue(form.metaTagInput, 280);

  const requestModel = buildRequestModel(
    {
      ...form,
      keyword: debouncedKeyword,
      tagInput: debouncedTagInput,
      metaTagInput: debouncedMetaTagInput,
    },
    pageSize,
  );
  const querySignature = JSON.stringify(requestModel);
  const [activeQuerySignature, setActiveQuerySignature] = useState(
    () => cachedState?.activeQuerySignature ?? querySignature,
  );

  function requestNextPage() {
    if (loadLockRef.current) {
      return;
    }

    loadLockRef.current = true;
    setPage((current) => current + 1);
  }

  function handleReachLoadThreshold(scrollRoot: HTMLElement) {
    if (isInitialLoading || isLoadingMore || !response.hasNextPage || error) {
      return;
    }

    const distanceToBottom =
      scrollRoot.scrollHeight - scrollRoot.clientHeight - scrollRoot.scrollTop;

    if (distanceToBottom > LOAD_MORE_DISTANCE) {
      return;
    }

    if (!autoLoadUnlocked) {
      setShowLoadMoreButton(true);
      return;
    }

    requestNextPage();
  }

  useEffect(() => {
    const element = gridHostRef.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }

    const updatePageSize = () => {
      const width = element.clientWidth;
      const columnCount = Math.max(1, Math.floor((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)));
      const nextPageSize = Math.max(10, Math.min(60, columnCount * 5));

      setPageSize((current) => (current === nextPageSize ? current : nextPageSize));
    };

    updatePageSize();

    const observer = new ResizeObserver(() => {
      updatePageSize();
    });

    observer.observe(element);

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    if (querySignature === activeQuerySignature) {
      return;
    }

    const scrollRoot = document.getElementById("app-scroll-root");
    scrollRoot?.scrollTo({ top: 0, behavior: "auto" });
    loadLockRef.current = false;

    startTransition(() => {
      setItems([]);
      setResponse(EMPTY_RESPONSE);
      setError(null);
      setPage(1);
      setShowLoadMoreButton(false);
      setActiveQuerySignature(querySignature);
    });
  }, [activeQuerySignature, querySignature]);

  useEffect(() => {
    if (activeQuerySignature !== querySignature) {
      return;
    }

    let cancelled = false;
    const activeRequest = JSON.parse(activeQuerySignature) as SearchRequestModel;
    const params = buildSearchParams(activeRequest, page);
    const loadingMore = page > 1;

    if (loadingMore) {
      setIsLoadingMore(true);
    } else {
      setIsInitialLoading(true);
    }

    void searchSubjects(params, deviceId, userToken)
      .then((nextResponse) => {
        if (cancelled) {
          return;
        }

        startTransition(() => {
          setResponse(nextResponse);
          setItems((currentItems) =>
            page === 1 ? nextResponse.items : mergeItems(currentItems, nextResponse.items),
          );
          setError(null);
        });
      })
      .catch((nextError: Error) => {
        if (cancelled) {
          return;
        }

        setError(nextError.message);
        if (page === 1) {
          setItems([]);
          setResponse(EMPTY_RESPONSE);
        }
      })
      .finally(() => {
        if (cancelled) {
          return;
        }

        loadLockRef.current = false;
        if (loadingMore) {
          setIsLoadingMore(false);
        } else {
          setIsInitialLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [activeQuerySignature, deviceId, page, querySignature, userToken]);

  useEffect(() => {
    const scrollRoot = document.getElementById("app-scroll-root");
    if (!scrollRoot) {
      return;
    }

    let ticking = false;

    const handleScroll = () => {
      if (ticking) {
        return;
      }

      ticking = true;
      window.requestAnimationFrame(() => {
        ticking = false;
        handleReachLoadThreshold(scrollRoot);
      });
    };

    scrollRoot.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      scrollRoot.removeEventListener("scroll", handleScroll);
    };
  }, [autoLoadUnlocked, error, isInitialLoading, isLoadingMore, response.hasNextPage]);

  useEffect(() => {
    const scrollRoot = document.getElementById("app-scroll-root");
    if (!scrollRoot) {
      return;
    }

    if (isInitialLoading || isLoadingMore || !response.hasNextPage || error) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      handleReachLoadThreshold(scrollRoot);
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [
    autoLoadUnlocked,
    error,
    isInitialLoading,
    isLoadingMore,
    items.length,
    response.hasNextPage,
  ]);

  function updateForm<K extends keyof SearchFormState>(key: K, value: SearchFormState[K]) {
    setForm((current) => ({
      ...current,
      [key]: value,
    }));
  }

  function resetFilters() {
    setForm(DEFAULT_FORM);
  }

  function handleLoadMore() {
    if (loadLockRef.current || isInitialLoading || isLoadingMore || !response.hasNextPage || error) {
      return;
    }

    setAutoLoadUnlocked(true);
    setShowLoadMoreButton(false);
    requestNextPage();
  }

  useEffect(() => {
    searchPageStateCache.set(cacheKey, {
      form,
      pageSize,
      page,
      items,
      response,
      activeQuerySignature,
      autoLoadUnlocked,
      showLoadMoreButton,
    });
  }, [
    activeQuerySignature,
    autoLoadUnlocked,
    cacheKey,
    form,
    items,
    page,
    pageSize,
    response,
    showLoadMoreButton,
  ]);

  return (
    <section className={styles.page}>
      <Card className={styles.searchBar}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            搜索
          </Text>
          <Text size={200} className={styles.muted}>
            Bangumi 动画条目
          </Text>
        </div>

        <div className={styles.filterGrid}>
          <Field label="关键词" className={styles.keywordField}>
            <Input
              value={form.keyword}
              onChange={(_, data) => updateForm("keyword", data.value)}
              placeholder="番名 / 别名 / 关键词"
            />
          </Field>

          <Field label="排序">
            <Select
              value={form.sort}
              onChange={(event) => updateForm("sort", event.target.value as SearchFormState["sort"])}
            >
              <option value="score">评分</option>
              <option value="rank">排名</option>
              <option value="heat">热度</option>
              <option value="match">匹配度</option>
            </Select>
          </Field>

          <Field label="年份">
            <Input
              type="number"
              value={form.year}
              onChange={(_, data) => updateForm("year", data.value)}
              placeholder="例如 2026"
            />
          </Field>

          <Field label="季度">
            <Select
              value={form.season}
              onChange={(event) =>
                updateForm("season", event.target.value as SearchFormState["season"])
              }
            >
              <option value="">全年</option>
              <option value="winter">冬</option>
              <option value="spring">春</option>
              <option value="summer">夏</option>
              <option value="fall">秋</option>
            </Select>
          </Field>

          <Field label="用户标签">
            <Input
              value={form.tagInput}
              onChange={(_, data) => updateForm("tagInput", data.value)}
              placeholder="逗号分隔"
            />
          </Field>

          <Field label="公共标签">
            <Input
              value={form.metaTagInput}
              onChange={(_, data) => updateForm("metaTagInput", data.value)}
              placeholder="支持 -标签 排除"
            />
          </Field>

          <Field label="起始日期">
            <Input
              type="date"
              value={form.startDate}
              onChange={(_, data) => updateForm("startDate", data.value)}
            />
          </Field>

          <Field label="结束日期">
            <Input
              type="date"
              value={form.endDate}
              onChange={(_, data) => updateForm("endDate", data.value)}
            />
          </Field>

          <Field label="最低评分">
            <Input
              type="number"
              step="0.1"
              value={form.ratingMin}
              onChange={(_, data) => updateForm("ratingMin", data.value)}
            />
          </Field>

          <Field label="最高评分">
            <Input
              type="number"
              step="0.1"
              value={form.ratingMax}
              onChange={(_, data) => updateForm("ratingMax", data.value)}
            />
          </Field>

          <Field label="最少评分人数">
            <Input
              type="number"
              value={form.ratingCountMin}
              onChange={(_, data) => updateForm("ratingCountMin", data.value)}
            />
          </Field>

          <Field label="最多评分人数">
            <Input
              type="number"
              value={form.ratingCountMax}
              onChange={(_, data) => updateForm("ratingCountMax", data.value)}
            />
          </Field>

          <Field label="排名下限">
            <Input
              type="number"
              value={form.rankMin}
              onChange={(_, data) => updateForm("rankMin", data.value)}
            />
          </Field>

          <Field label="排名上限">
            <Input
              type="number"
              value={form.rankMax}
              onChange={(_, data) => updateForm("rankMax", data.value)}
            />
          </Field>

          <Field label="R18">
            <Select
              value={form.nsfwMode}
              onChange={(event) =>
                updateForm("nsfwMode", event.target.value as SearchFormState["nsfwMode"])
              }
            >
              <option value="safe">仅非 R18</option>
              <option value="any">交给 Bangumi 默认处理</option>
              <option value="only">仅 R18</option>
            </Select>
          </Field>
        </div>

        <div className={styles.footerRow}>
          <Text size={200} className={styles.metaText}>
            已加载 {items.length} / {response.total} · 每次追加 {pageSize} 条
          </Text>

          <div className={styles.actions}>
            <Button appearance="secondary" onClick={resetFilters}>
              清空筛选
            </Button>
          </div>
        </div>
      </Card>

      <div className={styles.results}>
        {error ? <Text>{error}</Text> : null}
        {isInitialLoading ? <Spinner label="正在同步 Bangumi 条目..." /> : null}
        {!isInitialLoading && !error && items.length === 0 ? <Text>没有匹配的条目。</Text> : null}

        <div ref={gridHostRef} className={styles.gridHost}>
          <div className={styles.grid}>
            {items.map((subject) => (
              <SubjectCard key={subject.bangumiSubjectId} subject={subject} metaVariant="catalog" />
            ))}
          </div>
        </div>

        {showLoadMoreButton && !isInitialLoading && !isLoadingMore && response.hasNextPage ? (
          <div className={styles.loadMoreRow}>
            <Button appearance="secondary" onClick={handleLoadMore}>
              加载更多
            </Button>
          </div>
        ) : null}

        {isLoadingMore ? <Spinner label="正在加载更多..." /> : null}
        {!isInitialLoading && !isLoadingMore && !response.hasNextPage && items.length > 0 ? (
          <div className={styles.statusRow}>
            <Text size={200} className={styles.muted}>
              已经到底了。
            </Text>
          </div>
        ) : null}
      </div>
    </section>
  );
}
