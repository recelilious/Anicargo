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
    height: "438px",
    display: "grid",
    gridTemplateRows: "252px minmax(0, 1fr)",
    overflow: "hidden",
    backgroundColor: tokens.colorNeutralBackground1,
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  posterWrap: {
    position: "relative",
    overflow: "hidden",
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: "var(--app-fallback-hero)"
  },
  poster: {
    position: "absolute",
    inset: 0,
    backgroundSize: "cover",
    backgroundPosition: "center center"
  },
  status: {
    position: "absolute",
    left: "10px",
    top: "10px",
    zIndex: 1
  },
  tagRail: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: 0,
    zIndex: 1,
    display: "flex",
    flexWrap: "wrap",
    gap: "6px",
    padding: "10px",
    backgroundColor: "rgba(24, 14, 11, 0.70)"
  },
  tag: {
    backgroundColor: "rgba(255, 248, 241, 0.16)",
    color: "#fff7f1"
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
    paddingTop: "10px",
    minHeight: 0
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
    minHeight: 0
  },
  title: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.42",
    overflowWrap: "anywhere",
    wordBreak: "break-word"
  },
  subtitle: {
    color: tokens.colorNeutralForeground3,
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.42",
    overflowWrap: "anywhere",
    wordBreak: "break-word"
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
  rating: {
    color: tokens.colorBrandForeground1,
    fontVariantNumeric: "tabular-nums"
  },
  time: {
    color: tokens.colorNeutralForeground2,
    fontVariantNumeric: "tabular-nums"
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

function formatStatus(status: SubjectCardModel["releaseStatus"]) {
  return status === "completed" ? "已完结" : "放送中";
}

export function SubjectCard({ subject }: { subject: SubjectCardModel }) {
  const styles = useStyles();
  const primaryTitle = subject.titleCn || subject.title;
  const secondaryTitle = subject.titleCn && subject.titleCn !== subject.title ? subject.title : null;

  return (
    <Link to={`/title/${subject.bangumiSubjectId}`} className={styles.link}>
      <Card className={styles.card} appearance="filled-alternative">
        <div className={styles.posterWrap}>
          <div
            className={styles.poster}
            style={{
              backgroundImage: subject.imagePortrait ? `url(${subject.imagePortrait})` : undefined
            }}
          />

          <div className={styles.status}>
            <Badge appearance="filled">{formatStatus(subject.releaseStatus)}</Badge>
          </div>

          {subject.tags.length > 0 ? (
            <div className={styles.tagRail}>
              {subject.tags.slice(0, 8).map((tag) => (
                <Badge key={tag} appearance="outline" className={styles.tag}>
                  {tag}
                </Badge>
              ))}
            </div>
          ) : null}
        </div>

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

          <div className={styles.meta}>
            <Text weight="semibold" className={styles.rating}>
              {formatRating(subject.ratingScore)}
            </Text>
            <Text size={300} className={styles.time}>
              {extractBroadcastTime(subject.airDate)}
            </Text>
          </div>
        </div>
      </Card>
    </Link>
  );
}
