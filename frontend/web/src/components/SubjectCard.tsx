import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import type { SubjectCard as SubjectCardType } from "../types";

const useStyles = makeStyles({
  link: {
    textDecorationLine: "none",
    color: "inherit"
  },
  card: {
    minHeight: "360px",
    display: "grid",
    gridTemplateRows: "220px auto",
    overflow: "hidden",
    backgroundColor: tokens.colorNeutralBackground1
  },
  poster: {
    backgroundSize: "cover",
    backgroundPosition: "center center",
    borderRadius: tokens.borderRadiusLarge,
    minHeight: "220px"
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    paddingTop: "4px"
  },
  tags: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap"
  },
  summary: {
    color: tokens.colorNeutralForeground3
  }
});

export function SubjectCard({ subject }: { subject: SubjectCardType }) {
  const styles = useStyles();

  return (
    <Link to={`/title/${subject.bangumiSubjectId}`} className={styles.link}>
      <Card className={styles.card} appearance="filled-alternative">
        <div
          className={styles.poster}
          style={{
            backgroundImage: subject.imagePortrait
              ? `url(${subject.imagePortrait})`
              : "linear-gradient(160deg, #d9ebff 0%, #b8d5ff 100%)"
          }}
        />
        <div className={styles.body}>
          <div>
            <Text weight="semibold">{subject.titleCn || subject.title}</Text>
            <Text block size={300}>
              {subject.title}
            </Text>
          </div>

          <div className={styles.tags}>
            {(subject.tags.length > 0 ? subject.tags : ["连载中"]).map((tag) => (
              <Badge key={tag} appearance="tint">
                {tag}
              </Badge>
            ))}
          </div>

          <Text size={300} className={styles.summary}>
            {subject.summary || "Bangumi 暂无简介。"}
          </Text>
        </div>
      </Card>
    </Link>
  );
}
