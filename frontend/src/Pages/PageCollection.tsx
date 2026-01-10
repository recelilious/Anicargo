import { useEffect, useMemo, useState } from "react";
import AppShell from "../Components/AppShell";
import "../Styles/PageCollection.css";
import { apiFetch, apiFetchEmpty } from "../api";
import { useSession } from "../session";

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

export default function PageCollection() {
  const { session } = useSession();
  const [statusFilter, setStatusFilter] = useState("pending");
  const [items, setItems] = useState<CollectionItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [magnet, setMagnet] = useState("");
  const [magnetNote, setMagnetNote] = useState("");
  const [torrentNote, setTorrentNote] = useState("");
  const [torrentFile, setTorrentFile] = useState<File | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function loadItems() {
    if (!session) return;
    setLoading(true);
    setError(null);
    try {
      const query = statusFilter === "all" ? "" : `?status=${statusFilter}`;
      const data = await apiFetch<CollectionListResponse>(
        `/api/collection${query}`,
        {},
        session.token
      );
      setItems(data.items);
    } catch (err) {
      setError((err as Error).message || "Failed to load submissions.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadItems();
  }, [session, statusFilter]);

  async function submitMagnet(event: React.FormEvent) {
    event.preventDefault();
    if (!session) return;
    const trimmed = magnet.trim();
    if (!trimmed) {
      setError("Magnet link is required.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const payload: { magnet: string; note?: string } = { magnet: trimmed };
      const note = magnetNote.trim();
      if (note) payload.note = note;
      await apiFetch<CollectionCreateResponse>(
        "/api/collection/magnet",
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify(payload)
        },
        session.token
      );
      setMagnet("");
      setMagnetNote("");
      await loadItems();
    } catch (err) {
      setError((err as Error).message || "Failed to submit magnet.");
    } finally {
      setSubmitting(false);
    }
  }

  async function submitTorrent(event: React.FormEvent) {
    event.preventDefault();
    if (!session) return;
    if (!torrentFile) {
      setError("Torrent file is required.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const form = new FormData();
      form.append("torrent", torrentFile);
      const note = torrentNote.trim();
      if (note) form.append("note", note);
      await apiFetch<CollectionCreateResponse>(
        "/api/collection/torrent",
        {
          method: "POST",
          body: form
        },
        session.token
      );
      setTorrentFile(null);
      setTorrentNote("");
      await loadItems();
    } catch (err) {
      setError((err as Error).message || "Failed to submit torrent.");
    } finally {
      setSubmitting(false);
    }
  }

  async function deleteItem(id: number) {
    if (!session) return;
    setSubmitting(true);
    setError(null);
    try {
      await apiFetchEmpty(`/api/collection/${id}`, { method: "DELETE" }, session.token);
      await loadItems();
    } catch (err) {
      setError((err as Error).message || "Failed to delete submission.");
    } finally {
      setSubmitting(false);
    }
  }

  const statusCounts = useMemo(() => {
    return items.reduce(
      (acc, item) => {
        if (item.status === "pending") acc.pending += 1;
        else if (item.status === "approved") acc.approved += 1;
        else if (item.status === "rejected") acc.rejected += 1;
        return acc;
      },
      { pending: 0, approved: 0, rejected: 0 }
    );
  }, [items]);

  return (
    <AppShell title="Collection" subtitle="Submit magnets or torrents for review.">
      <div className="collection-toolbar app-card">
        <div>
          <h2 className="app-card-title">Queue</h2>
          <p className="app-card-subtitle">Submissions are reviewed by admins.</p>
        </div>
        <div className="collection-actions">
          <div className="collection-summary">
            <span className="app-pill">Pending {statusCounts.pending}</span>
            <span className="app-pill">Approved {statusCounts.approved}</span>
            <span className="app-pill">Rejected {statusCounts.rejected}</span>
          </div>
          <select
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value)}
            className="app-select"
          >
            <option value="pending">Pending</option>
            <option value="approved">Approved</option>
            <option value="rejected">Rejected</option>
            <option value="all">All</option>
          </select>
          <button type="button" className="app-btn" onClick={loadItems} disabled={loading}>
            {loading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </div>

      {error ? <div className="collection-error">{error}</div> : null}

      <section className="collection-grid">
        <form className="app-card collection-form" onSubmit={submitMagnet}>
          <div className="app-card-header">
            <h2 className="app-card-title">Magnet</h2>
            <span className="app-pill">link</span>
          </div>
          <label className="collection-label">
            <span>Magnet link</span>
            <textarea
              className="app-textarea"
              value={magnet}
              onChange={(event) => setMagnet(event.target.value)}
              placeholder="magnet:?xt=..."
              rows={3}
              disabled={submitting}
            />
          </label>
          <label className="collection-label">
            <span>Note (optional)</span>
            <input
              className="app-input"
              type="text"
              value={magnetNote}
              onChange={(event) => setMagnetNote(event.target.value)}
              placeholder="Series / season info"
              disabled={submitting}
            />
          </label>
          <button className="app-btn primary" type="submit" disabled={submitting}>
            Submit magnet
          </button>
        </form>

        <form className="app-card collection-form" onSubmit={submitTorrent}>
          <div className="app-card-header">
            <h2 className="app-card-title">Torrent</h2>
            <span className="app-pill">file</span>
          </div>
          <label className="collection-label">
            <span>Torrent file</span>
            <input
              className="app-input"
              type="file"
              accept=".torrent"
              onChange={(event) => setTorrentFile(event.target.files?.[0] ?? null)}
              disabled={submitting}
            />
          </label>
          <label className="collection-label">
            <span>Note (optional)</span>
            <input
              className="app-input"
              type="text"
              value={torrentNote}
              onChange={(event) => setTorrentNote(event.target.value)}
              placeholder="Resolution, subtitle group"
              disabled={submitting}
            />
          </label>
          <button className="app-btn primary" type="submit" disabled={submitting}>
            Submit torrent
          </button>
        </form>
      </section>

      <section className="collection-list">
        {items.length === 0 ? (
          <div className="collection-empty">
            {loading ? "Loading submissions..." : "No submissions yet."}
          </div>
        ) : null}
        {items.map((item) => (
          <div key={item.id} className="app-card collection-item">
            <div className="collection-item-header">
              <div className="collection-item-title">
                <strong>#{item.id}</strong>
                <span className={`collection-status status-${item.status}`}>
                  {item.status}
                </span>
              </div>
              <span className="app-pill">{item.kind}</span>
            </div>
            <div className="collection-item-body">
              <div className="collection-row">
                <span>Source</span>
                <span title={item.magnet ?? item.torrent_name ?? ""}>
                  {item.kind === "magnet"
                    ? compactText(item.magnet)
                    : compactText(item.torrent_name)}
                </span>
              </div>
              <div className="collection-row">
                <span>Submitter</span>
                <span>{item.submitter_id}</span>
              </div>
              <div className="collection-row">
                <span>Note</span>
                <span>{item.note || "--"}</span>
              </div>
              <div className="collection-row">
                <span>Submitted</span>
                <span>{formatDate(item.created_at)}</span>
              </div>
              <div className="collection-row">
                <span>Decision</span>
                <span>{item.decision_note || "--"}</span>
              </div>
              <div className="collection-row">
                <span>Decided by</span>
                <span>{item.decided_by || "--"}</span>
              </div>
              <div className="collection-row">
                <span>Decided at</span>
                <span>{formatDate(item.decided_at)}</span>
              </div>
            </div>
            {item.status === "pending" ? (
              <div className="collection-item-actions">
                <button
                  type="button"
                  className="app-btn ghost"
                  onClick={() => deleteItem(item.id)}
                  disabled={submitting}
                >
                  Delete
                </button>
              </div>
            ) : null}
          </div>
        ))}
      </section>
    </AppShell>
  );
}
