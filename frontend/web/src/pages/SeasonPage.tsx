import { useEffect, useState } from "react";
import {
  Card,
  Spinner,
  Tab,
  TabList,
  Text,
  makeStyles
} from "@fluentui/react-components";

import { fetchCalendar } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { CalendarDay } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px"
  },
  hero: {
    padding: "24px 28px",
    background: "linear-gradient(135deg, #dbeeff 0%, #f6fbff 55%, #ffe8cf 100%)"
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px"
  }
});

function currentBangumiWeekday() {
  const weekday = new Date().getDay();
  return weekday === 0 ? 7 : weekday;
}

export function SeasonPage() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const [days, setDays] = useState<CalendarDay[]>([]);
  const [selectedDay, setSelectedDay] = useState<number>(currentBangumiWeekday());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let isMounted = true;

    setIsLoading(true);
    void fetchCalendar(deviceId, userToken)
      .then((response) => {
        if (!isMounted) {
          return;
        }

        setDays(response.days);
        const today = currentBangumiWeekday();
        const fallback = response.days[0]?.weekday.id ?? today;
        setSelectedDay(response.days.some((day) => day.weekday.id === today) ? today : fallback);
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
  }, [deviceId, userToken]);

  const activeDay = days.find((day) => day.weekday.id === selectedDay);

  return (
    <section className={styles.page}>
      <Card className={styles.hero}>
        <Text weight="semibold" size={800}>
          新番时间表
        </Text>
        <Text>
          默认打开到今天。这里只展示 Bangumi 当前季时间表里的连载条目，点进卡片后可以直接订阅。
        </Text>
      </Card>

      {isLoading ? <Spinner label="正在拉取 Bangumi 时间表..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {days.length > 0 ? (
        <>
          <TabList selectedValue={String(selectedDay)} onTabSelect={(_, data) => setSelectedDay(Number(data.value))}>
            {days.map((day) => (
              <Tab key={day.weekday.id} value={String(day.weekday.id)}>
                {day.weekday.cn}
              </Tab>
            ))}
          </TabList>

          <div className={styles.grid}>
            {(activeDay?.items ?? []).map((subject) => (
              <SubjectCard key={subject.bangumiSubjectId} subject={subject} />
            ))}
          </div>
        </>
      ) : null}
    </section>
  );
}
