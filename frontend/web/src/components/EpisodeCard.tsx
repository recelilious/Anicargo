import { Badge, Card, Text, makeStyles, tokens } from "@fluentui/react-components";
import { Link } from "react-router-dom";

import type { Episode } from "../types";

const useStyles = makeStyles({
  link: {
    textDecorationLine: "none",
    color: "inherit"
  },
  card: {
    gap: "8px"
  },
  muted: {
    color: tokens.colorNeutralForeground3
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
      <Card className={styles.card} appearance="outline">
        <div>
          <Text weight="semibold">
            第 {episode.episodeNumber ?? episode.sort} 集
          </Text>
          <Text block size={300}>
            {episode.titleCn || episode.title || "未命名剧集"}
          </Text>
        </div>

        <div>
          <Badge appearance={episode.isAvailable ? "filled" : "outline"}>
            {episode.isAvailable ? "可播放" : "待入库"}
          </Badge>
        </div>

        <Text size={300} className={styles.muted}>
          {episode.availabilityNote ?? "资源状态未知。"}
        </Text>
      </Card>
    </Link>
  );
}
