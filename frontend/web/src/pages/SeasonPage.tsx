import { useEffect, useRef, useState } from "react";
import { Card, Spinner, Text, makeStyles, tokens } from "@fluentui/react-components";

import { fetchCalendar } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { CalendarDay } from "../types";

const calendarDataCache = new Map<string, CalendarDay[]>();
const calendarRequestCache = new Map<string, Promise<CalendarDay[]>>();
const calendarSelectionCache = new Map<string, number>();

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px",
  },
  header: {
    padding: "18px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  weekSwitch: {
    display: "grid",
    gridTemplateColumns: "repeat(7, minmax(0, 1fr))",
    gap: "8px",
    padding: "8px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  weekButton: {
    position: "relative",
    appearance: "none",
    border: "none",
    backgroundColor: "transparent",
    color: "var(--app-muted)",
    padding: "10px 8px 14px",
    cursor: "pointer",
    font: "inherit",
    transition: "color 160ms ease",
    "&:hover": {
      color: "var(--app-text)",
    },
    "&:focus-visible": {
      outlineOffset: "-2px",
    },
    "&::after": {
      content: '""',
      position: "absolute",
      left: "10px",
      right: "10px",
      bottom: 0,
      height: "3px",
      borderRadius: tokens.borderRadiusCircular,
      backgroundColor: "transparent",
      transition: "background-color 160ms ease",
    },
  },
  weekButtonActive: {
    color: "var(--app-text)",
    fontWeight: tokens.fontWeightSemibold,
    "&::after": {
      backgroundColor: "var(--app-selected-fg)",
    },
  },
  viewport: {
    overflow: "hidden",
  },
  track: {
    display: "flex",
    alignItems: "flex-start",
    willChange: "transform",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.22, 1, 0.36, 1)",
  },
  panel: {
    flex: "0 0 100%",
    minWidth: "100%",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px",
    paddingTop: "4px",
  },
  panelPlaceholder: {
    minHeight: "1px",
  },
});

function currentBangumiWeekday() {
  const weekday = new Date().getDay();
  return weekday === 0 ? 7 : weekday;
}

function createCacheKey(deviceId: string, userToken: string | null) {
  return `${deviceId}:${userToken ?? "guest"}`;
}

function resolveSelectedDay(days: CalendarDay[], cachedDay: number | undefined) {
  if (cachedDay && days.some((day) => day.weekday.id === cachedDay)) {
    return cachedDay;
  }

  const today = currentBangumiWeekday();
  if (days.some((day) => day.weekday.id === today)) {
    return today;
  }

  return days[0]?.weekday.id ?? today;
}

function collectMountedDayIds(days: CalendarDay[], fromDay: number, toDay: number) {
  const fromIndex = days.findIndex((day) => day.weekday.id === fromDay);
  const toIndex = days.findIndex((day) => day.weekday.id === toDay);

  if (fromIndex === -1 || toIndex === -1) {
    return [toDay];
  }

  const start = Math.min(fromIndex, toIndex);
  const end = Math.max(fromIndex, toIndex);
  return days.slice(start, end + 1).map((day) => day.weekday.id);
}

async function loadCalendar(deviceId: string, userToken: string | null, cacheKey: string) {
  let request = calendarRequestCache.get(cacheKey);
  if (!request) {
    request = fetchCalendar(deviceId, userToken).then((response) => {
      calendarDataCache.set(cacheKey, response.days);
      calendarRequestCache.delete(cacheKey);
      return response.days;
    });

    request = request.catch((error) => {
      calendarRequestCache.delete(cacheKey);
      throw error;
    });

    calendarRequestCache.set(cacheKey, request);
  }

  return request;
}

export function SeasonPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const transitionTimerRef = useRef<number | null>(null);
  const cacheKey = createCacheKey(deviceId, userToken);
  const [days, setDays] = useState<CalendarDay[]>([]);
  const [selectedDay, setSelectedDay] = useState<number>(currentBangumiWeekday());
  const [mountedDayIds, setMountedDayIds] = useState<number[]>([]);
  const [slideDurationMs, setSlideDurationMs] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    return () => {
      if (transitionTimerRef.current != null) {
        window.clearTimeout(transitionTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const cachedDays = calendarDataCache.get(cacheKey);

    if (cachedDays) {
      const nextSelectedDay = resolveSelectedDay(
        cachedDays,
        calendarSelectionCache.get(cacheKey),
      );

      setDays(cachedDays);
      setSelectedDay(nextSelectedDay);
      setMountedDayIds([nextSelectedDay]);
      setSlideDurationMs(0);
      setError(null);
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(null);

    void loadCalendar(deviceId, userToken, cacheKey)
      .then((nextDays) => {
        if (cancelled) {
          return;
        }

        const nextSelectedDay = resolveSelectedDay(
          nextDays,
          calendarSelectionCache.get(cacheKey),
        );

        setDays(nextDays);
        setSelectedDay(nextSelectedDay);
        setMountedDayIds([nextSelectedDay]);
        setSlideDurationMs(0);
      })
      .catch((nextError: Error) => {
        if (!cancelled) {
          setError(nextError.message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [cacheKey, deviceId, userToken]);

  const selectedIndex = Math.max(
    0,
    days.findIndex((day) => day.weekday.id === selectedDay),
  );
  const mountedDaySet = new Set(mountedDayIds);

  function handleSelectDay(nextDay: number) {
    if (nextDay === selectedDay) {
      return;
    }

    const currentIndex = days.findIndex((day) => day.weekday.id === selectedDay);
    const nextIndex = days.findIndex((day) => day.weekday.id === nextDay);
    const distance =
      currentIndex === -1 || nextIndex === -1 ? 1 : Math.abs(nextIndex - currentIndex);
    const nextMountedDayIds = collectMountedDayIds(days, selectedDay, nextDay);
    const nextDuration = Math.min(220 + distance * 55, 520);

    if (transitionTimerRef.current != null) {
      window.clearTimeout(transitionTimerRef.current);
    }

    setMountedDayIds(nextMountedDayIds);
    setSlideDurationMs(nextDuration);
    setSelectedDay(nextDay);
    calendarSelectionCache.set(cacheKey, nextDay);

    transitionTimerRef.current = window.setTimeout(() => {
      setMountedDayIds([nextDay]);
      setSlideDurationMs(0);
    }, nextDuration + 32);
  }

  return (
    <section className={styles.page}>
      <Card className={styles.header}>
        <Text weight="semibold" size={800}>
          新番时间表
        </Text>
      </Card>

      {isLoading ? <Spinner label="正在同步时间表..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {days.length > 0 ? (
        <>
          <div className={styles.weekSwitch} role="tablist" aria-label="新番时间表星期切换">
            {days.map((day) => {
              const isActive = day.weekday.id === selectedDay;

              return (
                <button
                  key={day.weekday.id}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  className={`${styles.weekButton} ${isActive ? styles.weekButtonActive : ""}`.trim()}
                  onClick={() => handleSelectDay(day.weekday.id)}
                >
                  {day.weekday.cn}
                </button>
              );
            })}
          </div>

          <div className={styles.viewport}>
            <div
              className={styles.track}
              style={{
                transform: `translateX(-${selectedIndex * 100}%)`,
                transitionDuration: `${slideDurationMs}ms`,
              }}
            >
              {days.map((day) => (
                <div key={day.weekday.id} className={styles.panel}>
                  {mountedDaySet.has(day.weekday.id) ? (
                    <div className={styles.grid}>
                      {day.items.map((subject) => (
                        <SubjectCard
                          key={subject.bangumiSubjectId}
                          subject={subject}
                          metaVariant="schedule"
                        />
                      ))}
                    </div>
                  ) : (
                    <div className={styles.panelPlaceholder} aria-hidden="true" />
                  )}
                </div>
              ))}
            </div>
          </div>
        </>
      ) : null}
    </section>
  );
}
