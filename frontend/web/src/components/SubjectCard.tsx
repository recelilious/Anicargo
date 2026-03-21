import { useEffect, useRef, useState } from "react";
import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import { fetchSubjectDetail } from "../api";
import { useSession } from "../session";
import type { SubjectCard as SubjectCardModel } from "../types";

type SubjectCardMetaVariant = "schedule" | "catalog";

const useStyles = makeStyles({
  link: {
    display: "block",
    textDecorationLine: "none",
    color: "inherit",
    height: "100%",
    perspective: "1200px",
  },
  card: {
    height: "414px",
    display: "grid",
    gridTemplateRows: "238px minmax(0, 1fr)",
    overflow: "hidden",
    backgroundColor: tokens.colorNeutralBackground1,
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    transform:
      "perspective(1000px) translateY(var(--card-lift, 0px)) rotateX(var(--card-rotate-x, 0deg)) rotateY(var(--card-rotate-y, 0deg))",
    transformStyle: "preserve-3d",
    transition: "transform 180ms ease, box-shadow 180ms ease",
    willChange: "transform",
    cursor: "pointer",
  },
  posterWrap: {
    position: "relative",
    overflow: "hidden",
    borderRadius: tokens.borderRadiusLarge,
    backgroundColor: "var(--app-fallback-hero)",
  },
  poster: {
    position: "absolute",
    inset: 0,
    backgroundSize: "cover",
    backgroundPosition: "center center",
    transform: "scale(var(--poster-scale, 1))",
    transition: "transform 180ms ease",
  },
  status: {
    position: "absolute",
    left: "10px",
    top: "10px",
    zIndex: 1,
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
    backgroundColor: "rgba(24, 14, 11, 0.70)",
  },
  tag: {
    backgroundColor: "rgba(255, 248, 241, 0.16)",
    color: "#fff7f1",
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    padding: "8px 12px 12px",
    minHeight: 0,
  },
  titleGroup: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    minHeight: 0,
  },
  title: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.42",
    overflowWrap: "anywhere",
    wordBreak: "break-word",
  },
  subtitle: {
    color: tokens.colorNeutralForeground3,
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.42",
    overflowWrap: "anywhere",
    wordBreak: "break-word",
  },
  meta: {
    marginTop: "auto",
    paddingTop: "10px",
    paddingInline: "8px",
    display: "flex",
    gap: "12px",
    alignItems: "center",
    justifyContent: "space-between",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  metaRatingOnly: {
    justifyContent: "flex-end",
  },
  rating: {
    color: tokens.colorBrandForeground1,
    fontVariantNumeric: "tabular-nums",
  },
  metaValue: {
    color: tokens.colorNeutralForeground2,
    fontVariantNumeric: "tabular-nums",
  },
});

const detailTagCache = new Map<number, string[]>();
const detailTagRequests = new Map<number, Promise<string[]>>();

function prefersReducedMotion() {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function formatRating(score: number | null) {
  return score == null ? "暂无评分" : score.toFixed(1);
}

function formatStatus(status: SubjectCardModel["releaseStatus"]) {
  return status === "completed" ? "已完结" : "放送中";
}

function extractCatalogYear(airDate: string | null) {
  const year = airDate?.match(/\d{4}/)?.[0];
  return year ?? null;
}

function resolveMetaValue(subject: SubjectCardModel, variant: SubjectCardMetaVariant) {
  if (variant === "schedule") {
    return subject.broadcastTime?.trim() || "--:--";
  }

  return extractCatalogYear(subject.airDate);
}

export function SubjectCard({
  subject,
  metaVariant = "schedule",
}: {
  subject: SubjectCardModel;
  metaVariant?: SubjectCardMetaVariant;
}) {
  const styles = useStyles();
  const { deviceId, userToken } = useSession();
  const linkRef = useRef<HTMLAnchorElement | null>(null);
  const [tags, setTags] = useState(() => subject.tags.slice(0, 8));
  const primaryTitle = subject.titleCn || subject.title;
  const secondaryTitle = subject.titleCn && subject.titleCn !== subject.title ? subject.title : null;
  const displayedTags = tags.slice(0, 8);
  const metaValue = resolveMetaValue(subject, metaVariant);

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

  function resetHoverMotion() {
    const link = linkRef.current;
    if (!link) {
      return;
    }

    link.style.setProperty("--card-lift", "0px");
    link.style.setProperty("--card-rotate-x", "0deg");
    link.style.setProperty("--card-rotate-y", "0deg");
    link.style.setProperty("--poster-scale", "1");
  }

  function handleMouseEnter() {
    if (prefersReducedMotion()) {
      return;
    }

    const link = linkRef.current;
    if (!link) {
      return;
    }

    link.style.setProperty("--card-lift", "-8px");
    link.style.setProperty("--poster-scale", "1.035");
  }

  function handleMouseMove(event: React.MouseEvent<HTMLAnchorElement>) {
    if (prefersReducedMotion()) {
      return;
    }

    const link = linkRef.current;
    if (!link) {
      return;
    }

    const rect = link.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    const rotateY = (x / rect.width - 0.5) * 7;
    const rotateX = (0.5 - y / rect.height) * 7;

    link.style.setProperty("--card-lift", "-8px");
    link.style.setProperty("--card-rotate-x", `${rotateX.toFixed(2)}deg`);
    link.style.setProperty("--card-rotate-y", `${rotateY.toFixed(2)}deg`);
    link.style.setProperty("--poster-scale", "1.035");
  }

  return (
    <Link
      ref={linkRef}
      to={`/title/${subject.bangumiSubjectId}`}
      className={styles.link}
      onMouseEnter={handleMouseEnter}
      onMouseMove={handleMouseMove}
      onMouseLeave={resetHoverMotion}
      onBlur={resetHoverMotion}
    >
      <Card className={styles.card} appearance="filled-alternative">
        <div className={styles.posterWrap}>
          <div
            className={styles.poster}
            style={{
              backgroundImage: subject.imagePortrait ? `url(${subject.imagePortrait})` : undefined,
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

          <div className={`${styles.meta} ${metaValue ? "" : styles.metaRatingOnly}`.trim()}>
            {metaValue ? (
              <Text size={300} className={styles.metaValue}>
                {metaValue}
              </Text>
            ) : null}
            <Text weight="semibold" className={styles.rating}>
              {formatRating(subject.ratingScore)}
            </Text>
          </div>
        </div>
      </Card>
    </Link>
  );
}
