import { useEffect, useMemo, useState } from "react";
import "../Styles/theme.css";
import "../Styles/PageAdmin.css";
import FooterNote from "./Components/FooterNote";
import { apiFetch } from "../api";
import { useSession } from "../session";

interface AdminMetricsResponse {
  uptime_secs: number;
  media_count: number;
  media_total_bytes: number;
  job_counts: {
    queued: number;
    running: number;
    retry: number;
    done: number;
    failed: number;
  };
  system: {
    total_memory_bytes: number;
    used_memory_bytes: number;
    process_memory_bytes: number;
    cpu_usage_percent: number;
  };
  storage: {
    media_dir?: DiskUsage;
    cache_dir?: DiskUsage;
    qbittorrent_download_dir?: DiskUsage;
  };
  network: {
    rx_bytes: number;
    tx_bytes: number;
    rx_bytes_per_sec: number;
    tx_bytes_per_sec: number;
    interfaces: NetworkInterfaceMetrics[];
  };
  in_flight_requests: number;
  max_in_flight: number;
  qbittorrent?: QbittorrentTransferMetrics | null;
}

interface DiskUsage {
  mount_point: string;
  total_bytes: number;
  available_bytes: number;
}

interface NetworkInterfaceMetrics {
  name: string;
  rx_bytes: number;
  tx_bytes: number;
}

interface QbittorrentTransferMetrics {
  download_speed_bytes: number;
  upload_speed_bytes: number;
  download_total_bytes: number;
  upload_total_bytes: number;
  download_rate_limit: number;
  upload_rate_limit: number;
  dht_nodes: number;
  connection_status: string;
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

function formatUptime(seconds: number): string {
  const safe = Math.max(0, Math.floor(seconds));
  const days = Math.floor(safe / 86400);
  const hours = Math.floor((safe % 86400) / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  if (days > 0) {
    return `${days}d ${hours}h ${minutes}m`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

function formatPercent(value: number): string {
  if (!Number.isFinite(value)) return "--";
  return `${value.toFixed(1)}%`;
}

export default function PageAdmin() {
  const { session } = useSession();
  const [metrics, setMetrics] = useState<AdminMetricsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  const lastUpdatedLabel = useMemo(() => {
    if (!lastUpdated) return "Not loaded yet";
    return lastUpdated.toLocaleTimeString();
  }, [lastUpdated]);

  async function loadMetrics() {
    if (!session) return;
    setLoading(true);
    setError(null);
    try {
      const data = await apiFetch<AdminMetricsResponse>(
        "/api/admin/metrics",
        {},
        session.token
      );
      setMetrics(data);
      setLastUpdated(new Date());
    } catch (err) {
      setError((err as Error).message || "Failed to load metrics.");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadMetrics();
  }, [session]);

  const jobStats = metrics?.job_counts;
  const system = metrics?.system;
  const storage = metrics?.storage;
  const network = metrics?.network;
  const qbittorrent = metrics?.qbittorrent ?? undefined;

  return (
    <div className="admin-shell">
      <header className="admin-header">
        <div>
          <p className="admin-kicker">Admin</p>
          <h1 className="admin-title">System status</h1>
          <p className="admin-subtitle">Last updated: {lastUpdatedLabel}</p>
        </div>
        <div className="admin-actions">
          <button
            type="button"
            className="admin-btn"
            onClick={loadMetrics}
            disabled={loading}
          >
            {loading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
      </header>

      {error ? <div className="admin-error">{error}</div> : null}

      <section className="admin-grid">
        <div className="admin-card">
          <h2>Library</h2>
          <div className="admin-metric">
            <span>Total media</span>
            <strong>{metrics ? metrics.media_count : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Media size</span>
            <strong>{metrics ? formatBytes(metrics.media_total_bytes) : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Uptime</span>
            <strong>{metrics ? formatUptime(metrics.uptime_secs) : "--"}</strong>
          </div>
        </div>

        <div className="admin-card">
          <h2>Requests</h2>
          <div className="admin-metric">
            <span>In-flight</span>
            <strong>{metrics ? metrics.in_flight_requests : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Max in-flight</span>
            <strong>{metrics ? metrics.max_in_flight : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>CPU usage</span>
            <strong>{system ? formatPercent(system.cpu_usage_percent) : "--"}</strong>
          </div>
        </div>

        <div className="admin-card">
          <h2>Jobs</h2>
          <div className="admin-metric">
            <span>Queued</span>
            <strong>{jobStats ? jobStats.queued : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Running</span>
            <strong>{jobStats ? jobStats.running : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Retry / Failed</span>
            <strong>{jobStats ? `${jobStats.retry} / ${jobStats.failed}` : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Done</span>
            <strong>{jobStats ? jobStats.done : "--"}</strong>
          </div>
        </div>

        <div className="admin-card">
          <h2>Memory</h2>
          <div className="admin-metric">
            <span>System used</span>
            <strong>{system ? formatBytes(system.used_memory_bytes) : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>System total</span>
            <strong>{system ? formatBytes(system.total_memory_bytes) : "--"}</strong>
          </div>
          <div className="admin-metric">
            <span>Process</span>
            <strong>{system ? formatBytes(system.process_memory_bytes) : "--"}</strong>
          </div>
        </div>
      </section>

      <section className="admin-grid admin-grid-wide">
        <div className="admin-card">
          <h2>Storage</h2>
          <div className="admin-metric">
            <span>Media dir</span>
            <strong>
              {storage?.media_dir
                ? `${formatBytes(storage.media_dir.available_bytes)} free`
                : "--"}
            </strong>
          </div>
          <div className="admin-submetric">
            {storage?.media_dir
              ? `Mount: ${storage.media_dir.mount_point} / ${formatBytes(storage.media_dir.total_bytes)}`
              : "Media disk info unavailable"}
          </div>
          <div className="admin-metric">
            <span>Cache dir</span>
            <strong>
              {storage?.cache_dir
                ? `${formatBytes(storage.cache_dir.available_bytes)} free`
                : "--"}
            </strong>
          </div>
          <div className="admin-submetric">
            {storage?.cache_dir
              ? `Mount: ${storage.cache_dir.mount_point} / ${formatBytes(storage.cache_dir.total_bytes)}`
              : "Cache disk info unavailable"}
          </div>
          <div className="admin-metric">
            <span>qBittorrent dir</span>
            <strong>
              {storage?.qbittorrent_download_dir
                ? `${formatBytes(storage.qbittorrent_download_dir.available_bytes)} free`
                : "--"}
            </strong>
          </div>
          <div className="admin-submetric">
            {storage?.qbittorrent_download_dir
              ? `Mount: ${storage.qbittorrent_download_dir.mount_point} / ${formatBytes(storage.qbittorrent_download_dir.total_bytes)}`
              : "qBittorrent disk info unavailable"}
          </div>
        </div>

        <div className="admin-card">
          <h2>Network</h2>
          <div className="admin-metric">
            <span>Total RX / TX</span>
            <strong>
              {network
                ? `${formatBytes(network.rx_bytes)} / ${formatBytes(network.tx_bytes)}`
                : "--"}
            </strong>
          </div>
          <div className="admin-metric">
            <span>RX / TX rate</span>
            <strong>
              {network
                ? `${formatBytes(network.rx_bytes_per_sec)}/s / ${formatBytes(
                    network.tx_bytes_per_sec
                  )}/s`
                : "--"}
            </strong>
          </div>
          <div className="admin-list">
            {network?.interfaces.length ? (
              network.interfaces.map((iface) => (
                <div key={iface.name} className="admin-list-item">
                  <span>{iface.name}</span>
                  <span>
                    {formatBytes(iface.rx_bytes)} / {formatBytes(iface.tx_bytes)}
                  </span>
                </div>
              ))
            ) : (
              <div className="admin-submetric">No interfaces reported.</div>
            )}
          </div>
        </div>
      </section>

      <section className="admin-grid">
        <div className="admin-card">
          <h2>qBittorrent</h2>
          {qbittorrent ? (
            <>
              <div className="admin-metric">
                <span>Status</span>
                <strong>{qbittorrent.connection_status || "--"}</strong>
              </div>
              <div className="admin-metric">
                <span>Speed</span>
                <strong>
                  {formatBytes(qbittorrent.download_speed_bytes)}/s down ·{" "}
                  {formatBytes(qbittorrent.upload_speed_bytes)}/s up
                </strong>
              </div>
              <div className="admin-metric">
                <span>Total</span>
                <strong>
                  {formatBytes(qbittorrent.download_total_bytes)} down ·{" "}
                  {formatBytes(qbittorrent.upload_total_bytes)} up
                </strong>
              </div>
              <div className="admin-metric">
                <span>Rate limits</span>
                <strong>
                  {qbittorrent.download_rate_limit > 0
                    ? formatBytes(qbittorrent.download_rate_limit)
                    : "∞"}{" "}
                  /{" "}
                  {qbittorrent.upload_rate_limit > 0
                    ? formatBytes(qbittorrent.upload_rate_limit)
                    : "∞"}
                </strong>
              </div>
              <div className="admin-metric">
                <span>DHT nodes</span>
                <strong>{qbittorrent.dht_nodes}</strong>
              </div>
            </>
          ) : (
            <div className="admin-submetric">qBittorrent not configured.</div>
          )}
        </div>
      </section>

      <FooterNote className="admin-footer" />
    </div>
  );
}
