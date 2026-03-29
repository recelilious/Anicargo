import { useEffect, useMemo, useState } from "react";
import { Card, Spinner, Text, makeStyles } from "@fluentui/react-components";

import { fetchCatalogPage } from "../api";
import { SubjectCard } from "../components/SubjectCard";
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
  sectionShell: {
    display: "flex",
    flexDirection: "column",
    gap: "14px",
    padding: "18px 0",
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
    paddingInline: "18px",
  },
  sectionCount: {
    color: "var(--app-muted)",
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

type CatalogPageKind = "preview" | "special";

type PageCopy = {
  title: string;
  source: string;
  emptyTitle: string;
  emptyNote: string;
};

function pageCopy(kind: CatalogPageKind): PageCopy {
  if (kind === "preview") {
    return {
      title: "新季度前瞻",
      source: "来源：新番卫星观测站 | 長門番堂",
      emptyTitle: "暂时还没有新的前瞻条目",
      emptyNote: "如果 Yuc 还没有放出下一个季度页面或前瞻内容，这里会暂时保持为空。",
    };
  }

  return {
    title: "特别放送",
    source: "来源：Movie / OVA / OAD / SP etc. | 長門番堂",
    emptyTitle: "暂时还没有特别放送条目",
    emptyNote: "等 Yuc 更新特别放送页面后，这里会自动显示新的内容。",
  };
}

function CatalogPageView({ kind }: { kind: CatalogPageKind }) {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const copy = useMemo(() => pageCopy(kind), [kind]);
  const [page, setPage] = useState<CatalogPageResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    void fetchCatalogPage(kind, deviceId, userToken)
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
  }, [deviceId, kind, userToken]);

  return (
    <section className={styles.page}>
      <Card className={styles.header}>
        <Text weight="semibold" size={800}>
          {copy.title}
        </Text>
        <Text size={300} className={styles.source}>
          {copy.source}
        </Text>
      </Card>

      {!page && !error ? <Spinner label="正在同步目录..." /> : null}
      {error ? <Text>{error}</Text> : null}

      {page && page.sections.length === 0 ? (
        <Card className={styles.emptyCard}>
          <Text weight="semibold">{copy.emptyTitle}</Text>
          <Text size={300} className={styles.muted}>
            {copy.emptyNote}
          </Text>
        </Card>
      ) : null}

      {page?.sections.length ? (
        <div className={styles.stack}>
          {page.sections.map((section) => (
            <section key={section.key} className={styles.sectionShell}>
              <div className={styles.sectionHeader}>
                <Text weight="semibold">{section.title}</Text>
                <Text size={200} className={styles.sectionCount}>
                  {section.items.length} 部
                </Text>
              </div>

              <div className={styles.grid}>
                {section.items.map((subject) => (
                  <SubjectCard
                    key={`${section.key}-${subject.bangumiSubjectId}`}
                    subject={subject}
                    metaVariant="preview"
                  />
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : null}
    </section>
  );
}

export function PreviewPage() {
  return <CatalogPageView kind="preview" />;
}

export function SpecialPage() {
  return <CatalogPageView kind="special" />;
}
