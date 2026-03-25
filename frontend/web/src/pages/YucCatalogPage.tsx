import { useEffect, useState } from "react";
import { Card, Spinner, Text, makeStyles } from "@fluentui/react-components";

import { fetchCatalogPage } from "../api";
import { SubjectCard } from "../components/SubjectCard";
import { useSession } from "../session";
import type { CatalogPageResponse } from "../types";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "20px"
  },
  header: {
    padding: "18px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "12px"
  },
  sectionHeader: {
    padding: "14px 18px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  grid: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))",
    gap: "16px"
  },
  muted: {
    color: "var(--app-muted)"
  },
  emptyCard: {
    padding: "24px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  }
});

function CatalogPageView({
  kind,
  fallbackTitle
}: {
  kind: "preview" | "special";
  fallbackTitle: string;
}) {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
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
          {page?.title || fallbackTitle}
        </Text>
      </Card>

      {!page && !error ? <Spinner label="正在同步目录..." /> : null}
      {error ? <Text>{error}</Text> : null}
      {page && page.sections.length === 0 ? (
        <Card className={styles.emptyCard}>
          <Text weight="semibold">褰撳墠娌℃湁鍙樉绀虹殑鐩綍鍐呭</Text>
          <Text size={300} className={styles.muted}>
            濡傛灉鍒氬垰鍚姩鍚庣涓€娆℃墦寮€锛屽悗绔鍦ㄧ紦瀛樺拰鍖归厤 Bangumi 鏁版嵁銆?
          </Text>
        </Card>
      ) : null}

      {page?.sections.map((section) => (
        <section key={section.key} className={styles.section}>
          <Card className={styles.sectionHeader}>
            <Text weight="semibold">{section.title}</Text>
            <Text size={300} className={styles.muted}>
              {section.items.length} 部
            </Text>
          </Card>

          <div className={styles.grid}>
            {section.items.map((subject) => (
              <SubjectCard key={subject.bangumiSubjectId} subject={subject} metaVariant="catalog" />
            ))}
          </div>
        </section>
      ))}
    </section>
  );
}

export function PreviewPage() {
  return <CatalogPageView kind="preview" fallbackTitle="新季度前瞻" />;
}

export function SpecialPage() {
  return <CatalogPageView kind="special" fallbackTitle="特别放送" />;
}
