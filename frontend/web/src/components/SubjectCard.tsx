import { useEffect, useState } from "react";
import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import { fetchSubjectDetail } from "../api";
import { useSession } from "../session";
import type { SubjectCard as SubjectCardModel } from "../types";

const useStyles = makeStyles({
  link: {
    textDecorationLine: "none",
    color: "inherit",
    height: "100%"
  },
  card: {
    height: "414px",
    display: "grid",
    gridTemplateRows: "238px minmax(0, 1fr)",
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
    gap: "6px",
    paddingTop: "8px",
    minHeight: 0
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
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
    paddingTop: "10px",
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

const detailTagCache = new Map<number, string[]>();
const detailTagRequests = new Map<number, Promise<string[]>>();

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
  const { deviceId, userToken } = useSession();
  const [tags, setTags] = useState(() => subject.tags.slice(0, 8));
  const primaryTitle = subject.titleCn || subject.title;
  const secondaryTitle = subject.titleCn && subject.titleCn !== subject.title ? subject.title : null;
  const displayedTags = tags.slice(0, 8);

  useEffect(() => {
    const nextTags = subject.tags.slice(0, 8);
    setTags(nextTags);

    if (nextTags.length > 0) {
      detailTagCache.set(subject.bangumiSubjectId, nextTags);
      return;
    }

    const cachedTags = detailTagCache.get(subject.bangumiSubjectId);
    if (cachedTags && cachedTags.length > 0) {
      setTags(cachedTags);
      return;
    }

    let cancelled = false;
    let request = detailTagRequests.get(subject.bangumiSubjectId);

    if (!request) {
      request = fetchSubjectDetail(subject.bangumiSubjectId, deviceId, userToken)
        .then((response) => {
          const resolvedTags = response.subject.tags.slice(0, 8);
          detailTagCache.set(subject.bangumiSubjectId, resolvedTags);
          detailTagRequests.delete(subject.bangumiSubjectId);
          return resolvedTags;
        })
        .catch((error) => {
          detailTagRequests.delete(subject.bangumiSubjectId);
          throw error;
        });

      detailTagRequests.set(subject.bangumiSubjectId, request);
    }

    void request
      .then((resolvedTags) => {
        if (!cancelled && resolvedTags.length > 0) {
          setTags(resolvedTags);
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [deviceId, subject.bangumiSubjectId, subject.tags, userToken]);

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

          {displayedTags.length > 0 ? (
            <div className={styles.tagRail}>
              {displayedTags.map((tag) => (
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
