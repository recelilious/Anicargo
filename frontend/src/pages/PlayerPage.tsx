import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import Hls from "hls.js";
import { apiFetch, resolveApiUrl } from "../api";
import { useSession } from "../session";

interface StreamResponse {
  id: string;
  playlist_url: string;
}

interface MediaEntry {
  id: string;
  filename: string;
}

export default function PlayerPage() {
  const { session } = useSession();
  const { id } = useParams();
  const [playlistUrl, setPlaylistUrl] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [title, setTitle] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement | null>(null);

  useEffect(() => {
    setPlaylistUrl(null);
    setStatus(null);
    setTitle(null);

    if (!id || !session?.token) {
      return;
    }

    async function fetchStream() {
      try {
        const response = await apiFetch<StreamResponse>(
          `/api/stream/${id}`,
          {},
          session.token
        );
        setPlaylistUrl(resolveApiUrl(response.playlist_url));
      } catch (error) {
        setStatus((error as Error).message);
      }
    }

    async function fetchTitle() {
      try {
        const entries = await apiFetch<MediaEntry[]>(
          "/api/library",
          {},
          session.token
        );
        const entry = entries.find((item) => item.id === id);
        setTitle(entry?.filename ?? null);
      } catch {
        setTitle(null);
      }
    }

    void fetchStream();
    void fetchTitle();
  }, [id, session?.token]);

  useEffect(() => {
    if (!playlistUrl || !videoRef.current) {
      return;
    }

    const video = videoRef.current;
    let hls: Hls | null = null;

    if (video.canPlayType("application/vnd.apple.mpegurl")) {
      video.src = playlistUrl;
    } else if (Hls.isSupported()) {
      hls = new Hls({ enableWorker: true });
      hls.loadSource(playlistUrl);
      hls.attachMedia(video);
    } else {
      setStatus("This browser cannot play HLS. Try mpv or Safari.");
    }

    return () => {
      if (hls) {
        hls.destroy();
      }
    };
  }, [playlistUrl]);

  return (
    <div className="grid">
      <section className="panel span-8">
        <div className="panel-header">
          <h2>Player</h2>
          <span className="selected">{title ?? id ?? "No selection"}</span>
        </div>
        <div className="video-shell">
          <video ref={videoRef} controls playsInline />
          {!playlistUrl ? (
            <div className="video-overlay">
              <div>
                <p>Select a title from the library.</p>
                <span>HLS stream will appear here.</span>
              </div>
            </div>
          ) : null}
        </div>
        <div className="player-actions">
          <button
            className="ghost"
            type="button"
            disabled={!playlistUrl}
            onClick={() => {
              if (playlistUrl) {
                navigator.clipboard.writeText(playlistUrl).catch(() => {
                  setStatus("Copy failed. Copy manually from the player URL.");
                });
              }
            }}
          >
            Copy stream URL
          </button>
          <span className="hint">
            {playlistUrl ? "Use mpv/VLC if the browser fails." : ""}
          </span>
        </div>
        {status ? <p className="status-line">{status}</p> : null}
      </section>

      <section className="panel span-4">
        <h2>Session</h2>
        <p className="subtitle">
          Tokens are required for every playlist and segment. The player URL is
          already authorized.
        </p>
        <div className="stack">
          <div className="stack-item">
            <h3>Need a title?</h3>
            <p>
              Browse the <Link to="/library">library</Link> to pick another file.
            </p>
          </div>
          <div className="stack-item">
            <h3>External player</h3>
            <p>Paste the copied URL into mpv or VLC.</p>
          </div>
          <div className="stack-item">
            <h3>Protected</h3>
            <p>Segment URLs inherit the token from the playlist path.</p>
          </div>
        </div>
      </section>
    </div>
  );
}
