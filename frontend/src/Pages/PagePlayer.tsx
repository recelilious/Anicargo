import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import Hls from "hls.js";
import AppShell from "../Components/AppShell";
import { apiFetch, resolveApiUrl } from "../api";
import { useSession } from "../session";
import "../Styles/PagePlayer.css";

interface MediaEntry {
  id: string;
  filename: string;
  size: number;
}

interface MediaParseInfo {
  title?: string | null;
  episode?: string | null;
  season?: string | null;
  year?: string | null;
  release_group?: string | null;
  resolution?: string | null;
}

interface BangumiSubjectInfo {
  id: number;
  name: string;
  name_cn: string;
  air_date?: string | null;
  total_episodes?: number | null;
}

interface BangumiEpisodeInfo {
  id: number;
  sort: number;
  ep?: number | null;
  name: string;
  name_cn: string;
  air_date?: string | null;
}

interface MediaMatchDetail {
  subject: BangumiSubjectInfo;
  episode?: BangumiEpisodeInfo | null;
  method: string;
  confidence?: number | null;
  reason?: string | null;
}

interface MediaProgressResponse {
  media_id: string;
  position_secs: number;
  duration_secs?: number | null;
}

interface MediaDetailResponse {
  entry: MediaEntry;
  parse?: MediaParseInfo | null;
  matched?: MediaMatchDetail | null;
  progress?: MediaProgressResponse | null;
}

interface StreamReadyResponse {
  id: string;
  playlist_url: string;
}

interface StreamQueuedResponse {
  status: string;
  job_id: number;
}

interface JobStatusResponse {
  job: {
    id: number;
    job_type: string;
    status: string;
    attempts: number;
    max_attempts: number;
    result?: unknown;
    last_error?: string | null;
  };
}

interface ProgressListResponse {
  items: Array<{
    media_id: string;
    filename: string;
    position_secs: number;
    duration_secs?: number | null;
    updated_at: string;
  }>;
}

function formatBytes(value: number): string {
  if (!Number.isFinite(value)) return "--";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let remaining = Math.max(0, value);
  let index = 0;
  while (remaining >= 1024 && index < units.length - 1) {
    remaining /= 1024;
    index += 1;
  }
  const precision = remaining >= 100 ? 0 : remaining >= 10 ? 1 : 2;
  return `${remaining.toFixed(precision)} ${units[index]}`;
}

function formatDuration(value?: number | null): string {
  if (!value || !Number.isFinite(value)) return "--";
  const total = Math.max(0, Math.floor(value));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds
      .toString()
      .padStart(2, "0")}`;
  }
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export default function PagePlayer() {
  const { id } = useParams();
  const { session } = useSession();
  const navigate = useNavigate();
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const lastProgressSent = useRef<number>(0);

  const [detail, setDetail] = useState<MediaDetailResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [streamState, setStreamState] = useState<"idle" | "loading" | "queued" | "ready" | "error">(
    "idle"
  );
  const [playlistUrl, setPlaylistUrl] = useState<string | null>(null);
  const [jobId, setJobId] = useState<number | null>(null);
  const [streamError, setStreamError] = useState<string | null>(null);
  const [recent, setRecent] = useState<ProgressListResponse["items"]>([]);

  const resumeAt = detail?.progress?.position_secs ?? 0;

  const headerSubtitle = useMemo(() => {
    if (!detail) return "Select a file to start playback.";
    return detail.entry.filename;
  }, [detail]);

  useEffect(() => {
    if (!session || !id) return;
    setPlaylistUrl(null);
    setJobId(null);
    setStreamState("idle");
    setStreamError(null);
    setDetail(null);
    setLoading(true);
    apiFetch<MediaDetailResponse>(`/api/media/${id}`, {}, session.token)
      .then((data) => setDetail(data))
      .catch((err) => {
        setStreamError((err as Error).message || "Failed to load media.");
      })
      .finally(() => setLoading(false));
  }, [id, session]);

  useEffect(() => {
    if (!session) return;
    apiFetch<ProgressListResponse>("/api/progress?limit=6", {}, session.token)
      .then((data) => setRecent(data.items))
      .catch(() => {
        setRecent([]);
      });
  }, [session, id]);

  useEffect(() => {
    if (!playlistUrl || !videoRef.current) return;
    const video = videoRef.current;
    let hls: Hls | null = null;

    if (Hls.isSupported()) {
      hls = new Hls();
      hls.loadSource(resolveApiUrl(playlistUrl));
      hls.attachMedia(video);
    } else {
      video.src = resolveApiUrl(playlistUrl);
    }

    return () => {
      if (hls) {
        hls.destroy();
      } else {
        video.removeAttribute("src");
        video.load();
      }
    };
  }, [playlistUrl]);

  async function requestStream() {
    if (!session || !id) return;
    setStreamState("loading");
    setStreamError(null);
    try {
      const response = await fetch(resolveApiUrl(`/api/stream/${id}`), {
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${session.token}`
        }
      });
      const payload = await response.json();
      if (!response.ok) {
        const message =
          typeof payload?.error === "string" ? payload.error : "Failed to prepare stream.";
        setStreamState("error");
        setStreamError(message);
        return;
      }
      if (response.status === 202) {
        const queued = payload as StreamQueuedResponse;
        setJobId(queued.job_id);
        setStreamState("queued");
        return;
      }
      const ready = payload as StreamReadyResponse;
      setPlaylistUrl(ready.playlist_url);
      setStreamState("ready");
    } catch (err) {
      setStreamState("error");
      setStreamError((err as Error).message || "Failed to prepare stream.");
    }
  }

  async function checkJob() {
    if (!session || !jobId) return;
    try {
      const status = await apiFetch<JobStatusResponse>(`/api/jobs/${jobId}`, {}, session.token);
      if (status.job.status === "done") {
        await requestStream();
      } else if (status.job.status === "failed") {
        setStreamState("error");
        setStreamError(status.job.last_error || "Stream job failed.");
      }
    } catch (err) {
      setStreamError((err as Error).message || "Failed to check job.");
    }
  }

  useEffect(() => {
    if (streamState !== "queued" || !jobId) return;
    const timer = window.setInterval(() => {
      checkJob();
    }, 2500);
    return () => window.clearInterval(timer);
  }, [streamState, jobId]);

  async function saveProgress() {
    if (!session || !id || !videoRef.current) return;
    const now = Date.now();
    if (now - lastProgressSent.current < 5000) return;
    lastProgressSent.current = now;
    const position = videoRef.current.currentTime;
    const duration = Number.isFinite(videoRef.current.duration)
      ? videoRef.current.duration
      : undefined;
    try {
      await apiFetch<MediaProgressResponse>(
        `/api/progress/${id}`,
        {
          method: "PUT",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({
            position_secs: position,
            duration_secs: duration
          })
        },
        session.token
      );
    } catch {
      // Ignore transient progress errors
    }
  }

  function handleLoadedMetadata() {
    if (!videoRef.current) return;
    if (resumeAt > 1 && resumeAt < videoRef.current.duration) {
      videoRef.current.currentTime = resumeAt;
    }
  }

  if (!id) {
    return (
      <AppShell title="Player" subtitle="Choose a file from the library.">
        <div className="app-card">
          <p className="app-muted">No media selected.</p>
        </div>
      </AppShell>
    );
  }

  return (
    <AppShell title="Player" subtitle={headerSubtitle}>
      {streamError ? <div className="player-error">{streamError}</div> : null}

      <div className="player-layout">
        <div className="app-card player-video-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Playback</h2>
            <span className="app-pill">{streamState}</span>
          </div>
          <div className="player-video-wrapper">
            {playlistUrl ? (
              <video
                ref={videoRef}
                controls
                onLoadedMetadata={handleLoadedMetadata}
                onTimeUpdate={saveProgress}
                onPause={saveProgress}
                onEnded={saveProgress}
              />
            ) : (
              <div className="player-placeholder">
                {loading ? "Loading..." : "Stream not ready yet."}
              </div>
            )}
          </div>
          <div className="player-actions">
            <button
              type="button"
              className="app-btn primary"
              onClick={requestStream}
              disabled={streamState === "loading"}
            >
              Prepare stream
            </button>
            {streamState === "queued" ? (
              <span className="app-muted">Preparing stream...</span>
            ) : null}
            {detail?.entry ? (
              <span className="app-muted">
                Size: {formatBytes(detail.entry.size)}
              </span>
            ) : null}
          </div>
        </div>

        <div className="app-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Info</h2>
          </div>
          {!detail ? (
            <div className="app-muted">Loading media info...</div>
          ) : (
            <div className="player-info">
              <div>
                <h3>Parse</h3>
                <div className="player-row">
                  <span>Title</span>
                  <span>{detail.parse?.title ?? "--"}</span>
                </div>
                <div className="player-row">
                  <span>Episode</span>
                  <span>{detail.parse?.episode ?? "--"}</span>
                </div>
                <div className="player-row">
                  <span>Year</span>
                  <span>{detail.parse?.year ?? "--"}</span>
                </div>
                <div className="player-row">
                  <span>Resolution</span>
                  <span>{detail.parse?.resolution ?? "--"}</span>
                </div>
              </div>

              <div>
                <h3>Match</h3>
                {detail.matched ? (
                  <>
                    <div className="player-row">
                      <span>Subject</span>
                      <span>{detail.matched.subject.name}</span>
                    </div>
                    <div className="player-row">
                      <span>Chinese</span>
                      <span>{detail.matched.subject.name_cn}</span>
                    </div>
                    <div className="player-row">
                      <span>Method</span>
                      <span>{detail.matched.method}</span>
                    </div>
                  </>
                ) : (
                  <div className="app-muted">No match data.</div>
                )}
              </div>

              <div>
                <h3>Progress</h3>
                <div className="player-row">
                  <span>Resume at</span>
                  <span>{formatDuration(resumeAt)}</span>
                </div>
                <div className="player-row">
                  <span>Duration</span>
                  <span>{formatDuration(detail.progress?.duration_secs ?? null)}</span>
                </div>
              </div>

              <div>
                <h3>Recent</h3>
                {recent.length ? (
                  <div className="player-recent">
                    {recent.map((item) => (
                      <button
                        key={item.media_id}
                        type="button"
                        className="player-recent-item"
                        onClick={() => navigate(`/player/${item.media_id}`)}
                      >
                        <span>{item.filename}</span>
                        <span>
                          {formatDuration(item.position_secs)} /{" "}
                          {formatDuration(item.duration_secs ?? null)}
                        </span>
                      </button>
                    ))}
                  </div>
                ) : (
                  <div className="app-muted">No recent playback.</div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </AppShell>
  );
}
