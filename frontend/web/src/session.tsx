import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

import { fetchBootstrap, login, logout, register } from "./api";
import type { BootstrapResponse, ViewerSummary } from "./types";

const DEVICE_KEY = "anicargo.device_id";
const GUEST_NAME_KEY = "anicargo.guest_name";
const USER_TOKEN_KEY = "anicargo.user_token";
const GUEST_PREFIXES = ["薄荷", "晴空", "星港", "海盐", "雾岚", "白塔", "琥珀", "月汐"];
const GUEST_SUFFIXES = ["旅人", "放映员", "观测者", "追番人", "导航员", "收藏家", "编目员", "信使"];

type SessionContextValue = {
  bootstrap: BootstrapResponse | null;
  deviceId: string;
  guestName: string;
  userToken: string | null;
  displayName: string;
  viewerModeLabel: string;
  viewerSubline: string;
  isGuestViewer: boolean;
  isReady: boolean;
  refresh: () => Promise<void>;
  registerAccount: (username: string, password: string) => Promise<void>;
  loginAccount: (username: string, password: string) => Promise<void>;
  logoutAccount: () => Promise<void>;
  setViewerFromAuth: (viewer: ViewerSummary, token: string) => void;
};

const SessionContext = createContext<SessionContextValue | null>(null);

function ensureDeviceId() {
  const existing = window.localStorage.getItem(DEVICE_KEY);
  if (existing) {
    return existing;
  }

  const next = crypto.randomUUID();
  window.localStorage.setItem(DEVICE_KEY, next);
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
  const existing = window.localStorage.getItem(GUEST_NAME_KEY);
  if (existing) {
    return existing;
  }

  const next = createGuestName(deviceId);
  window.localStorage.setItem(GUEST_NAME_KEY, next);
  return next;
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [deviceId] = useState(() => ensureDeviceId());
  const [guestName] = useState(() => ensureGuestName(deviceId));
  const [userToken, setUserToken] = useState<string | null>(() => window.localStorage.getItem(USER_TOKEN_KEY));
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
    window.localStorage.setItem(USER_TOKEN_KEY, response.token);
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
    window.localStorage.setItem(USER_TOKEN_KEY, response.token);
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

    window.localStorage.removeItem(USER_TOKEN_KEY);
    setUserToken(null);
    await refresh();
  }

  function setViewerFromAuth(viewer: ViewerSummary, token: string) {
    window.localStorage.setItem(USER_TOKEN_KEY, token);
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
  const displayName = isGuestViewer ? guestName : bootstrap?.viewer.label ?? guestName;
  const viewerModeLabel = isGuestViewer ? "设备订阅" : "账号订阅";
  const viewerSubline = isGuestViewer ? `游客设备 ${deviceId.slice(0, 8)}` : "订阅与历史会跟随账号同步";

  const value: SessionContextValue = {
    bootstrap,
    deviceId,
    guestName,
    userToken,
    displayName,
    viewerModeLabel,
    viewerSubline,
    isGuestViewer,
    isReady,
    refresh,
    registerAccount,
    loginAccount,
    logoutAccount,
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
