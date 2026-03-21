import { useState } from "react";
import { Card, Field, Input, Text, makeStyles } from "@fluentui/react-components";

const useStyles = makeStyles({
  page: {
    display: "flex",
    flexDirection: "column",
    gap: "18px"
  },
  surfaceCard: {
    padding: "20px 22px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  stats: {
    display: "grid",
    gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
    gap: "12px"
  },
  muted: {
    color: "var(--app-muted)"
  }
});

export function ResourcesPage() {
  const styles = useStyles();
  const [keyword, setKeyword] = useState("");

  return (
    <section className={styles.page}>
      <Card className={styles.surfaceCard}>
        <Text weight="semibold" size={800}>
          资源
        </Text>
        <Field label="搜索">
          <Input value={keyword} onChange={(_, data) => setKeyword(data.value)} placeholder="番名 / Bangumi ID / 文件名" />
        </Field>
      </Card>

      <div className={styles.stats}>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">已入库资源</Text>
          <Text>0</Text>
        </Card>
        <Card className={styles.surfaceCard}>
          <Text weight="semibold">可播放条目</Text>
          <Text>0</Text>
        </Card>
      </div>

      <Card className={styles.surfaceCard}>
        <Text weight="semibold">当前资源库为空</Text>
        <Text className={styles.muted}>暂无可用资源。</Text>
      </Card>
    </section>
  );
}
