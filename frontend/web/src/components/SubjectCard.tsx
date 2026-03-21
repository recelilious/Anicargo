import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import type { SubjectCard as SubjectCardModel } from "../types";

const useStyles = makeStyles({
  link: {
    textDecorationLine: "none",
    color: "inherit",
    height: "100%"
  },
  card: {
    height: "452px",
    display: "grid",
    gridTemplateRows: "252px minmax(0, 1fr)",
    overflow: "hidden",
    backgroundColor: tokens.colorNeutralBackground1,
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  poster: {
    backgroundSize: "cover",
    backgroundPosition: "center center",
    borderRadius: tokens.borderRadiusLarge,
    minHeight: "252px",
    backgroundColor: "var(--app-fallback-hero)"
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    paddingTop: "8px",
    minHeight: 0
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minHeight: "108px"
  },
  title: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.4",
    minHeight: "2.8em",
    overflowWrap: "anywhere",
    wordBreak: "break-word"
  },
  subtitle: {
    color: tokens.colorNeutralForeground3,
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.4",
    minHeight: "2.8em",
    overflowWrap: "anywhere",
    wordBreak: "break-word"
  },
  tags: {
    display: "flex",
    gap: "8px",
    flexWrap: "wrap",
    alignContent: "flex-start",
    minHeight: "58px",
    overflow: "hidden"
  },
  meta: {
    marginTop: "auto",
    paddingTop: "12px",
    display: "grid",
    gridTemplateColumns: "1fr auto",
    gap: "12px",
    alignItems: "center",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`
  },
  time: {
    fontVariantNumeric: "tabular-nums",
    color: tokens.colorBrandForeground1
  },
  rating: {
    color: tokens.colorNeutralForeground2
  }
});

function extractBroadcastTime(airDate: string | null) {
  if (!airDate) {
    return "--:--";
  }

  const match = airDate.match(/(\d{1,2}):(\d{2})/);
  if (!match) {
    return "--:--";
  }

  return `${match[1].padStart(2, "0")}:${match[2]}`;
}

function formatRating(score: number | null) {
  return score == null ? "暂无评分" : score.toFixed(1);
}

export function SubjectCard({ subject }: { subject: SubjectCardModel }) {
  const styles = useStyles();
  const primaryTitle = subject.titleCn || subject.title;
  const secondaryTitle = subject.titleCn && subject.titleCn !== subject.title ? subject.title : null;
  const tags = subject.tags.length > 0 ? subject.tags : ["连载中"];

  return (
    <Link to={`/title/${subject.bangumiSubjectId}`} className={styles.link}>
      <Card className={styles.card} appearance="filled-alternative">
        <div
          className={styles.poster}
          style={{
            backgroundImage: subject.imagePortrait ? `url(${subject.imagePortrait})` : undefined
          }}
        />

        <div className={styles.body}>
          <div className={styles.titleGroup}>
            <Text weight="semibold" className={styles.title}>
              {primaryTitle}
            </Text>
            {secondaryTitle ? (
              <Text block size={300} className={styles.subtitle}>
                {secondaryTitle}
              </Text>
            ) : null}
          </div>

          <div className={styles.tags}>
            {tags.map((tag) => (
              <Badge key={tag} appearance="tint">
                {tag}
              </Badge>
            ))}
          </div>

          <div className={styles.meta}>
            <Text weight="semibold" className={styles.time}>
              {extractBroadcastTime(subject.airDate)}
            </Text>
            <Text size={300} className={styles.rating}>
              {formatRating(subject.ratingScore)}
            </Text>
          </div>
        </div>
      </Card>
    </Link>
  );
}
