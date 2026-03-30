import { useEffect, useMemo, useState } from "react";
import { Card, Text, makeStyles } from "@fluentui/react-components";

import { fetchCatalogPage } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useLoadingStatus } from "../loading-status";
import { MotionPage, MotionPresence } from "../motion";
import { useSession } from "../session";
import type { CatalogPageResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px",
  },
  header: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    padding: "18px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  source: {
    color: "var(--app-muted)",
  },
  stack: {
    display: "flex",
    flexDirection: "column",
    gap: "18px",
  },
  sectionGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  sectionTitleCard: {
    padding: "16px 18px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  sectionHeader: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    flexWrap: "wrap",
  },
  sectionCount: {
    color: "var(--app-muted)",
  },
  sectionBody: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px",
  },
  emptyCard: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    padding: "24px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
  },
  muted: {
    color: "var(--app-muted)",
  },
});

function pageCopy() {
  return {
    title: "新季度前瞻",
    source: "新番卫星观测站 | 長門番堂",
    emptyTitle: "暂时还没有新的前瞻条目",
    emptyNote: "如果 Yuc 还没放出下一个季度页面或前瞻内容，这里会暂时保持为空。",
  };
}

function CatalogPageView() {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const copy = useMemo(() => pageCopy(), []);
  const [page, setPage] = useState<CatalogPageResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  useLoadingStatus(!page && !error ? "正在同步目录..." : null);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    void fetchCatalogPage("preview", deviceId, userToken)
      .then((response) => {
        if (!cancelled) {
          setPage(response);
        }
      })
      .catch((nextError: Error) => {
        if (!cancelled) {
          setError(nextError.message);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [deviceId, userToken]);

  return (
    <MotionPage className={styles.page}>
      <Card className={`${styles.header} app-motion-surface`}>
        <Text weight="semibold" size={800}>
          {copy.title}
        </Text>
        <Text size={300} className={styles.source}>
          {copy.source}
        </Text>
      </Card>
      <MotionPresence show={Boolean(error)} mode="soft">
        {error ? <Text>{error}</Text> : null}
      </MotionPresence>

      <MotionPresence show={Boolean(page && page.sections.length === 0)} mode="soft">
        <Card className={styles.emptyCard}>
          <Text weight="semibold">{copy.emptyTitle}</Text>
          <Text size={300} className={styles.muted}>
            {copy.emptyNote}
          </Text>
        </Card>
      </MotionPresence>
      {page?.sections.length ? (
        <div className={styles.stack}>
          {page.sections.map((section, sectionIndex) => (
            <section key={section.key} className={styles.sectionGroup}>
              <Card
                className={`${styles.sectionTitleCard} app-motion-surface`}
                style={{ ["--motion-delay" as string]: `${40 + sectionIndex * 46}ms` }}
              >
                <div className={styles.sectionHeader}>
                  <Text weight="semibold">{section.title}</Text>
                  <Text size={200} className={styles.sectionCount}>
                    {section.items.length} 项
                  </Text>
                </div>
              </Card>

              <div className={styles.sectionBody}>
                <div className={styles.grid}>
                  {section.items.map((subject, index) => (
                    <SubjectCard
                      key={`${section.key}-${subject.bangumiSubjectId}`}
                      subject={subject}
                      metaVariant="preview"
                      motionIndex={index}
                    />
                  ))}
                </div>
              </div>
            </section>
          ))}
        </div>
      ) : null}
    </MotionPage>
  );
}

export function PreviewPage() {
  return <CatalogPageView />;
}
