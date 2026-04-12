import { useEffect, useRef, useState } from "react";
import Artplayer from "artplayer";
import type { Option as ArtplayerOption } from "artplayer/types/option";

type AnicargoPlayerProps = {
  streamUrl: string;
  posterUrl?: string | null;
  subtitleUrl?: string | null;
  onPlaybackStart?: () => void;
};

type SubtitleTrackOption = {
  index: number;
  label: string;
  active: boolean;
};

type PlayerThemeConfig = {
  accent: string;
  background: string;
  fontColor: string;
  widgetBackground: string;
  progressColor: string;
  loadedColor: string;
  subtitleTextColor: string;
  subtitleTextShadow: string;
  subtitleBorder: string;
};

const DEFAULT_PLAYER_THEME: PlayerThemeConfig = {
  accent: "#4b2c23",
  background: "#271813",
  fontColor: "#f5eee8",
  widgetBackground: "rgba(44, 29, 24, 0.92)",
  progressColor: "rgba(255, 255, 255, 0.22)",
  loadedColor: "rgba(255, 255, 255, 0.32)",
  subtitleTextColor: "#f8f4f0",
  subtitleTextShadow: "0 2px 6px rgba(0, 0, 0, 0.88)",
  subtitleBorder: "rgba(0, 0, 0, 0.92)",
};

function collectTextTracks(video: HTMLVideoElement | null) {
  if (!video) {
    return [];
  }

  return Array.from({ length: video.textTracks.length }, (_, index) => video.textTracks[index]).filter(
    (track): track is TextTrack => Boolean(track),
  );
}

function createTrackLabel(track: TextTrack, index: number) {
  const parts = [track.label?.trim(), track.language?.trim(), track.kind?.trim()].filter(Boolean);
  return parts[0] ?? `字幕 ${index + 1}`;
}

function readSubtitleTracks(video: HTMLVideoElement | null): SubtitleTrackOption[] {
  return collectTextTracks(video).map((track, index) => ({
    index,
    label: createTrackLabel(track, index),
    active: track.mode === "showing",
  }));
}

function applySubtitleTrack(video: HTMLVideoElement | null, trackIndex: number | null) {
  for (const [index, track] of collectTextTracks(video).entries()) {
    track.mode = trackIndex != null && index === trackIndex ? "showing" : "disabled";
  }
}

function readThemeVariable(
  styles: CSSStyleDeclaration,
  name: string,
  fallback: string,
) {
  const value = styles.getPropertyValue(name).trim();
  return value || fallback;
}

function readPlayerTheme(): PlayerThemeConfig {
  if (typeof window === "undefined") {
    return DEFAULT_PLAYER_THEME;
  }

  const styles = window.getComputedStyle(document.documentElement);

  return {
    accent: readThemeVariable(styles, "--app-player-theme", DEFAULT_PLAYER_THEME.accent),
    background: readThemeVariable(styles, "--app-player-background", DEFAULT_PLAYER_THEME.background),
    fontColor: readThemeVariable(styles, "--app-player-font", DEFAULT_PLAYER_THEME.fontColor),
    widgetBackground: readThemeVariable(
      styles,
      "--app-player-widget",
      DEFAULT_PLAYER_THEME.widgetBackground,
    ),
    progressColor: readThemeVariable(styles, "--app-player-progress", DEFAULT_PLAYER_THEME.progressColor),
    loadedColor: readThemeVariable(styles, "--app-player-loaded", DEFAULT_PLAYER_THEME.loadedColor),
    subtitleTextColor: readThemeVariable(
      styles,
      "--app-player-subtitle-text",
      DEFAULT_PLAYER_THEME.subtitleTextColor,
    ),
    subtitleTextShadow: readThemeVariable(
      styles,
      "--app-player-subtitle-shadow",
      DEFAULT_PLAYER_THEME.subtitleTextShadow,
    ),
    subtitleBorder: readThemeVariable(
      styles,
      "--app-player-subtitle-border",
      DEFAULT_PLAYER_THEME.subtitleBorder,
    ),
  };
}

export function AnicargoPlayer({
  streamUrl,
  posterUrl,
  subtitleUrl,
  onPlaybackStart,
}: AnicargoPlayerProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const nativeVideoRef = useRef<HTMLVideoElement | null>(null);
  const playerRef = useRef<Artplayer | null>(null);
  const activeVideoRef = useRef<HTMLVideoElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const hasStartedRef = useRef(false);
  const playbackStartRef = useRef(onPlaybackStart);
  const [useNativeVideo, setUseNativeVideo] = useState(false);
  const [subtitleTracks, setSubtitleTracks] = useState<SubtitleTrackOption[]>([]);
  const [isSubtitleMenuOpen, setIsSubtitleMenuOpen] = useState(false);
  const [themeConfig, setThemeConfig] = useState<PlayerThemeConfig>(() => readPlayerTheme());

  playbackStartRef.current = onPlaybackStart;

  function syncSubtitleTracks(video: HTMLVideoElement | null = activeVideoRef.current) {
    const tracks = readSubtitleTracks(video);
    setSubtitleTracks(tracks);
    if (tracks.length === 0) {
      setIsSubtitleMenuOpen(false);
    }
  }

  function bindVideoTracks(video: HTMLVideoElement | null) {
    if (!video) {
      syncSubtitleTracks(null);
      return () => {};
    }

    activeVideoRef.current = video;
    syncSubtitleTracks(video);

    const handleTrackUpdate = () => syncSubtitleTracks(video);
    const handleLoadedMetadata = () => syncSubtitleTracks(video);
    const textTracks = video.textTracks as TextTrackList & EventTarget;

    video.addEventListener("loadedmetadata", handleLoadedMetadata);
    textTracks.addEventListener?.("change", handleTrackUpdate);
    textTracks.addEventListener?.("addtrack", handleTrackUpdate as EventListener);
    textTracks.addEventListener?.("removetrack", handleTrackUpdate as EventListener);

    return () => {
      if (activeVideoRef.current === video) {
        activeVideoRef.current = null;
      }
      video.removeEventListener("loadedmetadata", handleLoadedMetadata);
      textTracks.removeEventListener?.("change", handleTrackUpdate);
      textTracks.removeEventListener?.("addtrack", handleTrackUpdate as EventListener);
      textTracks.removeEventListener?.("removetrack", handleTrackUpdate as EventListener);
    };
  }

  function buildArtplayerOptions(container: HTMLDivElement, minimal = false): ArtplayerOption {
    if (minimal) {
      return {
        container,
        url: streamUrl,
        poster: posterUrl ?? undefined,
        theme: themeConfig.accent,
        volume: 0.8,
        autoplay: false,
        autoSize: true,
        fullscreen: true,
        fullscreenWeb: true,
        playsInline: true,
        moreVideoAttr: {
          preload: "metadata",
          crossOrigin: "anonymous",
          playsInline: true,
        },
      };
    }

    return {
      container,
      url: streamUrl,
      poster: posterUrl ?? undefined,
      theme: themeConfig.accent,
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
              color: themeConfig.subtitleTextColor,
              textShadow: themeConfig.subtitleTextShadow,
            },
          }
        : undefined,
      cssVar: {
        "--art-theme": themeConfig.accent,
        "--art-font-color": themeConfig.fontColor,
        "--art-control-opacity": 0.92,
        "--art-widget-background": themeConfig.widgetBackground,
        "--art-progress-color": themeConfig.progressColor,
        "--art-loaded-color": themeConfig.loadedColor,
        "--art-subtitle-font-size": "18px",
        "--art-subtitle-border": themeConfig.subtitleBorder,
      },
    };
  }

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const updateTheme = () => {
      const nextTheme = readPlayerTheme();
      setThemeConfig((currentTheme) => {
        const currentSignature = Object.values(currentTheme).join("|");
        const nextSignature = Object.values(nextTheme).join("|");
        return currentSignature === nextSignature ? currentTheme : nextTheme;
      });
    };

    updateTheme();

    const observer = new MutationObserver(() => {
      updateTheme();
    });

    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme", "class", "style"],
    });

    const mediaQuery = window.matchMedia?.("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      updateTheme();
    };

    mediaQuery?.addEventListener?.("change", handleSystemThemeChange);

    return () => {
      observer.disconnect();
      mediaQuery?.removeEventListener?.("change", handleSystemThemeChange);
    };
  }, []);

  useEffect(() => {
    const handlePointerDown = (event: PointerEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) {
        setIsSubtitleMenuOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
    };
  }, []);

  useEffect(() => {
    setIsSubtitleMenuOpen(false);
  }, [streamUrl, subtitleUrl]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    hasStartedRef.current = false;
    setUseNativeVideo(false);
    setSubtitleTracks([]);
    container.innerHTML = "";

    let cancelled = false;
    let disposeVideoBinding: () => void = () => {};

    function bindPlayer(player: Artplayer) {
      player.on("play", () => {
        if (hasStartedRef.current) {
          return;
        }

        hasStartedRef.current = true;
        playbackStartRef.current?.();
      });

      player.on("subtitleLoad", () => {
        syncSubtitleTracks(player.video ?? null);
      });

      playerRef.current = player;
      disposeVideoBinding = bindVideoTracks(player.video ?? null);
    }

    function mountPlayer() {
      try {
        if (cancelled || !containerRef.current) {
          return;
        }

        try {
          const player = new Artplayer(buildArtplayerOptions(containerRef.current));
          bindPlayer(player);
          return;
        } catch (primaryError) {
          console.warn("Failed to initialize Artplayer with full config, retrying with minimal config", primaryError);
        }

        try {
          containerRef.current.innerHTML = "";
          const player = new Artplayer(buildArtplayerOptions(containerRef.current, true));
          bindPlayer(player);
          return;
        } catch (fallbackError) {
          console.error("Failed to initialize Artplayer, falling back to native video", fallbackError);
          if (!cancelled) {
            setUseNativeVideo(true);
          }
        }
      } catch (error) {
        console.error("Unexpected player bootstrap failure, falling back to native video", error);
        if (!cancelled) {
          setUseNativeVideo(true);
        }
      }
    }

    mountPlayer();

    return () => {
      cancelled = true;
      disposeVideoBinding();
      playerRef.current?.destroy(false);
      playerRef.current = null;
      activeVideoRef.current = null;
    };
  }, [posterUrl, streamUrl, subtitleUrl, themeConfig]);

  useEffect(() => {
    if (!useNativeVideo) {
      return;
    }

    return bindVideoTracks(nativeVideoRef.current);
  }, [useNativeVideo, posterUrl, streamUrl, subtitleUrl, themeConfig]);

  function handleSubtitleSelect(trackIndex: number | null) {
    applySubtitleTrack(activeVideoRef.current, trackIndex);
    syncSubtitleTracks(activeVideoRef.current);
    setIsSubtitleMenuOpen(false);
  }

  const activeSubtitle = subtitleTracks.find((track) => track.active);

  return (
    <div className="anicargo-player-shell">
      {subtitleTracks.length > 0 ? (
        <div className="anicargo-player-menu" ref={menuRef}>
          <button
            type="button"
            className="anicargo-player-menu__button"
            onClick={() => setIsSubtitleMenuOpen((current) => !current)}
          >
            {activeSubtitle ? `字幕 · ${activeSubtitle.label}` : "字幕 · 关闭"}
          </button>

          {isSubtitleMenuOpen ? (
            <div className="anicargo-player-menu__panel">
              <button
                type="button"
                className={`anicargo-player-menu__item ${activeSubtitle ? "" : "is-active"}`.trim()}
                onClick={() => handleSubtitleSelect(null)}
              >
                关闭字幕
              </button>

              {subtitleTracks.map((track) => (
                <button
                  key={track.index}
                  type="button"
                  className={`anicargo-player-menu__item ${track.active ? "is-active" : ""}`.trim()}
                  onClick={() => handleSubtitleSelect(track.index)}
                >
                  {track.label}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}

      {useNativeVideo ? (
        <video
          ref={nativeVideoRef}
          className="anicargo-player anicargo-player-host"
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
          style={{ width: "100%", height: "100%", backgroundColor: themeConfig.background }}
        >
          <source src={streamUrl} />
          {subtitleUrl ? <track kind="subtitles" src={subtitleUrl} default /> : null}
        </video>
      ) : (
        <div
          ref={containerRef}
          className="anicargo-player anicargo-player-host"
          style={{ width: "100%", height: "100%" }}
        />
      )}
    </div>
  );
}
