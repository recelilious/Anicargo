import type {
  AdminAuthResponse,
  AdminDashboardResponse,
  AuthResponse,
  BootstrapResponse,
  CalendarResponse,
  EpisodePlaybackResponse,
  Policy,
  SearchResponse,
  SubjectDetailResponse
} from "./types";

const API_BASE = import.meta.env.VITE_API_BASE_URL ?? "";

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
  return `${API_BASE}${path}`;
}

export function fetchBootstrap(deviceId: string, userToken: string | null) {
  return request<BootstrapResponse>("/api/public/bootstrap", {}, deviceId, userToken ?? undefined);
}

export function fetchCalendar(deviceId: string, userToken: string | null) {
  return request<CalendarResponse>("/api/public/calendar", {}, deviceId, userToken ?? undefined);
}

export function searchSubjects(params: URLSearchParams, deviceId: string, userToken: string | null) {
  return request<SearchResponse>(`/api/public/search?${params.toString()}`, {}, deviceId, userToken ?? undefined);
}

export function fetchSubjectDetail(subjectId: number, deviceId: string, userToken: string | null) {
  return request<SubjectDetailResponse>(`/api/public/subjects/${subjectId}`, {}, deviceId, userToken ?? undefined);
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
