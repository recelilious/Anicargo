import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
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

function parsePositiveNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return Math.floor(parsed);
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
  const [searchParams] = useSearchParams();
  const [items, setItems] = useState<MediaEntry[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
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
  const [query, setQuery] = useState(() => searchParams.get("q") ?? "");
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [bulkSubjectInput, setBulkSubjectInput] = useState("");
  const [bulkEpisodeInput, setBulkEpisodeInput] = useState("");
  const [bulkWorking, setBulkWorking] = useState(false);

  const isAdmin = (session?.roleLevel ?? 0) >= 3;
  const selectedSet = useMemo(() => new Set(selectedIds), [selectedIds]);

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

  const manualSubjectOverride = useMemo(() => {
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
  const subjectIdForEpisodes = manualSubjectOverride;
  const matchedSubjectId = detail?.matched?.subject?.id ?? null;
  const bulkSubjectId = useMemo(
    () => parsePositiveNumber(bulkSubjectInput),
    [bulkSubjectInput]
  );
  const bulkEpisodeId = useMemo(
    () => parsePositiveNumber(bulkEpisodeInput),
    [bulkEpisodeInput]
  );
  const bulkSubjectInvalid = bulkSubjectInput.trim().length > 0 && bulkSubjectId === null;
  const bulkEpisodeInvalid = bulkEpisodeInput.trim().length > 0 && bulkEpisodeId === null;
  const allFilteredSelected =
    isAdmin && filteredItems.length > 0 && filteredItems.every((item) => selectedSet.has(item.id));

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
      setError((err as Error).message || "Failed to load episodes.");
      setEpisodes([]);
      setEpisodesSubject(null);
    } finally {
      setEpisodesLoading(false);
    }
  }

  async function applyManualMatch() {
    if (!session || !selectedId) return;
    if (manualSubjectInvalid) {
      setError("Invalid subject id.");
      return;
    }
    if (!manualSubjectOverride) {
      setError("Select a candidate or enter a subject id.");
      return;
    }
    const subjectOverride = manualSubjectOverride;

    setCandidateLoading(true);
    setError(null);
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

  function toggleSelected(id: string) {
    setSelectedIds((current) =>
      current.includes(id) ? current.filter((item) => item !== id) : [...current, id]
    );
  }

  function toggleSelectAll() {
    if (allFilteredSelected) {
      setSelectedIds((current) =>
        current.filter((id) => !filteredItems.some((item) => item.id === id))
      );
      return;
    }
    setSelectedIds((current) => {
      const next = new Set(current);
      filteredItems.forEach((item) => next.add(item.id));
      return Array.from(next);
    });
  }

  async function applyBulkMatch() {
    if (!session || !isAdmin) return;
    if (bulkSubjectInvalid || bulkEpisodeInvalid) {
      setError("Invalid bulk match input.");
      return;
    }
    if (!bulkSubjectId) {
      setError("Bulk match needs a subject id.");
      return;
    }
    if (!selectedIds.length) {
      setError("Select at least one file for bulk match.");
      return;
    }
    setBulkWorking(true);
    setError(null);
    let failures = 0;
    const payload: { subject_id: number; episode_id?: number } = {
      subject_id: bulkSubjectId
    };
    if (bulkEpisodeId) {
      payload.episode_id = bulkEpisodeId;
    }
    for (const id of selectedIds) {
      try {
        await apiFetch<unknown>(
          `/api/matches/${id}`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json"
            },
            body: JSON.stringify(payload)
          },
          session.token
        );
      } catch {
        failures += 1;
      }
    }
    if (selectedId && selectedIds.includes(selectedId)) {
      await loadDetail(selectedId);
    }
    if (failures > 0) {
      setError(`Bulk match finished with ${failures} error(s).`);
    }
    setBulkWorking(false);
  }

  async function clearBulkMatches() {
    if (!session || !isAdmin) return;
    if (!selectedIds.length) {
      setError("Select at least one file to clear.");
      return;
    }
    setBulkWorking(true);
    setError(null);
    let failures = 0;
    for (const id of selectedIds) {
      try {
        await apiFetch<unknown>(`/api/matches/${id}`, { method: "DELETE" }, session.token);
      } catch {
        failures += 1;
      }
    }
    if (selectedId && selectedIds.includes(selectedId)) {
      await loadDetail(selectedId);
    }
    if (failures > 0) {
      setError(`Bulk clear finished with ${failures} error(s).`);
    }
    setBulkWorking(false);
  }

  useEffect(() => {
    loadLibrary(false);
  }, [session]);

  useEffect(() => {
    const q = searchParams.get("q");
    setQuery(q ?? "");
  }, [searchParams]);

  useEffect(() => {
    if (!selectedId) return;
    if (!items.some((item) => item.id === selectedId)) {
      setSelectedId(null);
      setDetail(null);
    }
  }, [items, selectedId]);

  useEffect(() => {
    if (!selectedIds.length) return;
    setSelectedIds((current) => current.filter((id) => items.some((item) => item.id === id)));
  }, [items, selectedIds.length]);

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
    if (!selectedId || !isAdmin) {
      setCandidates([]);
      setManualSubjectId(null);
      setEpisodes([]);
      setEpisodesSubject(null);
      return;
    }
    loadCandidates(selectedId);
  }, [selectedId, isAdmin]);

  useEffect(() => {
    if (!isAdmin || !subjectIdForEpisodes) {
      setEpisodes([]);
      setEpisodesSubject(null);
      return;
    }
    setManualEpisodeId("");
    loadEpisodes(subjectIdForEpisodes);
  }, [subjectIdForEpisodes, isAdmin]);

  useEffect(() => {
    if (manualSubjectInput.trim()) return;
    if (!filteredCandidates.length) return;
    if (!manualSubjectId || !filteredCandidates.some((item) => item.subject_id === manualSubjectId)) {
      setManualSubjectId(filteredCandidates[0].subject_id);
    }
  }, [filteredCandidates, manualSubjectId, manualSubjectInput]);

  function handleCandidateSelect(candidate: MatchCandidate) {
    setManualSubjectInput("");
    setManualSubjectId(candidate.subject_id);
  }

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
                    {isAdmin ? (
                      <th className="library-checkbox">
                        <input
                          type="checkbox"
                          checked={allFilteredSelected}
                          onChange={toggleSelectAll}
                          aria-label="Select all"
                        />
                      </th>
                    ) : null}
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
                      {isAdmin ? (
                        <td
                          className="library-checkbox"
                          onClick={(event) => event.stopPropagation()}
                        >
                          <input
                            type="checkbox"
                            checked={selectedSet.has(item.id)}
                            onChange={() => toggleSelected(item.id)}
                            aria-label={`Select ${item.filename}`}
                          />
                        </td>
                      ) : null}
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
          {isAdmin ? (
            <div className="library-bulk">
              <div className="library-bulk-header">
                <div>
                  <h3>Bulk actions</h3>
                  <p className="app-card-subtitle">Apply one subject to many files.</p>
                </div>
                <span className="app-pill">{selectedIds.length} selected</span>
              </div>
              <div className="library-bulk-fields">
                <label className="library-label">
                  <span>Subject id</span>
                  <input
                    className="app-input"
                    type="number"
                    value={bulkSubjectInput}
                    onChange={(event) => setBulkSubjectInput(event.target.value)}
                    placeholder="Bangumi subject id"
                    disabled={bulkWorking}
                  />
                </label>
                <label className="library-label">
                  <span>Episode id (optional)</span>
                  <input
                    className="app-input"
                    type="number"
                    value={bulkEpisodeInput}
                    onChange={(event) => setBulkEpisodeInput(event.target.value)}
                    placeholder="Bangumi episode id"
                    disabled={bulkWorking}
                  />
                </label>
              </div>
              {bulkSubjectInvalid || bulkEpisodeInvalid ? (
                <div className="library-help error">Enter valid numeric IDs.</div>
              ) : null}
              <div className="library-actions">
                <button
                  type="button"
                  className="app-btn primary"
                  onClick={applyBulkMatch}
                  disabled={
                    bulkWorking ||
                    !selectedIds.length ||
                    bulkSubjectInvalid ||
                    bulkEpisodeInvalid ||
                    !bulkSubjectId
                  }
                >
                  Apply match
                </button>
                <button
                  type="button"
                  className="app-btn ghost"
                  onClick={clearBulkMatches}
                  disabled={bulkWorking || !selectedIds.length}
                >
                  Clear matches
                </button>
                <button
                  type="button"
                  className="app-btn ghost"
                  onClick={() => setSelectedIds([])}
                  disabled={!selectedIds.length}
                >
                  Clear selection
                </button>
              </div>
            </div>
          ) : null}
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
                  ) : filteredCandidates.length ? (
                    <>
                      <div className="library-candidate-tools">
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

                      <div className="library-candidate-list">
                        {filteredCandidates.map((candidate) => {
                          const isSelected =
                            !manualSubjectInput.trim() &&
                            manualSubjectId === candidate.subject_id;
                          const isCurrent = matchedSubjectId === candidate.subject_id;
                          const displayName = candidate.name_cn || candidate.name;
                          return (
                            <button
                              key={candidate.subject_id}
                              type="button"
                              className={`library-candidate-item${
                                isSelected ? " is-selected" : ""
                              }${isCurrent ? " is-current" : ""}`}
                              onClick={() => handleCandidateSelect(candidate)}
                            >
                              <div className="library-candidate-main">
                                <span className="library-candidate-title">{displayName}</span>
                                <span className="library-candidate-subtitle">{candidate.name}</span>
                              </div>
                              <div className="library-candidate-meta">
                                <span className="library-candidate-score">
                                  {candidate.confidence.toFixed(2)}
                                </span>
                                <span className="library-candidate-reason">{candidate.reason}</span>
                                {isCurrent ? <span className="app-pill">current</span> : null}
                              </div>
                            </button>
                          );
                        })}
                      </div>
                      <label className="library-label">
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
                        <div className="library-help error">Invalid subject id.</div>
                      ) : null}
                      <label className="library-label">
                        <span>
                          Episodes
                          {episodesSubject ? ` · ${episodesSubject.name}` : ""}
                        </span>
                        {episodesLoading ? (
                          <div className="library-empty">Loading episodes...</div>
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
                      <div className="library-actions">
                        <button
                          type="button"
                          className="app-btn primary"
                          onClick={applyManualMatch}
                          disabled={!manualSubjectOverride || manualSubjectInvalid}
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
