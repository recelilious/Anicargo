import { startTransition, useEffect, useRef, useState } from "react";
import { Button, Card, Field, Input, Select, Text, makeStyles } from "@fluentui/react-components";
import { ArrowUpRegular } from "@fluentui/react-icons";

import { searchSubjects } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence } from "../motion";
import { useSession } from "../session";
import { useUiPreferences } from "../ui-preferences";
import type { SearchResponse, SubjectCard as SubjectCardModel } from "../types";

type SearchFormState = {
  keyword: string;
  sort: "score" | "rank" | "heat" | "match";
  year: string;
  season: "" | "winter" | "spring" | "summer" | "fall";
  startDate: string;
  endDate: string;
  ratingMin: string;
  ratingMax: string;
};

type SearchRequestModel = {
  keyword: string;
  sort: SearchFormState["sort"];
  airDateStart: string | null;
  airDateEnd: string | null;
  ratingMin: string | null;
  ratingMax: string | null;
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
  startDate: "",
  endDate: "",
  ratingMin: "",
  ratingMax: "",
};

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
  titleCard: {
    padding: "20px 22px 16px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  filterCard: {
    padding: "20px 22px 16px",
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
  filterRows: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  primaryRow: {
    display: "grid",
    gridTemplateColumns: "minmax(280px, 1fr) 180px",
    gap: "12px",
    alignItems: "end",
  },
  secondaryRow: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
    gap: "12px",
    alignItems: "end",
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
    gridTemplateColumns: "repeat(auto-fill, minmax(var(--app-subject-card-min-width), 1fr))",
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
  backToTopButton: {
    position: "fixed",
    top: "24px",
    left: "50%",
    transform: "translateX(-50%)",
    zIndex: 8,
    width: "58px",
    height: "58px",
    minWidth: "58px",
    padding: 0,
    borderRadius: "999px",
    border: "2px solid var(--app-border-strong)",
    boxShadow: "var(--app-card-shadow-strong)",
  },
});

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
    airDateStart,
    airDateEnd,
    ratingMin: form.ratingMin || null,
    ratingMax: form.ratingMax || null,
    pageSize,
  };
}

function buildSearchParams(request: SearchRequestModel, page: number) {
  const params = new URLSearchParams({
    keyword: request.keyword,
    sort: request.sort,
    page: String(page),
    pageSize: String(request.pageSize),
  });

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
  const { uiScaleProfile } = useUiPreferences();
  const cacheKey = createSearchCacheKey(deviceId, userToken);
  const cachedState = searchPageStateCache.get(cacheKey);
  const gridHostRef = useRef<HTMLDivElement | null>(null);
  const filterCardRef = useRef<HTMLDivElement | null>(null);
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
  const [showBackToTop, setShowBackToTop] = useState(false);
  useLoadingStatus(
    isInitialLoading ? "正在同步 Bangumi 条目..." : isLoadingMore ? "正在加载更多搜索结果..." : null,
  );

  const debouncedKeyword = useDebouncedValue(form.keyword, 280);
  const requestModel = buildRequestModel(
    {
      ...form,
      keyword: debouncedKeyword,
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

  function updateBackToTopVisibility(scrollRoot: HTMLElement) {
    const filterCard = filterCardRef.current;
    if (!filterCard) {
      setShowBackToTop(false);
      return;
    }

    const scrollBounds = scrollRoot.getBoundingClientRect();
    const filterBounds = filterCard.getBoundingClientRect();
    setShowBackToTop(filterBounds.bottom <= scrollBounds.top + 12);
  }

  useEffect(() => {
    const element = gridHostRef.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }

    const updatePageSize = () => {
      const width = element.clientWidth;
      const columnCount = Math.max(
        1,
        Math.floor((width + CARD_GAP) / (uiScaleProfile.subjectCardMinWidth + CARD_GAP)),
      );
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
  }, [uiScaleProfile.subjectCardMinWidth]);

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
        updateBackToTopVisibility(scrollRoot);
      });
    };

    handleScroll();
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

    updateBackToTopVisibility(scrollRoot);

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

  function handleBackToTop() {
    const scrollRoot = document.getElementById("app-scroll-root");
    scrollRoot?.scrollTo({ top: 0, behavior: "smooth" });
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
    <MotionPage className={styles.page}>
      <MotionPresence show={showBackToTop}>
        <Button
          appearance="secondary"
          icon={<ArrowUpRegular />}
          className={styles.backToTopButton}
          onClick={handleBackToTop}
          aria-label="回到顶部"
        />
      </MotionPresence>

      <Card className={`${styles.titleCard} app-motion-surface`}>
        <div className={styles.headerRow}>
          <Text weight="semibold" size={800}>
            搜索
          </Text>
          <Text size={300} className={styles.headerSource}>
            Bangumi 动画条目
          </Text>
        </div>
      </Card>

      <Card
        ref={filterCardRef}
        className={`${styles.filterCard} app-motion-surface`}
        style={{ ["--motion-delay" as string]: "48ms" }}
      >
        <div className={styles.filterRows}>
          <div className={styles.primaryRow}>
            <Field label="关键词">
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
          </div>

          <div className={styles.secondaryRow}>
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
          </div>
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
        <MotionPresence show={Boolean(error)} mode="soft">
          {error ? <Text>{error}</Text> : null}
        </MotionPresence>
        {!isInitialLoading && !error && items.length === 0 ? <Text>没有匹配的条目。</Text> : null}

        <div ref={gridHostRef} className={styles.gridHost}>
          <div className={styles.grid}>
            {items.map((subject, index) => (
              <SubjectCard
                key={subject.bangumiSubjectId}
                subject={subject}
                metaVariant="catalog"
                motionIndex={index}
              />
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
        {!isInitialLoading && !isLoadingMore && !response.hasNextPage && items.length > 0 ? (
          <div className={styles.statusRow}>
            <Text size={200} className={styles.muted}>
              已经到底了。
            </Text>
          </div>
        ) : null}
      </div>
    </MotionPage>
  );
}
