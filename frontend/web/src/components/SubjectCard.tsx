import { useEffect, useRef, useState, type CSSProperties, type MouseEvent as ReactMouseEvent } from "react";
import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link, useLocation } from "react-router-dom";

import { fetchSubjectDetail } from "../api";
import { buildRoutePath, rememberReturnTarget, type RouteState } from "../navigation";
import { useSession } from "../session";
import type { SubjectCard as SubjectCardModel } from "../types";

type SubjectCardMetaVariant = "schedule" | "catalog" | "preview";

const useStyles = makeStyles({
  link: {
    display: "block",
    height: "100%",
    color: "inherit",
    textDecorationLine: "none",
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
    backgroundColor: "rgba(24, 14, 11, 0.7)",
    transform: "translateY(var(--tag-translate-y, 0%))",
    opacity: "var(--tag-opacity, 1)",
    transition:
      "transform var(--tag-transition-duration, 220ms) cubic-bezier(0.22, 1, 0.36, 1), opacity var(--tag-transition-duration, 220ms) ease",
    willChange: "transform, opacity",
  },
  tag: {
    display: "inline-flex",
    flex: "0 1 auto",
    minWidth: 0,
    maxWidth: "100%",
    backgroundColor: "rgba(255, 248, 241, 0.16)",
    color: "#fff7f1",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  body: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minHeight: 0,
    padding: "8px 12px 12px",
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
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    paddingTop: "10px",
    paddingInline: "8px",
    borderTop: `1px solid ${tokens.colorNeutralStroke2}`,
  },
  metaSoloLeft: {
    justifyContent: "flex-start",
  },
  metaSoloRight: {
    justifyContent: "flex-end",
  },
  metaValue: {
    minWidth: 0,
    color: tokens.colorNeutralForeground2,
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  rating: {
    color: tokens.colorBrandForeground1,
    fontVariantNumeric: "tabular-nums",
    whiteSpace: "nowrap",
  },
});

const detailTagCache = new Map<number, string[]>();
const detailTagRequests = new Map<number, Promise<string[]>>();

function prefersReducedMotion() {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function formatRating(score: number | null) {
  return score == null ? "\u6682\u65e0\u8bc4\u5206" : score.toFixed(1);
}

function formatStatus(status: SubjectCardModel["releaseStatus"]) {
  switch (status) {
    case "airing":
      return "\u653e\u9001\u4e2d";
    case "upcoming":
      return "\u672a\u64ad\u51fa";
    default:
      return "\u5df2\u5b8c\u7ed3";
  }
}

function extractCatalogYear(airDate: string | null) {
  const year = airDate?.match(/\d{4}/)?.[0];
  return year ?? null;
}

function normalizeCompactType(value: string) {
  const normalized = value.trim();
  const compact = normalized.replace(/\s+/g, "").toLowerCase();

  if (!compact) {
    return null;
  }

  if (
    compact.includes("\u884d\u751f") ||
    compact.includes("\u5409\u7965\u7269") ||
    compact.includes("spinoff") ||
    compact.includes("spin-off")
  ) {
    return "\u884d\u751f";
  }

  if (compact.includes("\u6f2b\u6539") || compact.includes("\u6f2b\u753b\u6539")) {
    return "\u6f2b\u6539";
  }

  if (
    compact.includes("\u8f7b\u5c0f\u8bf4\u6539") ||
    compact.includes("\u5c0f\u8bf4\u6539")
  ) {
    return "\u5c0f\u8bf4\u6539";
  }

  if (compact.includes("\u6e38\u620f\u6539")) {
    return "\u6e38\u620f\u6539";
  }

  if (compact.includes("\u539f\u521b")) {
    return "\u539f\u521b";
  }

  if (compact.includes("movie") || compact.includes("\u5267\u573a")) {
    return "\u5267\u573a\u7248";
  }

  if (compact.includes("ova")) {
    return "OVA";
  }

  if (compact.includes("oad")) {
    return "OAD";
  }

  if (compact === "sp" || compact.includes("special")) {
    return "SP";
  }

  if (compact.includes("web")) {
    return "WEB";
  }

  return normalized;
}

function inferTypeLabel(subject: SubjectCardModel) {
  const catalogType = subject.catalogLabel?.trim().split(/\s+/)[0];
  const catalogMapped = catalogType ? normalizeCompactType(catalogType) : null;
  if (catalogMapped) {
    return catalogMapped;
  }

  for (const tag of subject.tags) {
    const mapped = normalizeCompactType(tag);
    if (mapped) {
      return mapped;
    }
  }

  return null;
}

function extractCatalogTiming(label: string | null) {
  const value = label?.trim();
  if (!value) {
    return null;
  }

  const parts = value.split(/\s+/);
  if (parts.length <= 1) {
    return value;
  }

  return parts[1] ?? null;
}

function resolveMeta(subject: SubjectCardModel, variant: SubjectCardMetaVariant) {
  if (variant === "schedule") {
    return {
      left: inferTypeLabel(subject),
      right: subject.broadcastTime?.trim() || null,
      rating: null as string | null,
    };
  }

  if (variant === "preview") {
    return {
      left: inferTypeLabel(subject),
      right: extractCatalogTiming(subject.catalogLabel) || extractCatalogYear(subject.airDate),
      rating: null as string | null,
    };
  }

  return {
    left: subject.catalogLabel?.trim() || extractCatalogYear(subject.airDate),
    right: null as string | null,
    rating: formatRating(subject.ratingScore),
  };
}

export function SubjectCard({
  subject,
  metaVariant = "schedule",
}: {
  subject: SubjectCardModel;
  metaVariant?: SubjectCardMetaVariant;
}) {
  const styles = useStyles();
  const location = useLocation();
  const { deviceId, userToken } = useSession();
  const linkRef = useRef<HTMLAnchorElement | null>(null);
  const frameRef = useRef<number | null>(null);
  const tagRevealFrameRef = useRef<number | null>(null);
  const pendingMotionRef = useRef<{ rotateX: number; rotateY: number } | null>(null);
  const [tags, setTags] = useState(() => subject.tags.slice(0, 8));
  const [isHovering, setIsHovering] = useState(false);
  const [isTagRailReady, setIsTagRailReady] = useState(() => prefersReducedMotion());
  const primaryTitle = subject.titleCn || subject.title;
  const secondaryTitle = subject.titleCn && subject.titleCn !== subject.title ? subject.title : null;
  const displayedTags = tags.slice(0, 8);
  const displayedTagsKey = displayedTags.join("|");
  const meta = resolveMeta(subject, metaVariant);
  const fromPath = buildRoutePath(location);
  const isLinkedCard = subject.bangumiSubjectId > 0;

  useEffect(() => {
    if (!isLinkedCard) {
      return;
    }

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
  }, [deviceId, isLinkedCard, subject.bangumiSubjectId, subject.tags, userToken]);

  useEffect(() => {
    return () => {
      if (frameRef.current != null) {
        window.cancelAnimationFrame(frameRef.current);
      }

      if (tagRevealFrameRef.current != null) {
        window.cancelAnimationFrame(tagRevealFrameRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (tagRevealFrameRef.current != null) {
      window.cancelAnimationFrame(tagRevealFrameRef.current);
      tagRevealFrameRef.current = null;
    }

    if (displayedTags.length === 0) {
      setIsTagRailReady(false);
      return;
    }

    if (prefersReducedMotion()) {
      setIsTagRailReady(true);
      return;
    }

    setIsTagRailReady(false);
    tagRevealFrameRef.current = window.requestAnimationFrame(() => {
      tagRevealFrameRef.current = null;
      setIsTagRailReady(true);
    });
  }, [displayedTags.length, displayedTagsKey]);

  function applyMotion(rotateX: number, rotateY: number) {
    const link = linkRef.current;
    if (!link) {
      return;
    }

    link.style.setProperty("--card-lift", "-8px");
    link.style.setProperty("--card-rotate-x", `${rotateX.toFixed(2)}deg`);
    link.style.setProperty("--card-rotate-y", `${rotateY.toFixed(2)}deg`);
    link.style.setProperty("--poster-scale", "1.035");
  }

  function resetHoverMotion() {
    setIsHovering(false);
    const link = linkRef.current;
    if (!link) {
      return;
    }

    if (frameRef.current != null) {
      window.cancelAnimationFrame(frameRef.current);
      frameRef.current = null;
    }

    pendingMotionRef.current = null;
    link.style.setProperty("--card-lift", "0px");
    link.style.setProperty("--card-rotate-x", "0deg");
    link.style.setProperty("--card-rotate-y", "0deg");
    link.style.setProperty("--poster-scale", "1");
  }

  function handleMouseEnter() {
    setIsHovering(true);
    if (prefersReducedMotion()) {
      return;
    }

    applyMotion(0, 0);
  }

  function handleMouseMove(event: ReactMouseEvent<HTMLAnchorElement>) {
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

    pendingMotionRef.current = { rotateX, rotateY };

    if (frameRef.current != null) {
      return;
    }

    frameRef.current = window.requestAnimationFrame(() => {
      frameRef.current = null;
      const motion = pendingMotionRef.current;
      if (!motion) {
        return;
      }

      applyMotion(motion.rotateX, motion.rotateY);
    });
  }

  function handleCardClick() {
    const scrollTop = document.getElementById("app-scroll-root")?.scrollTop ?? 0;
    rememberReturnTarget(fromPath, scrollTop);
  }

  const isTagRailShown = displayedTags.length > 0 && isTagRailReady && !isHovering;
  const tagRailStyle = {
    "--tag-translate-y": isTagRailShown ? "0%" : "105%",
    "--tag-opacity": isTagRailShown ? "1" : "0",
    "--tag-transition-duration": prefersReducedMotion() ? "0ms" : "220ms",
  } as CSSProperties;

  const metaClassName = [
    styles.meta,
    meta.left && !meta.right && !meta.rating ? styles.metaSoloLeft : "",
    !meta.left && (meta.right || meta.rating) ? styles.metaSoloRight : "",
  ]
    .filter(Boolean)
    .join(" ");

  const cardContent = (
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
          <div className={styles.tagRail} style={tagRailStyle}>
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

        <div className={metaClassName}>
          {meta.left ? (
            <Text size={300} className={styles.metaValue}>
              {meta.left}
            </Text>
          ) : null}
          {meta.right ? (
            <Text size={300} className={styles.metaValue}>
              {meta.right}
            </Text>
          ) : null}
          {meta.rating ? (
            <Text weight="semibold" className={styles.rating}>
              {meta.rating}
            </Text>
          ) : null}
        </div>
      </div>
    </Card>
  );

  if (!isLinkedCard) {
    return <div className={styles.link}>{cardContent}</div>;
  }

  return (
    <Link
      ref={linkRef}
      to={`/title/${subject.bangumiSubjectId}`}
      state={{ fromPath } satisfies RouteState}
      className={styles.link}
      onClick={handleCardClick}
      onMouseEnter={handleMouseEnter}
      onMouseMove={handleMouseMove}
      onMouseLeave={resetHoverMotion}
      onBlur={resetHoverMotion}
    >
      {cardContent}
    </Link>
  );
}
