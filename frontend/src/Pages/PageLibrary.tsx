import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import AppShell from "../Components/AppShell";
import { apiFetch } from "../api";
import { useSession } from "../session";
import "../Styles/PageLibrary.css";

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

interface MatchCandidate {
  subject_id: number;
  confidence: number;
  reason: string;
  name: string;
  name_cn: string;
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

export default function PageLibrary() {
  const { session } = useSession();
  const navigate = useNavigate();
  const [items, setItems] = useState<MediaEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MediaDetailResponse | null>(null);
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);
  const [candidateLoading, setCandidateLoading] = useState(false);
  const [manualSubjectId, setManualSubjectId] = useState<number | null>(null);
  const [manualEpisodeId, setManualEpisodeId] = useState("");
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const isAdmin = (session?.roleLevel ?? 0) >= 3;

  const filteredItems = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) => item.filename.toLowerCase().includes(needle));
  }, [items, query]);

  async function loadLibrary(refresh = false) {
    if (!session) return;
    setLoading(true);
    setError(null);
    try {
      const url = refresh ? "/api/library?refresh=true" : "/api/library";
      const data = await apiFetch<MediaEntry[]>(url, {}, session.token);
      setItems(data);
    } catch (err) {
      setError((err as Error).message || "Failed to load library.");
    } finally {
      setLoading(false);
    }
  }

  async function loadDetail(id: string) {
    if (!session) return;
    setDetailLoading(true);
    setError(null);
    try {
      const data = await apiFetch<MediaDetailResponse>(`/api/media/${id}`, {}, session.token);
      setDetail(data);
    } catch (err) {
      setError((err as Error).message || "Failed to load media detail.");
    } finally {
      setDetailLoading(false);
    }
  }

  async function loadCandidates(id: string) {
    if (!session || !isAdmin) return;
    setCandidateLoading(true);
    try {
      const data = await apiFetch<{ candidates: MatchCandidate[] }>(
        `/api/matches/${id}/candidates`,
        {},
        session.token
      );
      setCandidates(data.candidates);
      setManualSubjectId(data.candidates[0]?.subject_id ?? null);
    } catch (err) {
      setError((err as Error).message || "Failed to load match candidates.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function applyManualMatch() {
    if (!session || !selectedId || !manualSubjectId) return;
    setCandidateLoading(true);
    setError(null);
    try {
      const payload: { subject_id: number; episode_id?: number } = {
        subject_id: manualSubjectId
      };
      const episode = Number(manualEpisodeId);
      if (!Number.isNaN(episode) && episode > 0) {
        payload.episode_id = episode;
      }
      await apiFetch<unknown>(
        `/api/matches/${selectedId}`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify(payload)
        },
        session.token
      );
      await loadDetail(selectedId);
    } catch (err) {
      setError((err as Error).message || "Failed to set manual match.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function clearMatch() {
    if (!session || !selectedId) return;
    setCandidateLoading(true);
    setError(null);
    try {
      await apiFetch<unknown>(
        `/api/matches/${selectedId}`,
        { method: "DELETE" },
        session.token
      );
      await loadDetail(selectedId);
      setCandidates([]);
      setManualSubjectId(null);
    } catch (err) {
      setError((err as Error).message || "Failed to clear match.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function handleRefresh() {
    if (!isAdmin) return;
    setRefreshing(true);
    await loadLibrary(true);
    setRefreshing(false);
  }

  useEffect(() => {
    loadLibrary(false);
  }, [session]);

  useEffect(() => {
    if (!selectedId) return;
    if (!items.some((item) => item.id === selectedId)) {
      setSelectedId(null);
      setDetail(null);
    }
  }, [items, selectedId]);

  useEffect(() => {
    if (!selectedId || !isAdmin) {
      setCandidates([]);
      setManualSubjectId(null);
      return;
    }
    loadCandidates(selectedId);
  }, [selectedId, isAdmin]);

  function handleSelect(id: string) {
    setSelectedId(id);
    loadDetail(id);
  }

  function openPlayer(id: string) {
    navigate(`/player/${id}`);
  }

  return (
    <AppShell
      title="Library"
      subtitle="Indexed media files and match status."
      actions={
        isAdmin ? (
          <button
            type="button"
            className="app-btn"
            onClick={handleRefresh}
            disabled={refreshing}
          >
            {refreshing ? "Refreshing..." : "Refresh"}
          </button>
        ) : undefined
      }
    >
      {error ? <div className="library-error">{error}</div> : null}

      <div className="library-toolbar app-card">
        <div>
          <h2 className="app-card-title">Browse</h2>
          <p className="app-card-subtitle">Search by filename.</p>
        </div>
        <div className="library-toolbar-actions">
          <input
            className="app-input"
            type="search"
            placeholder="Search..."
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <span className="app-pill">{filteredItems.length} items</span>
        </div>
      </div>

      <div className="library-layout">
        <div className="app-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Files</h2>
            {loading ? <span className="app-muted">Loading...</span> : null}
          </div>
          {filteredItems.length === 0 ? (
            <div className="library-empty">
              {loading ? "Loading library..." : "No media files found."}
            </div>
          ) : (
            <div className="library-table-wrap">
              <table className="app-table">
                <thead>
                  <tr>
                    <th>Filename</th>
                    <th>Size</th>
                    <th>Action</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredItems.map((item) => (
                    <tr
                      key={item.id}
                      className={selectedId === item.id ? "is-active" : undefined}
                      onClick={() => handleSelect(item.id)}
                    >
                      <td>{item.filename}</td>
                      <td>{formatBytes(item.size)}</td>
                      <td>
                        <button
                          type="button"
                          className="app-btn ghost"
                          onClick={(event) => {
                            event.stopPropagation();
                            openPlayer(item.id);
                          }}
                        >
                          Open
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        <div className="app-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Details</h2>
            {detailLoading ? <span className="app-muted">Loading...</span> : null}
          </div>
          {!detail ? (
            <div className="library-empty">Select a file to view details.</div>
          ) : (
            <div className="library-detail">
              <div className="library-detail-block">
                <h3>Parse</h3>
                <div className="library-row">
                  <span>Title</span>
                  <span>{detail.parse?.title ?? "--"}</span>
                </div>
                <div className="library-row">
                  <span>Episode</span>
                  <span>{detail.parse?.episode ?? "--"}</span>
                </div>
                <div className="library-row">
                  <span>Season</span>
                  <span>{detail.parse?.season ?? "--"}</span>
                </div>
                <div className="library-row">
                  <span>Year</span>
                  <span>{detail.parse?.year ?? "--"}</span>
                </div>
                <div className="library-row">
                  <span>Group</span>
                  <span>{detail.parse?.release_group ?? "--"}</span>
                </div>
                <div className="library-row">
                  <span>Resolution</span>
                  <span>{detail.parse?.resolution ?? "--"}</span>
                </div>
              </div>

              <div className="library-detail-block">
                <h3>Match</h3>
                {detail.matched ? (
                  <>
                    <div className="library-row">
                      <span>Subject</span>
                      <span>{detail.matched.subject.name}</span>
                    </div>
                    <div className="library-row">
                      <span>Chinese</span>
                      <span>{detail.matched.subject.name_cn}</span>
                    </div>
                    <div className="library-row">
                      <span>Method</span>
                      <span>{detail.matched.method}</span>
                    </div>
                    <div className="library-row">
                      <span>Confidence</span>
                      <span>
                        {detail.matched.confidence !== null && detail.matched.confidence !== undefined
                          ? detail.matched.confidence.toFixed(2)
                          : "--"}
                      </span>
                    </div>
                    <div className="library-row">
                      <span>Reason</span>
                      <span>{detail.matched.reason ?? "--"}</span>
                    </div>
                  </>
                ) : (
                  <div className="library-empty">No match yet.</div>
                )}
              </div>

              {isAdmin ? (
                <div className="library-detail-block">
                  <h3>Match control</h3>
                  {candidateLoading ? (
                    <div className="library-empty">Loading candidates...</div>
                  ) : candidates.length ? (
                    <>
                      <label className="library-label">
                        <span>Candidate</span>
                        <select
                          className="app-select"
                          value={manualSubjectId ?? undefined}
                          onChange={(event) => setManualSubjectId(Number(event.target.value))}
                        >
                          {candidates.map((candidate) => (
                            <option key={candidate.subject_id} value={candidate.subject_id}>
                              {candidate.name} ({candidate.name_cn}) · {candidate.confidence.toFixed(2)}
                            </option>
                          ))}
                        </select>
                      </label>
                      <label className="library-label">
                        <span>Episode id (optional)</span>
                        <input
                          className="app-input"
                          type="number"
                          value={manualEpisodeId}
                          onChange={(event) => setManualEpisodeId(event.target.value)}
                          placeholder="episode id"
                        />
                      </label>
                      <div className="library-actions">
                        <button
                          type="button"
                          className="app-btn primary"
                          onClick={applyManualMatch}
                          disabled={!manualSubjectId}
                        >
                          Set match
                        </button>
                        <button type="button" className="app-btn ghost" onClick={clearMatch}>
                          Clear match
                        </button>
                      </div>
                    </>
                  ) : (
                    <div className="library-empty">No candidates available.</div>
                  )}
                </div>
              ) : null}

              <div className="library-detail-block">
                <h3>Progress</h3>
                <div className="library-row">
                  <span>Position</span>
                  <span>{formatDuration(detail.progress?.position_secs)}</span>
                </div>
                <div className="library-row">
                  <span>Duration</span>
                  <span>{formatDuration(detail.progress?.duration_secs ?? null)}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </AppShell>
  );
}
