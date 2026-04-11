import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

import { adminLogin, adminLogout, fetchBootstrap, login, logout, register } from "./api";
import type { BootstrapResponse, ViewerSummary } from "./types";

const DEVICE_KEY = "anicargo.device_id";
const GUEST_NAME_KEY = "anicargo.guest_name";
const USER_TOKEN_KEY = "anicargo.user_token";
const ADMIN_TOKEN_KEY = "anicargo.admin_token";
const ADMIN_NAME_KEY = "anicargo.admin_username";
const DEEP_NIGHT_MODE_KEY = "anicargo.deep_night_mode";
const GUEST_PREFIXES = ["晨星", "雾海", "白塔", "晴岚", "落樱", "月砂", "霜原", "潮音"];
const GUEST_SUFFIXES = ["旅人", "观测者", "追番者", "记录员", "领航员", "收藏家", "放映员", "信使"];

type SessionContextValue = {
  bootstrap: BootstrapResponse | null;
  deviceId: string;
  guestName: string;
  userToken: string | null;
  adminToken: string | null;
  adminUsername: string | null;
  systemTimeZone: string;
  deepNightMode: boolean;
  displayName: string;
  viewerModeLabel: string;
  viewerSubline: string;
  isGuestViewer: boolean;
  isAdmin: boolean;
  isReady: boolean;
  refresh: () => Promise<void>;
  setDeepNightMode: (next: boolean) => void;
  registerAccount: (username: string, password: string) => Promise<void>;
  loginAccount: (username: string, password: string) => Promise<void>;
  logoutAccount: () => Promise<void>;
  loginAdmin: (username: string, password: string) => Promise<void>;
  logoutAdmin: () => Promise<void>;
  setViewerFromAuth: (viewer: ViewerSummary, token: string) => void;
};

const SessionContext = createContext<SessionContextValue | null>(null);

function safeLocalStorageGet(key: string) {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeLocalStorageSet(key: string, value: string) {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // ignore storage write failures on restricted browsers
  }
}

function safeLocalStorageRemove(key: string) {
  try {
    window.localStorage.removeItem(key);
  } catch {
    // ignore storage write failures on restricted browsers
  }
}

function fallbackDeviceId() {
  const timestamp = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 10);
  const extra = Math.random().toString(36).slice(2, 10);
  return `device-${timestamp}-${random}${extra}`;
}

function ensureDeviceId() {
  const existing = safeLocalStorageGet(DEVICE_KEY);
  if (existing) {
    return existing;
  }

  const next =
    globalThis.crypto?.randomUUID?.() ??
    (() => {
      const buffer = globalThis.crypto?.getRandomValues?.(new Uint32Array(4));
      if (!buffer) {
        return fallbackDeviceId();
      }

      return `device-${Array.from(buffer)
        .map((value) => value.toString(16).padStart(8, "0"))
        .join("")}`;
    })();

  safeLocalStorageSet(DEVICE_KEY, next);
  return next;
}

function hashText(value: string) {
  let hash = 0;

  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) >>> 0;
  }

  return hash;
}

function createGuestName(deviceId: string) {
  const hash = hashText(deviceId);
  const prefix = GUEST_PREFIXES[hash % GUEST_PREFIXES.length];
  const suffix = GUEST_SUFFIXES[Math.floor(hash / GUEST_PREFIXES.length) % GUEST_SUFFIXES.length];
  const serial = String(hash % 1000).padStart(3, "0");

  return `${prefix}${suffix}${serial}`;
}

function ensureGuestName(deviceId: string) {
  const existing = safeLocalStorageGet(GUEST_NAME_KEY);
  if (existing) {
    return existing;
  }

  const next = createGuestName(deviceId);
  safeLocalStorageSet(GUEST_NAME_KEY, next);
  return next;
}

function detectSystemTimeZone() {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

function ensureDeepNightMode() {
  const stored = safeLocalStorageGet(DEEP_NIGHT_MODE_KEY);
  if (stored == null) {
    safeLocalStorageSet(DEEP_NIGHT_MODE_KEY, "true");
    return true;
  }

  return stored !== "false";
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [deviceId] = useState(() => ensureDeviceId());
  const [guestName] = useState(() => ensureGuestName(deviceId));
  const [userToken, setUserToken] = useState<string | null>(() => safeLocalStorageGet(USER_TOKEN_KEY));
  const [adminToken, setAdminToken] = useState<string | null>(() => safeLocalStorageGet(ADMIN_TOKEN_KEY));
  const [adminUsername, setAdminUsername] = useState<string | null>(() => safeLocalStorageGet(ADMIN_NAME_KEY));
  const [systemTimeZone] = useState(() => detectSystemTimeZone());
  const [deepNightMode, setDeepNightModeState] = useState(() => ensureDeepNightMode());
  const [bootstrap, setBootstrap] = useState<BootstrapResponse | null>(null);
  const [isReady, setIsReady] = useState(false);

  async function refresh() {
    const data = await fetchBootstrap(deviceId, userToken);
    setBootstrap(data);
  }

  useEffect(() => {
    void refresh().finally(() => setIsReady(true));
  }, [deviceId, userToken]);

  async function registerAccount(username: string, password: string) {
    const response = await register(username, password);
    safeLocalStorageSet(USER_TOKEN_KEY, response.token);
    setUserToken(response.token);
    setBootstrap((current) =>
      current
        ? {
            ...current,
            viewer: response.viewer
          }
        : current
    );
  }

  async function loginAccount(username: string, password: string) {
    const response = await login(username, password);
    safeLocalStorageSet(USER_TOKEN_KEY, response.token);
    setUserToken(response.token);
    setBootstrap((current) =>
      current
        ? {
            ...current,
            viewer: response.viewer
          }
        : current
    );
  }

  async function logoutAccount() {
    if (userToken) {
      await logout(userToken);
    }

    safeLocalStorageRemove(USER_TOKEN_KEY);
    setUserToken(null);
    await refresh();
  }

  async function loginAdmin(username: string, password: string) {
    const response = await adminLogin(username, password);
    safeLocalStorageSet(ADMIN_TOKEN_KEY, response.token);
    safeLocalStorageSet(ADMIN_NAME_KEY, response.adminUsername);
    setAdminToken(response.token);
    setAdminUsername(response.adminUsername);
  }

  async function logoutAdmin() {
    if (adminToken) {
      await adminLogout(deviceId, adminToken);
    }

    safeLocalStorageRemove(ADMIN_TOKEN_KEY);
    safeLocalStorageRemove(ADMIN_NAME_KEY);
    setAdminToken(null);
    setAdminUsername(null);
  }

  function setDeepNightMode(next: boolean) {
    safeLocalStorageSet(DEEP_NIGHT_MODE_KEY, String(next));
    setDeepNightModeState(next);
  }

  function setViewerFromAuth(viewer: ViewerSummary, token: string) {
    safeLocalStorageSet(USER_TOKEN_KEY, token);
    setUserToken(token);
    setBootstrap((current) =>
      current
        ? {
            ...current,
            viewer
          }
        : current
    );
  }

  const isGuestViewer = bootstrap?.viewer.kind !== "user";
  const isAdmin = Boolean(adminToken);
  const displayName = isGuestViewer ? guestName : bootstrap?.viewer.label ?? guestName;
  const viewerModeLabel = isGuestViewer ? "设备订阅" : "账号订阅";
  const viewerSubline = isGuestViewer ? `设备 ${deviceId.slice(0, 8)}` : "账号已连接";

  const value: SessionContextValue = {
    bootstrap,
    deviceId,
    guestName,
    userToken,
    adminToken,
    adminUsername,
    systemTimeZone,
    deepNightMode,
    displayName,
    viewerModeLabel,
    viewerSubline,
    isGuestViewer,
    isAdmin,
    isReady,
    refresh,
    setDeepNightMode,
    registerAccount,
    loginAccount,
    logoutAccount,
    loginAdmin,
    logoutAdmin,
    setViewerFromAuth
  };

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession() {
  const context = useContext(SessionContext);

  if (!context) {
    throw new Error("Session context is unavailable");
  }

  return context;
}
