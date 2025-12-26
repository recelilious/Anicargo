import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

export type UserRole = "admin" | "user";

export interface SessionInfo {
  token: string;
  userId: string;
  role: UserRole;
  expiresIn: number;
}

interface SessionContextValue {
  session: SessionInfo | null;
  setSession: (session: SessionInfo | null) => void;
  clearSession: () => void;
}

const sessionKey = "anicargo.session";
const SessionContext = createContext<SessionContextValue | undefined>(undefined);

function loadSession(): SessionInfo | null {
  const raw = window.localStorage.getItem(sessionKey);
  if (!raw) {
    return null;
  }
  try {
    return JSON.parse(raw) as SessionInfo;
  } catch {
    return null;
  }
}

function saveSession(session: SessionInfo | null) {
  if (!session) {
    window.localStorage.removeItem(sessionKey);
    return;
  }
  window.localStorage.setItem(sessionKey, JSON.stringify(session));
}

export function SessionProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState<SessionInfo | null>(() => loadSession());

  useEffect(() => {
    saveSession(session);
  }, [session]);

  const value = useMemo(
    () => ({
      session,
      setSession,
      clearSession: () => setSession(null)
    }),
    [session]
  );

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession() {
  const context = useContext(SessionContext);
  if (!context) {
    throw new Error("useSession must be used within SessionProvider");
  }
  return context;
}
