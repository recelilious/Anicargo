import { Fragment, useEffect, useMemo, useState } from "react";
import AppShell from "../Components/AppShell";
import "../Styles/PageAdmin.css";
import { apiFetch, apiFetchEmpty } from "../api";
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

interface AdminJob {
  id: number;
  job_type: string;
  status: string;
  attempts: number;
  max_attempts: number;
  payload?: Record<string, unknown> | null;
  result?: unknown;
  last_error?: string | null;
  scheduled_at: string;
  locked_at?: string | null;
  locked_by?: string | null;
  created_at: string;
  updated_at: string;
}

interface AdminJobsResponse {
  jobs: AdminJob[];
}

interface UserRow {
  user_id: string;
  role: string;
  role_level: number;
  created_at: string;
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

function roleLabel(level: number): string {
  if (level >= 5) return "Admin 5";
  if (level === 4) return "Admin 4";
  if (level === 3) return "Admin 3";
  if (level === 2) return "User 2";
  return "User 1";
}

function formatJson(value: unknown): string {
  if (value === null || value === undefined) return "--";
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function formatDate(value?: string | null): string {
  if (!value) return "--";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

export default function PageAdmin() {
  const { session } = useSession();
  const [metrics, setMetrics] = useState<AdminMetricsResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const [jobs, setJobs] = useState<AdminJob[]>([]);
  const [jobStatus, setJobStatus] = useState("running");
  const [jobLimit, setJobLimit] = useState(50);
  const [jobLoading, setJobLoading] = useState(false);
  const [jobError, setJobError] = useState<string | null>(null);
  const [expandedJobs, setExpandedJobs] = useState<Record<number, boolean>>({});
  const [users, setUsers] = useState<UserRow[]>([]);
  const [userLoading, setUserLoading] = useState(false);
  const [userError, setUserError] = useState<string | null>(null);
  const [userEdits, setUserEdits] = useState<Record<string, number>>({});

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

  async function loadJobs() {
    if (!session) return;
    setJobLoading(true);
    setJobError(null);
    try {
      const limit = Math.min(500, Math.max(1, jobLimit));
      const params = new URLSearchParams();
      if (jobStatus !== "all") {
        params.set("status", jobStatus);
      }
      params.set("limit", limit.toString());
      const data = await apiFetch<AdminJobsResponse>(
        `/api/admin/jobs?${params.toString()}`,
        {},
        session.token
      );
      setJobs(data.jobs);
    } catch (err) {
      setJobError((err as Error).message || "Failed to load jobs.");
    } finally {
      setJobLoading(false);
    }
  }

  async function loadUsers() {
    if (!session) return;
    setUserLoading(true);
    setUserError(null);
    try {
      const data = await apiFetch<UserRow[]>("/api/users", {}, session.token);
      setUsers(data);
      setUserEdits((current) => {
        const next = { ...current };
        data.forEach((user) => {
          if (!(user.user_id in next)) {
            next[user.user_id] = user.role_level;
          }
        });
        Object.keys(next).forEach((key) => {
          if (!data.some((user) => user.user_id === key)) {
            delete next[key];
          }
        });
        return next;
      });
    } catch (err) {
      setUserError((err as Error).message || "Failed to load users.");
    } finally {
      setUserLoading(false);
    }
  }

  async function updateUserRole(user: UserRow) {
    if (!session) return;
    const targetLevel = userEdits[user.user_id] ?? user.role_level;
    if (targetLevel === user.role_level) return;
    if (targetLevel >= session.roleLevel || user.user_id === session.userId) {
      setUserError("Insufficient permissions to change this user.");
      return;
    }
    setUserLoading(true);
    setUserError(null);
    try {
      await apiFetch<unknown>(
        `/api/users/${encodeURIComponent(user.user_id)}/role`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json"
          },
          body: JSON.stringify({ role_level: targetLevel })
        },
        session.token
      );
      await loadUsers();
    } catch (err) {
      setUserError((err as Error).message || "Failed to update role.");
    } finally {
      setUserLoading(false);
    }
  }

  async function deleteUser(user: UserRow) {
    if (!session) return;
    if (user.user_id === session.userId) {
      setUserError("Cannot delete current user.");
      return;
    }
    if (user.role_level >= session.roleLevel) {
      setUserError("Insufficient permissions to delete this user.");
      return;
    }
    setUserLoading(true);
    setUserError(null);
    try {
      await apiFetchEmpty(`/api/users/${encodeURIComponent(user.user_id)}`, {}, session.token);
      await loadUsers();
    } catch (err) {
      setUserError((err as Error).message || "Failed to delete user.");
    } finally {
      setUserLoading(false);
    }
  }

  function toggleJobDetail(id: number) {
    setExpandedJobs((current) => ({
      ...current,
      [id]: !current[id]
    }));
  }

  useEffect(() => {
    loadMetrics();
  }, [session]);

  useEffect(() => {
    loadJobs();
  }, [session, jobStatus, jobLimit]);

  useEffect(() => {
    loadUsers();
  }, [session]);

  const jobStats = metrics?.job_counts;
  const system = metrics?.system;
  const storage = metrics?.storage;
  const network = metrics?.network;
  const qbittorrent = metrics?.qbittorrent ?? undefined;
  const maxAssignable = Math.max(0, (session?.roleLevel ?? 0) - 1);
  const assignableLevels = Array.from({ length: maxAssignable }, (_, index) => index + 1);

  return (
    <AppShell
      title="Admin"
      subtitle={`Last updated: ${lastUpdatedLabel}`}
      actions={(
        <button type="button" className="app-btn" onClick={loadMetrics} disabled={loading}>
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      )}
    >
      {error ? <div className="admin-error">{error}</div> : null}

      <section className="admin-grid">
        <div className="app-card">
          <h2 className="app-card-title">Library</h2>
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

        <div className="app-card">
          <h2 className="app-card-title">Requests</h2>
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

        <div className="app-card">
          <h2 className="app-card-title">Jobs</h2>
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

        <div className="app-card">
          <h2 className="app-card-title">Memory</h2>
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
        <div className="app-card">
          <h2 className="app-card-title">Storage</h2>
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

        <div className="app-card">
          <h2 className="app-card-title">Network</h2>
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
        <div className="app-card">
          <h2 className="app-card-title">qBittorrent</h2>
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

      <section className="app-card admin-jobs">
        <div className="admin-jobs-header">
          <div>
            <h2 className="app-card-title">Jobs</h2>
            <p className="app-card-subtitle">Queue activity and background work.</p>
          </div>
          <div className="admin-jobs-toolbar">
            <select
              className="app-select"
              value={jobStatus}
              onChange={(event) => setJobStatus(event.target.value)}
            >
              <option value="all">All</option>
              <option value="queued">Queued</option>
              <option value="running">Running</option>
              <option value="retry">Retry</option>
              <option value="done">Done</option>
              <option value="failed">Failed</option>
            </select>
            <input
              className="app-input"
              type="number"
              min={1}
              max={500}
              value={jobLimit}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value)) {
                  setJobLimit(value);
                }
              }}
            />
            <button
              type="button"
              className="app-btn"
              onClick={loadJobs}
              disabled={jobLoading}
            >
              {jobLoading ? "Refreshing..." : "Refresh"}
            </button>
          </div>
        </div>
        {jobError ? <div className="admin-error">{jobError}</div> : null}
        {jobs.length === 0 ? (
          <div className="admin-submetric">No jobs found.</div>
        ) : (
          <div className="admin-jobs-table">
            <table className="app-table">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Type</th>
                  <th>Status</th>
                  <th>Attempts</th>
                  <th>Updated</th>
                  <th>Error</th>
                  <th>Details</th>
                </tr>
              </thead>
              <tbody>
                {jobs.map((job) => {
                  const expanded = Boolean(expandedJobs[job.id]);
                  return (
                    <Fragment key={job.id}>
                      <tr>
                        <td>{job.id}</td>
                        <td>{job.job_type}</td>
                        <td>{job.status}</td>
                        <td>
                          {job.attempts} / {job.max_attempts}
                        </td>
                        <td>{formatDate(job.updated_at)}</td>
                        <td className="admin-job-error">{job.last_error || "--"}</td>
                        <td>
                          <button
                            type="button"
                            className="app-btn ghost"
                            onClick={() => toggleJobDetail(job.id)}
                          >
                            {expanded ? "Hide" : "Show"}
                          </button>
                        </td>
                      </tr>
                      {expanded ? (
                        <tr className="admin-job-detail">
                          <td colSpan={7}>
                            <div className="admin-job-detail-grid">
                              <div>
                                <div className="admin-job-label">Scheduled</div>
                                <div>{formatDate(job.scheduled_at)}</div>
                              </div>
                              <div>
                                <div className="admin-job-label">Created</div>
                                <div>{formatDate(job.created_at)}</div>
                              </div>
                              <div>
                                <div className="admin-job-label">Locked</div>
                                <div>{formatDate(job.locked_at)}</div>
                              </div>
                              <div>
                                <div className="admin-job-label">Worker</div>
                                <div>{job.locked_by || "--"}</div>
                              </div>
                            </div>
                            <div className="admin-job-detail-block">
                              <div className="admin-job-label">Payload</div>
                              <pre className="admin-job-pre">{formatJson(job.payload)}</pre>
                            </div>
                            <div className="admin-job-detail-block">
                              <div className="admin-job-label">Result</div>
                              <pre className="admin-job-pre">{formatJson(job.result)}</pre>
                            </div>
                          </td>
                        </tr>
                      ) : null}
                    </Fragment>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="app-card admin-users">
        <div className="admin-users-header">
          <div>
            <h2 className="app-card-title">Users</h2>
            <p className="app-card-subtitle">Manage roles and access levels.</p>
          </div>
          <button
            type="button"
            className="app-btn"
            onClick={loadUsers}
            disabled={userLoading}
          >
            {userLoading ? "Refreshing..." : "Refresh"}
          </button>
        </div>
        {userError ? <div className="admin-error">{userError}</div> : null}
        {users.length === 0 ? (
          <div className="admin-submetric">No users found.</div>
        ) : (
          <div className="admin-users-table">
            <table className="app-table">
              <thead>
                <tr>
                  <th>User</th>
                  <th>Role</th>
                  <th>Level</th>
                  <th>Created</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => {
                  const isSelf = user.user_id === session?.userId;
                  const canManage = !isSelf && user.role_level < (session?.roleLevel ?? 0);
                  const selectedLevel = userEdits[user.user_id] ?? user.role_level;
                  return (
                    <tr key={user.user_id}>
                      <td>
                        {user.user_id}
                        {isSelf ? <span className="app-pill">you</span> : null}
                      </td>
                      <td>{user.role}</td>
                      <td>{roleLabel(user.role_level)}</td>
                      <td>{formatDate(user.created_at)}</td>
                      <td>
                        <div className="admin-user-actions">
                          {canManage ? (
                            <>
                              <select
                                className="app-select"
                                value={selectedLevel}
                                onChange={(event) =>
                                  setUserEdits((current) => ({
                                    ...current,
                                    [user.user_id]: Number(event.target.value)
                                  }))
                                }
                              >
                                {assignableLevels.map((level) => (
                                  <option key={level} value={level}>
                                    {roleLabel(level)}
                                  </option>
                                ))}
                              </select>
                              <button
                                type="button"
                                className="app-btn ghost"
                                onClick={() => updateUserRole(user)}
                                disabled={userLoading || selectedLevel === user.role_level}
                              >
                                Apply
                              </button>
                              <button
                                type="button"
                                className="app-btn ghost"
                                onClick={() => deleteUser(user)}
                                disabled={userLoading}
                              >
                                Delete
                              </button>
                            </>
                          ) : (
                            <span className="admin-submetric">No permission</span>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </AppShell>
  );
}
