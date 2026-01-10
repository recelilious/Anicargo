import { useEffect, useMemo, useState } from "react";
import "../Styles/theme.css";
import "../Styles/PageCollection.css";
import FooterNote from "./Components/FooterNote";
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

  const isAdmin = useMemo(() => (session?.roleLevel ?? 0) >= 3, [session]);

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

  async function approveItem(id: number) {
    if (!session) return;
    setSubmitting(true);
    setError(null);
    try {
      await apiFetch<CollectionCreateResponse>(
        `/api/collection/${id}/approve`,
        { method: "POST" },
        session.token
      );
      await loadItems();
    } catch (err) {
      setError((err as Error).message || "Failed to approve submission.");
    } finally {
      setSubmitting(false);
    }
  }

  async function rejectItem(id: number) {
    if (!session) return;
    setSubmitting(true);
    setError(null);
    try {
      await apiFetch<CollectionCreateResponse>(
        `/api/collection/${id}/reject`,
        { method: "POST" },
        session.token
      );
      await loadItems();
    } catch (err) {
      setError((err as Error).message || "Failed to reject submission.");
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

  return (
    <div className="collection-shell">
      <header className="collection-header">
        <div>
          <p className="collection-kicker">Collector</p>
          <h1 className="collection-title">Resource intake</h1>
          <p className="collection-subtitle">Submit magnets or torrents for review.</p>
        </div>
        <div className="collection-actions">
          <select
            value={statusFilter}
            onChange={(event) => setStatusFilter(event.target.value)}
            className="collection-select"
          >
            <option value="pending">Pending</option>
            <option value="approved">Approved</option>
            <option value="rejected">Rejected</option>
            <option value="all">All</option>
          </select>
          <button
            type="button"
            className="collection-btn"
            onClick={loadItems}
            disabled={loading}
          >
            {loading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </header>

      {error ? <div className="collection-error">{error}</div> : null}

      <section className="collection-grid">
        <form className="collection-card" onSubmit={submitMagnet}>
          <h2>Magnet</h2>
          <label>
            <span>Magnet link</span>
            <textarea
              value={magnet}
              onChange={(event) => setMagnet(event.target.value)}
              placeholder="magnet:?xt=..."
              rows={3}
              disabled={submitting}
            />
          </label>
          <label>
            <span>Note (optional)</span>
            <input
              type="text"
              value={magnetNote}
              onChange={(event) => setMagnetNote(event.target.value)}
              placeholder="Series / season info"
              disabled={submitting}
            />
          </label>
          <button className="collection-btn solid" type="submit" disabled={submitting}>
            Submit magnet
          </button>
        </form>

        <form className="collection-card" onSubmit={submitTorrent}>
          <h2>Torrent</h2>
          <label>
            <span>Torrent file</span>
            <input
              type="file"
              accept=".torrent"
              onChange={(event) => setTorrentFile(event.target.files?.[0] ?? null)}
              disabled={submitting}
            />
          </label>
          <label>
            <span>Note (optional)</span>
            <input
              type="text"
              value={torrentNote}
              onChange={(event) => setTorrentNote(event.target.value)}
              placeholder="Resolution, subtitle group"
              disabled={submitting}
            />
          </label>
          <button className="collection-btn solid" type="submit" disabled={submitting}>
            Submit torrent
          </button>
        </form>
      </section>

      <section className="collection-list">
        {items.length === 0 && !loading ? (
          <div className="collection-empty">No submissions yet.</div>
        ) : null}
        {items.map((item) => (
          <div key={item.id} className="collection-item">
            <div className="collection-item-header">
              <div>
                <strong>#{item.id}</strong>
                <span className={`collection-status status-${item.status}`}>
                  {item.status}
                </span>
              </div>
              <span className="collection-kind">{item.kind}</span>
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
            </div>
            <div className="collection-item-actions">
              {item.status === "pending" ? (
                <>
                  {isAdmin ? (
                    <>
                      <button
                        type="button"
                        className="collection-btn"
                        onClick={() => approveItem(item.id)}
                        disabled={submitting}
                      >
                        Approve
                      </button>
                      <button
                        type="button"
                        className="collection-btn ghost"
                        onClick={() => rejectItem(item.id)}
                        disabled={submitting}
                      >
                        Reject
                      </button>
                    </>
                  ) : null}
                  <button
                    type="button"
                    className="collection-btn ghost"
                    onClick={() => deleteItem(item.id)}
                    disabled={submitting}
                  >
                    Delete
                  </button>
                </>
              ) : isAdmin ? (
                <button
                  type="button"
                  className="collection-btn ghost"
                  onClick={() => deleteItem(item.id)}
                  disabled={submitting}
                >
                  Remove
                </button>
              ) : null}
            </div>
          </div>
        ))}
      </section>

      <FooterNote className="collection-footer" />
    </div>
  );
}
