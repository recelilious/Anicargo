import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import AppShell from "../Components/AppShell";
import { apiFetch, apiFetchEmpty } from "../api";
import { useSession } from "../session";
import "../Styles/PageManagement.css";

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

interface EpisodeListResponse {
  subject: BangumiSubjectInfo;
  episodes: BangumiEpisodeInfo[];
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

interface CollectionItem {
  id: number;
  submitter_id: string;
  kind: string;
  status: string;
  magnet?: string | null;
  torrent_name?: string | null;
  note?: string | null;
  decision_note?: string | null;
  created_at: string;
  decided_at?: string | null;
  decided_by?: string | null;
}

interface CollectionListResponse {
  items: CollectionItem[];
}

interface CollectionCreateResponse {
  id: number;
  status: string;
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

function formatDate(value?: string | null): string {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function compactText(value?: string | null, max = 38): string {
  if (!value) return "--";
  if (value.length <= max) return value;
  return `${value.slice(0, max - 3)}...`;
}

export default function PageManagement() {
  const { session } = useSession();
  const navigate = useNavigate();
  const [items, setItems] = useState<MediaEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<MediaDetailResponse | null>(null);
  const [candidates, setCandidates] = useState<MatchCandidate[]>([]);
  const [candidateLoading, setCandidateLoading] = useState(false);
  const [manualSubjectId, setManualSubjectId] = useState<number | null>(null);
  const [manualEpisodeId, setManualEpisodeId] = useState("");
  const [candidateQuery, setCandidateQuery] = useState("");
  const [candidateSort, setCandidateSort] = useState("confidence");
  const [manualSubjectInput, setManualSubjectInput] = useState("");
  const [episodes, setEpisodes] = useState<BangumiEpisodeInfo[]>([]);
  const [episodesLoading, setEpisodesLoading] = useState(false);
  const [episodesSubject, setEpisodesSubject] = useState<BangumiSubjectInfo | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [matchError, setMatchError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const [collectionItems, setCollectionItems] = useState<CollectionItem[]>([]);
  const [collectionLoading, setCollectionLoading] = useState(false);
  const [collectionError, setCollectionError] = useState<string | null>(null);
  const [collectionFilter, setCollectionFilter] = useState("pending");
  const [decisionNotes, setDecisionNotes] = useState<Record<number, string>>({});
  const [collectionWorking, setCollectionWorking] = useState(false);

  const filteredItems = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) => item.filename.toLowerCase().includes(needle));
  }, [items, query]);

  const filteredCandidates = useMemo(() => {
    let next = [...candidates];
    const needle = candidateQuery.trim().toLowerCase();
    if (needle) {
      next = next.filter(
        (candidate) =>
          candidate.name.toLowerCase().includes(needle) ||
          candidate.name_cn.toLowerCase().includes(needle)
      );
    }
    if (candidateSort === "name") {
      next.sort((a, b) => a.name.localeCompare(b.name));
    } else {
      next.sort((a, b) => b.confidence - a.confidence);
    }
    return next;
  }, [candidates, candidateQuery, candidateSort]);

  const subjectIdForEpisodes = useMemo(() => {
    const trimmed = manualSubjectInput.trim();
    if (trimmed) {
      const value = Number(trimmed);
      if (Number.isFinite(value) && value > 0) {
        return value;
      }
      return null;
    }
    return manualSubjectId ?? null;
  }, [manualSubjectInput, manualSubjectId]);

  const manualSubjectInvalid = useMemo(() => {
    const trimmed = manualSubjectInput.trim();
    if (!trimmed) return false;
    const value = Number(trimmed);
    return !Number.isFinite(value) || value <= 0;
  }, [manualSubjectInput]);

  async function loadLibrary(refresh = false) {
    if (!session) return;
    setLoading(true);
    setMatchError(null);
    try {
      const url = refresh ? "/api/library?refresh=true" : "/api/library";
      const data = await apiFetch<MediaEntry[]>(url, {}, session.token);
      setItems(data);
    } catch (err) {
      setMatchError((err as Error).message || "Failed to load library.");
    } finally {
      setLoading(false);
    }
  }

  async function loadDetail(id: string) {
    if (!session) return;
    setDetailLoading(true);
    setMatchError(null);
    try {
      const data = await apiFetch<MediaDetailResponse>(`/api/media/${id}`, {}, session.token);
      setDetail(data);
    } catch (err) {
      setMatchError((err as Error).message || "Failed to load media detail.");
    } finally {
      setDetailLoading(false);
    }
  }

  async function loadCandidates(id: string) {
    if (!session) return;
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
      setMatchError((err as Error).message || "Failed to load match candidates.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function loadEpisodes(subjectId: number) {
    if (!session) return;
    setEpisodesLoading(true);
    try {
      const data = await apiFetch<EpisodeListResponse>(
        `/api/subjects/${subjectId}/episodes`,
        {},
        session.token
      );
      setEpisodes(data.episodes);
      setEpisodesSubject(data.subject);
    } catch (err) {
      setMatchError((err as Error).message || "Failed to load episodes.");
      setEpisodes([]);
      setEpisodesSubject(null);
    } finally {
      setEpisodesLoading(false);
    }
  }

  async function applyManualMatch() {
    if (!session || !selectedId) return;
    if (manualSubjectInvalid) {
      setMatchError("Invalid subject id.");
      return;
    }
    const trimmed = manualSubjectInput.trim();
    const subjectOverride = trimmed ? Number(trimmed) : manualSubjectId;
    if (!subjectOverride) {
      setMatchError("Select a candidate or enter a subject id.");
      return;
    }

    setCandidateLoading(true);
    setMatchError(null);
    try {
      const payload: { subject_id: number; episode_id?: number } = {
        subject_id: subjectOverride
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
      setMatchError((err as Error).message || "Failed to set manual match.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function clearMatch() {
    if (!session || !selectedId) return;
    setCandidateLoading(true);
    setMatchError(null);
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
      setMatchError((err as Error).message || "Failed to clear match.");
    } finally {
      setCandidateLoading(false);
    }
  }

  async function handleRefresh() {
    setRefreshing(true);
    await loadLibrary(true);
    setRefreshing(false);
  }

  function handleSelect(id: string) {
    setSelectedId(id);
    loadDetail(id);
  }

  function openPlayer(id: string) {
    navigate(`/player/${id}`);
  }

  async function loadCollection() {
    if (!session) return;
    setCollectionLoading(true);
    setCollectionError(null);
    try {
      const query = collectionFilter === "all" ? "" : `?status=${collectionFilter}`;
      const data = await apiFetch<CollectionListResponse>(
        `/api/collection${query}`,
        {},
        session.token
      );
      setCollectionItems(data.items);
      setDecisionNotes((current) => {
        const next = { ...current };
        data.items.forEach((item) => {
          if (!(item.id in next)) {
            next[item.id] = item.decision_note ?? "";
          }
        });
        Object.keys(next).forEach((key) => {
          const id = Number(key);
          if (!data.items.some((item) => item.id === id)) {
            delete next[id];
          }
        });
        return next;
      });
    } catch (err) {
      setCollectionError((err as Error).message || "Failed to load submissions.");
    } finally {
      setCollectionLoading(false);
    }
  }

  async function approveItem(id: number) {
    if (!session) return;
    setCollectionWorking(true);
    setCollectionError(null);
    try {
      const note = decisionNotes[id]?.trim() || null;
      await apiFetch<CollectionCreateResponse>(
        `/api/collection/${id}/approve`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({ note })
        },
        session.token
      );
      await loadCollection();
    } catch (err) {
      setCollectionError((err as Error).message || "Failed to approve submission.");
    } finally {
      setCollectionWorking(false);
    }
  }

  async function rejectItem(id: number) {
    if (!session) return;
    setCollectionWorking(true);
    setCollectionError(null);
    try {
      const note = decisionNotes[id]?.trim() || null;
      await apiFetch<CollectionCreateResponse>(
        `/api/collection/${id}/reject`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({ note })
        },
        session.token
      );
      await loadCollection();
    } catch (err) {
      setCollectionError((err as Error).message || "Failed to reject submission.");
    } finally {
      setCollectionWorking(false);
    }
  }

  async function deleteItem(id: number) {
    if (!session) return;
    setCollectionWorking(true);
    setCollectionError(null);
    try {
      await apiFetchEmpty(`/api/collection/${id}`, { method: "DELETE" }, session.token);
      await loadCollection();
    } catch (err) {
      setCollectionError((err as Error).message || "Failed to delete submission.");
    } finally {
      setCollectionWorking(false);
    }
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
    if (!selectedId) return;
    setManualEpisodeId("");
    setManualSubjectInput("");
    setCandidateQuery("");
    setCandidateSort("confidence");
    setEpisodes([]);
    setEpisodesSubject(null);
  }, [selectedId]);

  useEffect(() => {
    if (!selectedId) {
      setCandidates([]);
      setManualSubjectId(null);
      setEpisodes([]);
      setEpisodesSubject(null);
      return;
    }
    loadCandidates(selectedId);
  }, [selectedId]);

  useEffect(() => {
    if (!subjectIdForEpisodes) {
      setEpisodes([]);
      setEpisodesSubject(null);
      return;
    }
    setManualEpisodeId("");
    loadEpisodes(subjectIdForEpisodes);
  }, [subjectIdForEpisodes]);

  useEffect(() => {
    if (manualSubjectInput.trim()) return;
    if (!filteredCandidates.length) return;
    if (!manualSubjectId || !filteredCandidates.some((item) => item.subject_id === manualSubjectId)) {
      setManualSubjectId(filteredCandidates[0].subject_id);
    }
  }, [filteredCandidates, manualSubjectId, manualSubjectInput]);

  useEffect(() => {
    loadCollection();
  }, [session, collectionFilter]);

  return (
    <AppShell title="Management" subtitle="Match control and approvals.">
      {matchError ? <div className="management-error">{matchError}</div> : null}

      <section className="management-layout">
        <div className="app-card">
          <div className="app-card-header">
            <h2 className="app-card-title">Files</h2>
            {loading ? <span className="app-muted">Loading...</span> : null}
          </div>
          <div className="management-search">
            <input
              className="app-input"
              type="search"
              placeholder="Search files..."
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            <button
              type="button"
              className="app-btn"
              onClick={handleRefresh}
              disabled={refreshing}
            >
              {refreshing ? "Refreshing..." : "Refresh"}
            </button>
          </div>
          {filteredItems.length === 0 ? (
            <div className="management-empty">
              {loading ? "Loading library..." : "No media files found."}
            </div>
          ) : (
            <div className="management-table-wrap">
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
            <h2 className="app-card-title">Match control</h2>
            {detailLoading ? <span className="app-muted">Loading...</span> : null}
          </div>
          {!detail ? (
            <div className="management-empty">Select a file to view details.</div>
          ) : (
            <div className="management-detail">
              <div className="management-detail-block">
                <h3>Parse</h3>
                <div className="management-row">
                  <span>Title</span>
                  <span>{detail.parse?.title ?? "--"}</span>
                </div>
                <div className="management-row">
                  <span>Episode</span>
                  <span>{detail.parse?.episode ?? "--"}</span>
                </div>
                <div className="management-row">
                  <span>Season</span>
                  <span>{detail.parse?.season ?? "--"}</span>
                </div>
                <div className="management-row">
                  <span>Year</span>
                  <span>{detail.parse?.year ?? "--"}</span>
                </div>
                <div className="management-row">
                  <span>Group</span>
                  <span>{detail.parse?.release_group ?? "--"}</span>
                </div>
                <div className="management-row">
                  <span>Resolution</span>
                  <span>{detail.parse?.resolution ?? "--"}</span>
                </div>
              </div>

              <div className="management-detail-block">
                <h3>Match</h3>
                {detail.matched ? (
                  <>
                    <div className="management-row">
                      <span>Subject</span>
                      <span>{detail.matched.subject.name}</span>
                    </div>
                    <div className="management-row">
                      <span>Chinese</span>
                      <span>{detail.matched.subject.name_cn}</span>
                    </div>
                    <div className="management-row">
                      <span>Method</span>
                      <span>{detail.matched.method}</span>
                    </div>
                    <div className="management-row">
                      <span>Confidence</span>
                      <span>
                        {detail.matched.confidence !== null && detail.matched.confidence !== undefined
                          ? detail.matched.confidence.toFixed(2)
                          : "--"}
                      </span>
                    </div>
                    <div className="management-row">
                      <span>Reason</span>
                      <span>{detail.matched.reason ?? "--"}</span>
                    </div>
                  </>
                ) : (
                  <div className="management-empty">No match yet.</div>
                )}
              </div>

              <div className="management-detail-block">
                <h3>Match control</h3>
                {candidateLoading ? (
                  <div className="management-empty">Loading candidates...</div>
                ) : filteredCandidates.length ? (
                  <>
                    <div className="management-candidate-tools">
                      <input
                        className="app-input"
                        type="search"
                        placeholder="Filter candidates..."
                        value={candidateQuery}
                        onChange={(event) => setCandidateQuery(event.target.value)}
                      />
                      <select
                        className="app-select"
                        value={candidateSort}
                        onChange={(event) => setCandidateSort(event.target.value)}
                      >
                        <option value="confidence">Sort by confidence</option>
                        <option value="name">Sort by name</option>
                      </select>
                      <span className="app-pill">{filteredCandidates.length} results</span>
                    </div>

                    <label className="management-label">
                      <span>Candidate</span>
                      <select
                        className="app-select"
                        value={manualSubjectId ?? undefined}
                        onChange={(event) => setManualSubjectId(Number(event.target.value))}
                        disabled={Boolean(manualSubjectInput.trim())}
                      >
                        {filteredCandidates.map((candidate) => (
                          <option key={candidate.subject_id} value={candidate.subject_id}>
                            {candidate.name} ({candidate.name_cn}) · {candidate.confidence.toFixed(2)}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="management-label">
                      <span>Manual subject id (override)</span>
                      <input
                        className="app-input"
                        type="number"
                        value={manualSubjectInput}
                        onChange={(event) => setManualSubjectInput(event.target.value)}
                        placeholder="Bangumi subject id"
                      />
                    </label>
                    {manualSubjectInvalid ? (
                      <div className="management-help error">Invalid subject id.</div>
                    ) : null}
                    <label className="management-label">
                      <span>Episodes{episodesSubject ? ` · ${episodesSubject.name}` : ""}</span>
                      {episodesLoading ? (
                        <div className="management-empty">Loading episodes...</div>
                      ) : episodes.length ? (
                        <select
                          className="app-select"
                          value={manualEpisodeId}
                          onChange={(event) => setManualEpisodeId(event.target.value)}
                        >
                          <option value="">Not set</option>
                          {episodes.map((episode) => (
                            <option key={episode.id} value={episode.id.toString()}>
                              E{episode.ep ?? episode.sort} {episode.name_cn || episode.name}
                            </option>
                          ))}
                        </select>
                      ) : (
                        <input
                          className="app-input"
                          type="number"
                          value={manualEpisodeId}
                          onChange={(event) => setManualEpisodeId(event.target.value)}
                          placeholder="Episode id (optional)"
                        />
                      )}
                    </label>
                    <div className="management-actions">
                      <button
                        type="button"
                        className="app-btn primary"
                        onClick={applyManualMatch}
                        disabled={manualSubjectInvalid || (!manualSubjectId && !manualSubjectInput.trim())}
                      >
                        Set match
                      </button>
                      <button type="button" className="app-btn ghost" onClick={clearMatch}>
                        Clear match
                      </button>
                    </div>
                  </>
                ) : (
                  <div className="management-empty">No candidates available.</div>
                )}
              </div>

              <div className="management-detail-block">
                <h3>Progress</h3>
                <div className="management-row">
                  <span>Position</span>
                  <span>{formatDuration(detail.progress?.position_secs)}</span>
                </div>
                <div className="management-row">
                  <span>Duration</span>
                  <span>{formatDuration(detail.progress?.duration_secs ?? null)}</span>
                </div>
              </div>
            </div>
          )}
        </div>
      </section>

      <section className="app-card management-collection">
        <div className="app-card-header">
          <div>
            <h2 className="app-card-title">Collection approvals</h2>
            <p className="app-card-subtitle">Approve or reject submissions.</p>
          </div>
          <div className="management-collection-actions">
            <select
              value={collectionFilter}
              onChange={(event) => setCollectionFilter(event.target.value)}
              className="app-select"
            >
              <option value="pending">Pending</option>
              <option value="approved">Approved</option>
              <option value="rejected">Rejected</option>
              <option value="all">All</option>
            </select>
            <button
              type="button"
              className="app-btn"
              onClick={loadCollection}
              disabled={collectionLoading}
            >
              {collectionLoading ? "Refreshing..." : "Refresh"}
            </button>
          </div>
        </div>
        {collectionError ? <div className="management-error">{collectionError}</div> : null}
        {collectionItems.length === 0 ? (
          <div className="management-empty">
            {collectionLoading ? "Loading submissions..." : "No submissions."}
          </div>
        ) : (
          <div className="management-collection-list">
            {collectionItems.map((item) => (
              <div key={item.id} className="app-card management-collection-item">
                <div className="management-collection-header">
                  <div>
                    <strong>#{item.id}</strong>
                    <span className={`management-status status-${item.status}`}>
                      {item.status}
                    </span>
                  </div>
                  <span className="app-pill">{item.kind}</span>
                </div>
                <div className="management-collection-body">
                  <div className="management-row">
                    <span>Source</span>
                    <span title={item.magnet ?? item.torrent_name ?? ""}>
                      {item.kind === "magnet"
                        ? compactText(item.magnet)
                        : compactText(item.torrent_name)}
                    </span>
                  </div>
                  <div className="management-row">
                    <span>Note</span>
                    <span>{item.note || "--"}</span>
                  </div>
                  <div className="management-row">
                    <span>Submitted</span>
                    <span>{formatDate(item.created_at)}</span>
                  </div>
                  <div className="management-row">
                    <span>Submitter</span>
                    <span>{item.submitter_id}</span>
                  </div>
                  <div className="management-row">
                    <span>Decision</span>
                    <span>{item.decision_note || "--"}</span>
                  </div>
                  <div className="management-row">
                    <span>Decided by</span>
                    <span>{item.decided_by || "--"}</span>
                  </div>
                  <div className="management-row">
                    <span>Decided at</span>
                    <span>{formatDate(item.decided_at)}</span>
                  </div>
                  {item.status === "pending" ? (
                    <label className="management-label">
                      <span>Decision note</span>
                      <input
                        className="app-input"
                        type="text"
                        value={decisionNotes[item.id] ?? ""}
                        onChange={(event) =>
                          setDecisionNotes((prev) => ({
                            ...prev,
                            [item.id]: event.target.value
                          }))
                        }
                        placeholder="Optional note for approval/rejection"
                        disabled={collectionWorking}
                      />
                    </label>
                  ) : null}
                </div>
                <div className="management-actions">
                  {item.status === "pending" ? (
                    <>
                      <button
                        type="button"
                        className="app-btn"
                        onClick={() => approveItem(item.id)}
                        disabled={collectionWorking}
                      >
                        Approve
                      </button>
                      <button
                        type="button"
                        className="app-btn ghost"
                        onClick={() => rejectItem(item.id)}
                        disabled={collectionWorking}
                      >
                        Reject
                      </button>
                    </>
                  ) : null}
                  <button
                    type="button"
                    className="app-btn ghost"
                    onClick={() => deleteItem(item.id)}
                    disabled={collectionWorking}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </AppShell>
  );
}
