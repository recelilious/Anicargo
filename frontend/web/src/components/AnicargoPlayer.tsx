import { useEffect, useRef } from "react";
import Artplayer from "artplayer";

type AnicargoPlayerProps = {
  streamUrl: string;
  posterUrl?: string | null;
  subtitleUrl?: string | null;
  onPlaybackStart?: () => void;
};

export function AnicargoPlayer({
  streamUrl,
  posterUrl,
  subtitleUrl,
  onPlaybackStart,
}: AnicargoPlayerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const playerRef = useRef<Artplayer | null>(null);
  const hasStartedRef = useRef(false);
  const playbackStartRef = useRef(onPlaybackStart);

  playbackStartRef.current = onPlaybackStart;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    hasStartedRef.current = false;
    const player = new Artplayer({
      container,
      url: streamUrl,
      poster: posterUrl ?? undefined,
      theme: "#4b2c23",
      volume: 0.8,
      autoplay: false,
      autoSize: true,
      backdrop: true,
      fullscreen: true,
      fullscreenWeb: true,
      pip: true,
      screenshot: false,
      setting: true,
      playbackRate: true,
      aspectRatio: true,
      subtitleOffset: true,
      miniProgressBar: true,
      mutex: true,
      playsInline: true,
      autoPlayback: true,
      moreVideoAttr: {
        preload: "metadata",
        crossOrigin: "anonymous",
        playsInline: true,
      },
      subtitle: subtitleUrl
        ? {
            url: subtitleUrl,
            type: subtitleUrl.endsWith(".ass")
              ? "ass"
              : subtitleUrl.endsWith(".srt")
                ? "srt"
                : "vtt",
            style: {
              fontFamily: "\"JetBrains Mono Variable\", \"Maple Mono CN\", monospace",
              fontSize: "18px",
              color: "#f8f4f0",
              textShadow: "0 2px 6px rgba(0, 0, 0, 0.88)",
            },
          }
        : undefined,
      cssVar: {
        "--art-theme": "#4b2c23",
        "--art-font-color": "#f5eee8",
        "--art-control-opacity": 0.92,
        "--art-widget-background": "rgba(13, 18, 24, 0.92)",
        "--art-subtitle-font-size": "18px",
      },
    });

    player.on("play", () => {
      if (hasStartedRef.current) {
        return;
      }

      hasStartedRef.current = true;
      playbackStartRef.current?.();
    });

    playerRef.current = player;

    return () => {
      playerRef.current = null;
      player.destroy(false);
    };
  }, [posterUrl, streamUrl, subtitleUrl]);

  return <div ref={containerRef} className="anicargo-player" style={{ width: "100%", height: "100%" }} />;
}
