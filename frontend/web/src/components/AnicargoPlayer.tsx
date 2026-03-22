import { useEffect, useRef, useState } from "react";

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
  const playerRef = useRef<{ destroy: (removeHtml?: boolean) => void } | null>(null);
  const hasStartedRef = useRef(false);
  const playbackStartRef = useRef(onPlaybackStart);
  const [useNativeVideo, setUseNativeVideo] = useState(false);

  playbackStartRef.current = onPlaybackStart;

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    hasStartedRef.current = false;
    setUseNativeVideo(false);
    container.innerHTML = "";

    let cancelled = false;

    async function mountPlayer() {
      try {
        const { default: Artplayer } = await import("artplayer");
        if (cancelled || !containerRef.current) {
          return;
        }

        const player = new Artplayer({
          container: containerRef.current,
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
      } catch (error) {
        console.error("Failed to initialize Artplayer, falling back to native video", error);
        if (!cancelled) {
          setUseNativeVideo(true);
        }
      }
    }

    void mountPlayer();

    return () => {
      cancelled = true;
      playerRef.current?.destroy(false);
      playerRef.current = null;
    };
  }, [posterUrl, streamUrl, subtitleUrl]);

  if (useNativeVideo) {
    return (
      <video
        className="anicargo-player"
        controls
        poster={posterUrl ?? undefined}
        preload="metadata"
        onPlay={() => {
          if (hasStartedRef.current) {
            return;
          }

          hasStartedRef.current = true;
          playbackStartRef.current?.();
        }}
        style={{ width: "100%", height: "100%", backgroundColor: "#070a10" }}
      >
        <source src={streamUrl} />
        {subtitleUrl ? <track kind="subtitles" src={subtitleUrl} default /> : null}
      </video>
    );
  }

  return (
    <div
      ref={containerRef}
      className="anicargo-player"
      style={{ width: "100%", height: "100%" }}
    />
  );
}
