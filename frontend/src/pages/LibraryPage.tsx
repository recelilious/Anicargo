import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { apiFetch } from "../api";
import { useSession } from "../session";

interface MediaEntry {
  id: string;
  filename: string;
  size: number;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(1)} ${units[index]}`;
}

export default function LibraryPage() {
  const { session } = useSession();
  const [library, setLibrary] = useState<MediaEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    void refreshLibrary();
  }, []);

  async function refreshLibrary() {
    if (!session?.token) {
      return;
    }
    setLoading(true);
    setStatus(null);
    try {
      const entries = await apiFetch<MediaEntry[]>("/api/library", {}, session.token);
      setLibrary(entries);
    } catch (error) {
      setStatus((error as Error).message);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="grid">
      <section className="panel span-7">
        <div className="panel-header">
          <h2>Library</h2>
          <div className="panel-actions">
            <button className="ghost" type="button" onClick={refreshLibrary}>
              {loading ? "Refreshing..." : "Refresh"}
            </button>
            <span className="count">{library.length} titles</span>
          </div>
        </div>
        <div className="list">
          {library.map((entry, index) => (
            <Link
              key={entry.id}
              to={`/player/${entry.id}`}
              className="media-item"
              style={{ "--delay": `${index * 50}ms` } as React.CSSProperties}
            >
              <div>
                <span className="title">{entry.filename}</span>
                <span className="meta">{formatBytes(entry.size)}</span>
              </div>
              <span className="pill">Play</span>
            </Link>
          ))}
          {library.length === 0 && !loading ? (
            <div className="empty">No media found in the directory.</div>
          ) : null}
        </div>
        {status ? <p className="status-line">{status}</p> : null}
      </section>

      <section className="panel span-5">
        <h2>Library notes</h2>
        <p className="subtitle">
          Every title is served as HLS. Select a file to open the player and
          request a fresh playlist with your token.
        </p>
        <div className="stack">
          <div className="stack-item">
            <h3>Auto refresh</h3>
            <p>New files appear after a scan. Use Refresh to pull again.</p>
          </div>
          <div className="stack-item">
            <h3>Direct playback</h3>
            <p>Use the copy button in the player if you prefer mpv or VLC.</p>
          </div>
          <div className="stack-item">
            <h3>Protected streams</h3>
            <p>Segment URLs inherit the token for consistent access.</p>
          </div>
        </div>
      </section>
    </div>
  );
}
