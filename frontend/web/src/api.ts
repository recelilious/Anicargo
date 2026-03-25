import type {
  ActiveDownloadsResponse,
  AdminAuthResponse,
  AdminDashboardResponse,
  AdminRuntimeResponse,
  AuthResponse,
  BootstrapResponse,
  CalendarResponse,
  CatalogManifestResponse,
  CatalogPageResponse,
  DownloadExecution,
  DownloadExecutionEvent,
  DownloadJob,
  EpisodePlaybackResponse,
  PlaybackHistoryResponse,
  Policy,
  ResourceCandidate,
  ResourceLibraryResponse,
  SearchResponse,
  SubjectCollectionResponse,
  SubjectDownloadStatus,
  SubjectDetailResponse
} from "./types";

const API_BASE = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/+$/, "");

type Envelope<T> = {
  data: T;
};

async function request<T>(path: string, options: RequestInit = {}, deviceId?: string, userToken?: string, adminToken?: string): Promise<T> {
  const headers = new Headers(options.headers ?? {});

  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }

  if (deviceId) {
    headers.set("x-anicargo-device-id", deviceId);
  }

  if (userToken) {
    headers.set("Authorization", `Bearer ${userToken}`);
  }

  if (adminToken) {
    headers.set("x-anicargo-admin-token", adminToken);
  }

  const response = await fetch(buildApiUrl(path), {
    ...options,
    headers
  });

  if (!response.ok) {
    const errorBody = (await response.json().catch(() => null)) as { message?: string } | null;
    throw new Error(errorBody?.message ?? `Request failed: ${response.status}`);
  }

  const payload = (await response.json()) as Envelope<T>;
  return payload.data;
}

export function buildApiUrl(path: string) {
  if (/^https?:\/\//.test(path)) {
    return path;
  }

  return `${API_BASE}${path}`;
}

export function fetchBootstrap(deviceId: string, userToken: string | null) {
  return request<BootstrapResponse>("/api/public/bootstrap", {}, deviceId, userToken ?? undefined);
}

export function fetchCalendar(
  deviceId: string,
  userToken: string | null,
  options?: { timezone?: string; deepNightMode?: boolean }
) {
  const params = new URLSearchParams();
  if (options?.timezone) {
    params.set("timezone", options.timezone);
  }
  if (typeof options?.deepNightMode === "boolean") {
    params.set("deepNightMode", String(options.deepNightMode));
  }

  const suffix = params.size > 0 ? `?${params.toString()}` : "";
  return request<CalendarResponse>(
    `/api/public/calendar${suffix}`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function fetchCatalogManifest(deviceId: string, userToken: string | null) {
  return request<CatalogManifestResponse>(
    "/api/public/catalogs/manifest",
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function fetchCatalogPage(kind: "preview" | "special", deviceId: string, userToken: string | null) {
  return request<CatalogPageResponse>(
    `/api/public/catalogs/${kind}`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function searchSubjects(params: URLSearchParams, deviceId: string, userToken: string | null) {
  return request<SearchResponse>(`/api/public/search?${params.toString()}`, {}, deviceId, userToken ?? undefined);
}

export function fetchSubjectDetail(subjectId: number, deviceId: string, userToken: string | null) {
  return request<SubjectDetailResponse>(`/api/public/subjects/${subjectId}`, {}, deviceId, userToken ?? undefined);
}

export function fetchSubjectDownloadStatus(subjectId: number, deviceId: string, userToken: string | null) {
  return request<SubjectDownloadStatus | null>(
    `/api/public/subjects/${subjectId}/download-status`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function fetchEpisodePlayback(subjectId: number, episodeId: number, deviceId: string, userToken: string | null) {
  return request<EpisodePlaybackResponse>(
    `/api/public/subjects/${subjectId}/episodes/${episodeId}/playback`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function toggleSubscription(subjectId: number, deviceId: string, userToken: string | null) {
  return request<{ bangumiSubjectId: number; subscription: SubjectDetailResponse["subscription"] }>(
    `/api/public/subscriptions/${subjectId}/toggle`,
    { method: "POST" },
    deviceId,
    userToken ?? undefined
  );
}

export function fetchSubscriptions(params: URLSearchParams, deviceId: string, userToken: string | null) {
  return request<SubjectCollectionResponse>(
    `/api/public/subscriptions?${params.toString()}`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function fetchPlaybackHistory(params: URLSearchParams, deviceId: string, userToken: string | null) {
  return request<PlaybackHistoryResponse>(
    `/api/public/history?${params.toString()}`,
    {},
    deviceId,
    userToken ?? undefined
  );
}

export function recordPlaybackHistory(
  payload: { bangumiSubjectId: number; bangumiEpisodeId: number; mediaInventoryId: number },
  deviceId: string,
  userToken: string | null
) {
  return request<boolean>(
    "/api/public/history/playback",
    {
      method: "POST",
      body: JSON.stringify(payload)
    },
    deviceId,
    userToken ?? undefined
  );
}

export function register(username: string, password: string) {
  return request<AuthResponse>("/api/auth/register", {
    method: "POST",
    body: JSON.stringify({ username, password })
  });
}

export function login(username: string, password: string) {
  return request<AuthResponse>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password })
  });
}

export function logout(userToken: string) {
  return request<boolean>("/api/auth/logout", { method: "POST" }, undefined, userToken);
}

export function adminLogin(username: string, password: string) {
  return request<AdminAuthResponse>("/api/admin/login", {
    method: "POST",
    body: JSON.stringify({ username, password })
  });
}

export function fetchAdminDashboard(deviceId: string, adminToken: string) {
  return request<AdminDashboardResponse>("/api/admin/dashboard", {}, deviceId, undefined, adminToken);
}

export function fetchAdminRuntime(deviceId: string, adminToken: string) {
  return request<AdminRuntimeResponse>("/api/admin/runtime", {}, deviceId, undefined, adminToken);
}

export function fetchAdminDownloads(deviceId: string, adminToken: string) {
  return request<{ items: DownloadJob[] }>("/api/admin/downloads", {}, deviceId, undefined, adminToken);
}

export function fetchAdminDownloadCandidates(deviceId: string, adminToken: string, jobId: number) {
  return request<{ downloadJobId: number; items: ResourceCandidate[] }>(
    `/api/admin/downloads/${jobId}/candidates`,
    {},
    deviceId,
    undefined,
    adminToken
  );
}

export function fetchAdminDownloadExecutions(deviceId: string, adminToken: string, jobId: number) {
  return request<{ downloadJobId: number; items: DownloadExecution[] }>(
    `/api/admin/downloads/${jobId}/executions`,
    {},
    deviceId,
    undefined,
    adminToken
  );
}

export function fetchAdminExecutionEvents(deviceId: string, adminToken: string, executionId: number) {
  return request<{ downloadExecutionId: number; items: DownloadExecutionEvent[] }>(
    `/api/admin/executions/${executionId}/events`,
    {},
    deviceId,
    undefined,
    adminToken
  );
}

export function forceAdminDownload(deviceId: string, adminToken: string, subjectId: number) {
  return request<{ bangumiSubjectId: number }>(
    `/api/admin/downloads/${subjectId}/force`,
    { method: "POST" },
    deviceId,
    undefined,
    adminToken
  );
}

export function activateAdminDownload(deviceId: string, adminToken: string, jobId: number) {
  return request<{ downloadJobId: number }>(
    `/api/admin/downloads/${jobId}/execute`,
    { method: "POST" },
    deviceId,
    undefined,
    adminToken
  );
}

export function fetchResources(params: URLSearchParams, deviceId: string, userToken: string | null) {
  return request<ResourceLibraryResponse>(`/api/public/resources?${params.toString()}`, {}, deviceId, userToken ?? undefined);
}

export function fetchActiveDownloads(deviceId: string, userToken: string | null) {
  return request<ActiveDownloadsResponse>("/api/public/downloads/active", {}, deviceId, userToken ?? undefined);
}

export function updatePolicy(deviceId: string, adminToken: string, policy: Policy) {
  return request<Policy>(
    "/api/admin/policy",
    {
      method: "PUT",
      body: JSON.stringify(policy)
    },
    deviceId,
    undefined,
    adminToken
  );
}

export function createFansubRule(
  deviceId: string,
  adminToken: string,
  payload: { fansubName: string; localePreference: string; priority: number; isBlacklist: boolean }
) {
  return request<AdminDashboardResponse["fansubRules"][number]>(
    "/api/admin/fansub-rules",
    {
      method: "POST",
      body: JSON.stringify(payload)
    },
    deviceId,
    undefined,
    adminToken
  );
}

export function adminLogout(deviceId: string, adminToken: string) {
  return request<boolean>("/api/admin/logout", { method: "POST" }, deviceId, undefined, adminToken);
}
