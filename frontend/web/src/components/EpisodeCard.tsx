import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import type { Episode } from "../types";

const useStyles = makeStyles({
  link: {
    textDecorationLine: "none",
    color: "inherit"
  },
  card: {
    gap: "8px",
    backgroundColor: "var(--app-surface-1)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)"
  },
  muted: {
    color: tokens.colorNeutralForeground3
  },
  title: {
    display: "-webkit-box",
    overflow: "hidden",
    WebkitBoxOrient: "vertical",
    WebkitLineClamp: "2",
    lineHeight: "1.4",
    overflowWrap: "anywhere",
    wordBreak: "break-word"
  }
});

export function EpisodeCard({
  episode,
  subjectId
}: {
  episode: Episode;
  subjectId: number;
}) {
  const styles = useStyles();

  return (
    <Link to={`/watch/${subjectId}/${episode.bangumiEpisodeId}`} className={styles.link}>
      <Card className={styles.card} appearance="filled-alternative">
        <div>
          <Text weight="semibold">第 {episode.episodeNumber ?? episode.sort} 集</Text>
          <Text block size={300} className={styles.title}>
            {episode.titleCn || episode.title || "未命名剧集"}
          </Text>
        </div>

        <div>
          <Badge appearance={episode.isAvailable ? "filled" : "outline"}>
            {episode.isAvailable ? "可播放" : "待入库"}
          </Badge>
        </div>

        <Text size={300} className={styles.muted}>
          {episode.availabilityNote ?? "状态未知"}
        </Text>
      </Card>
    </Link>
  );
}
