import {
  createContext,
  useContext,
  useEffect,
  useState
} from "react";

import { fetchBootstrap, login, logout, register } from "./api";
import type { BootstrapResponse, ViewerSummary } from "./types";

const DEVICE_KEY = "anicargo.device_id";
const USER_TOKEN_KEY = "anicargo.user_token";

type SessionContextValue = {
  bootstrap: BootstrapResponse | null;
  deviceId: string;
  userToken: string | null;
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

export function SessionProvider({ children }: { children: React.ReactNode }) {
  const [deviceId] = useState(() => ensureDeviceId());
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

  const value: SessionContextValue = {
    bootstrap,
    deviceId,
    userToken,
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
