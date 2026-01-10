import { useEffect, useMemo, useState, type CSSProperties } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import AppShell from "../Components/AppShell";
import { apiFetch } from "../api";
import { useSession } from "../session";
import { loadLibraryColumns } from "../uiSettings";
import "../Styles/PageLibrary.css";

interface MediaEntry {
  id: string;
  filename: string;
  size: number;
}

interface MediaParseInfo {
  episode?: string | null;
}

interface BangumiSubjectInfo {
  id: number;
  name: string;
  name_cn: string;
  air_date?: string | null;
  total_episodes?: number | null;
}

interface MediaMatchDetail {
  subject: BangumiSubjectInfo;
}

interface MediaDetailResponse {
  entry: MediaEntry;
  parse?: MediaParseInfo | null;
  matched?: MediaMatchDetail | null;
}

interface SubjectCard {
  subjectId: number;
  name: string;
  name_cn: string;
  air_date?: string | null;
  total_episodes?: number | null;
  primaryMediaId: string;
  mediaCount: number;
  year?: number | null;
  primaryEpisode?: number | null;
}

function parseEpisodeNumber(value?: string | null): number | null {
  if (!value) return null;
  const match = value.match(/\d+/);
  if (!match) return null;
  const parsed = Number(match[0]);
  return Number.isFinite(parsed) ? parsed : null;
}

function parseYear(value?: string | null): number | null {
  if (!value) return null;
  const match = value.match(/(\d{4})/);
  if (!match) return null;
  const parsed = Number(match[1]);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeNumberInput(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed)) return null;
  return parsed;
}

export default function PageLibrary() {
  const { session } = useSession();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [cards, setCards] = useState<SubjectCard[]>([]);
  const [unmatchedCount, setUnmatchedCount] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState(() => searchParams.get("q") ?? "");
  const [yearEnabled, setYearEnabled] = useState(false);
  const [yearMin, setYearMin] = useState("");
  const [yearMax, setYearMax] = useState("");
  const [episodesEnabled, setEpisodesEnabled] = useState(false);
  const [episodesMode, setEpisodesMode] = useState(">=");
  const [episodesValue, setEpisodesValue] = useState("");

  const libraryColumns = loadLibraryColumns();

  const normalizedSearch = useMemo(() => search.trim().toLowerCase(), [search]);
  const yearMinValue = useMemo(() => normalizeNumberInput(yearMin), [yearMin]);
  const yearMaxValue = useMemo(() => normalizeNumberInput(yearMax), [yearMax]);
  const episodesFilterValue = useMemo(
    () => normalizeNumberInput(episodesValue),
    [episodesValue]
  );

  useEffect(() => {
    const q = searchParams.get("q");
    setSearch(q ?? "");
  }, [searchParams]);

  useEffect(() => {
    if (!session) return;
    loadSubjects();
  }, [session]);

  async function loadSubjects() {
    if (!session) return;
    setLoading(true);
    setError(null);
    try {
      const entries = await apiFetch<MediaEntry[]>("/api/library", {}, session.token);
      const details = await fetchDetails(entries);
      const map = new Map<number, SubjectCard>();
      let unmatched = 0;

      details.forEach((detail, index) => {
        if (!detail?.matched?.subject) {
          unmatched += 1;
          return;
        }
        const subject = detail.matched.subject;
        const entry = entries[index];
        if (!entry) return;
        const episodeNumber = parseEpisodeNumber(detail.parse?.episode);
        const year = parseYear(subject.air_date);

        const existing = map.get(subject.id);
        if (!existing) {
          map.set(subject.id, {
            subjectId: subject.id,
            name: subject.name,
            name_cn: subject.name_cn,
            air_date: subject.air_date,
            total_episodes: subject.total_episodes,
            primaryMediaId: entry.id,
            mediaCount: 1,
            year,
            primaryEpisode: episodeNumber
          });
          return;
        }

        existing.mediaCount += 1;
        if (
          episodeNumber !== null &&
          (existing.primaryEpisode === null || episodeNumber < existing.primaryEpisode)
        ) {
          existing.primaryEpisode = episodeNumber;
          existing.primaryMediaId = entry.id;
        }
      });

      const nextCards = Array.from(map.values()).sort((a, b) =>
        a.name.localeCompare(b.name)
      );
      setCards(nextCards);
      setUnmatchedCount(unmatched);
    } catch (err) {
      setError((err as Error).message || "Failed to load library.");
    } finally {
      setLoading(false);
    }
  }

  async function fetchDetails(entries: MediaEntry[]) {
    if (!session) return [];
    const limit = 6;
    const results: Array<MediaDetailResponse | null> = new Array(entries.length).fill(null);
    let cursor = 0;

    async function worker() {
      while (cursor < entries.length) {
        const index = cursor;
        cursor += 1;
        const entry = entries[index];
        if (!entry) continue;
        try {
          const detail = await apiFetch<MediaDetailResponse>(
            `/api/media/${entry.id}`,
            {},
            session.token
          );
          results[index] = detail;
        } catch {
          results[index] = null;
        }
      }
    }

    const workers = Array.from({ length: Math.min(limit, entries.length) }, () => worker());
    await Promise.all(workers);
    return results;
  }

  const filteredCards = useMemo(() => {
    let next = cards;
    if (normalizedSearch) {
      next = next.filter((card) => {
        const name = card.name.toLowerCase();
        const nameCn = card.name_cn.toLowerCase();
        return name.includes(normalizedSearch) || nameCn.includes(normalizedSearch);
      });
    }

    if (yearEnabled) {
      next = next.filter((card) => {
        if (!card.year) return false;
        if (yearMinValue !== null && card.year < yearMinValue) return false;
        if (yearMaxValue !== null && card.year > yearMaxValue) return false;
        return true;
      });
    }

    if (episodesEnabled && episodesFilterValue !== null) {
      next = next.filter((card) => {
        const total = card.total_episodes;
        if (!total || !Number.isFinite(total)) return false;
        if (episodesMode === ">=") {
          return total >= episodesFilterValue;
        }
        return total <= episodesFilterValue;
      });
    }

    return next;
  }, [
    cards,
    normalizedSearch,
    yearEnabled,
    yearMinValue,
    yearMaxValue,
    episodesEnabled,
    episodesFilterValue,
    episodesMode
  ]);

  return (
    <AppShell
      title="Library"
      subtitle="Browse anime titles in your library."
      actions={(
        <button type="button" className="app-btn" onClick={loadSubjects} disabled={loading}>
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      )}
    >
      {error ? <div className="library-error">{error}</div> : null}

      <section className="app-card library-filters">
        <div>
          <h2 className="app-card-title">Filters</h2>
          <p className="app-card-subtitle">Search and refine subjects.</p>
        </div>
        <div className="library-filter-grid">
          <label className="library-label">
            <span>Name search</span>
            <input
              className="app-input"
              type="search"
              placeholder="Search (JP / CN / EN)"
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <label className="library-label">
            <span>Tag search (coming soon)</span>
            <input
              className="app-input"
              type="text"
              placeholder="Tags"
              disabled
            />
          </label>
        </div>

        <div className="library-filter-grid">
          <label className="library-label inline">
            <input
              type="checkbox"
              checked={yearEnabled}
              onChange={(event) => setYearEnabled(event.target.checked)}
            />
            <span>Filter by year</span>
          </label>
          <div className="library-filter-row">
            <input
              className="app-input"
              type="number"
              placeholder="From"
              value={yearMin}
              onChange={(event) => setYearMin(event.target.value)}
              disabled={!yearEnabled}
            />
            <input
              className="app-input"
              type="number"
              placeholder="To"
              value={yearMax}
              onChange={(event) => setYearMax(event.target.value)}
              disabled={!yearEnabled}
            />
          </div>
        </div>

        <div className="library-filter-grid">
          <label className="library-label inline">
            <input
              type="checkbox"
              checked={episodesEnabled}
              onChange={(event) => setEpisodesEnabled(event.target.checked)}
            />
            <span>Filter by episodes</span>
          </label>
          <div className="library-filter-row">
            <select
              className="app-select"
              value={episodesMode}
              onChange={(event) => setEpisodesMode(event.target.value)}
              disabled={!episodesEnabled}
            >
              <option value=">=">Greater / Equal</option>
              <option value="<=">Less / Equal</option>
            </select>
            <input
              className="app-input"
              type="number"
              placeholder="Episodes"
              value={episodesValue}
              onChange={(event) => setEpisodesValue(event.target.value)}
              disabled={!episodesEnabled}
            />
          </div>
        </div>

        <div className="library-filter-meta">
          <span className="app-pill">{filteredCards.length} titles</span>
          {unmatchedCount > 0 ? (
            <span className="app-pill">{unmatchedCount} unmatched</span>
          ) : null}
        </div>
      </section>

      <section
        className="library-grid"
        style={{ "--library-columns": libraryColumns } as CSSProperties}
      >
        {loading && cards.length === 0 ? (
          <div className="library-empty">Loading library...</div>
        ) : filteredCards.length === 0 ? (
          <div className="library-empty">No titles match your filters.</div>
        ) : (
          filteredCards.map((card) => (
            <button
              key={card.subjectId}
              type="button"
              className="library-card"
              onClick={() => navigate(`/player/${card.primaryMediaId}`)}
            >
              <div className="library-card-header">
                <h3>{card.name}</h3>
                <span>{card.year ?? "--"}</span>
              </div>
              <div className="library-card-subtitle">{card.name_cn || "--"}</div>
              <div className="library-card-meta">
                <span>Total episodes</span>
                <span>{card.total_episodes ?? "--"}</span>
              </div>
              <div className="library-card-meta">
                <span>Air date</span>
                <span>{card.air_date ?? "--"}</span>
              </div>
            </button>
          ))
        )}
      </section>
    </AppShell>
  );
}
