import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { Spinner, Text, makeStyles } from "@fluentui/react-components";
import { MotionPresence } from "./motion";

type StatusEntry = {
  id: number;
  message: string;
};

type LoadingStatusContextValue = {
  currentStatus: StatusEntry | null;
  pushStatus: (message: string) => number;
  updateStatus: (id: number, message: string) => void;
  removeStatus: (id: number) => void;
};

const LoadingStatusContext = createContext<LoadingStatusContextValue | null>(null);

const useStyles = makeStyles({
  viewport: {
    position: "fixed",
    left: "18px",
    bottom: "18px",
    zIndex: 30,
    width: "min(188px, calc(100vw - 36px))",
    pointerEvents: "none",
  },
  statusCard: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
    padding: "10px 12px",
    borderRadius: "18px",
    backgroundColor: "var(--app-panel)",
    border: "1px solid var(--app-border)",
    boxShadow: "var(--app-card-shadow)",
    minWidth: 0,
  },
  statusText: {
    minWidth: 0,
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    color: "var(--app-muted)",
  },
});

export function LoadingStatusProvider({ children }: { children: ReactNode }) {
  const nextIdRef = useRef(1);
  const [entries, setEntries] = useState<StatusEntry[]>([]);
  const currentStatus = entries[entries.length - 1] ?? null;

  const pushStatus = useCallback((message: string) => {
    const id = nextIdRef.current;
    nextIdRef.current += 1;
    setEntries((current) => [...current, { id, message }]);
    return id;
  }, []);

  const updateStatus = useCallback((id: number, message: string) => {
    setEntries((current) =>
      current.map((entry) => (entry.id === id ? { ...entry, message } : entry)),
    );
  }, []);

  const removeStatus = useCallback((id: number) => {
    setEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const value = useMemo<LoadingStatusContextValue>(
    () => ({
      currentStatus,
      pushStatus,
      updateStatus,
      removeStatus,
    }),
    [currentStatus, pushStatus, removeStatus, updateStatus],
  );

  return (
    <LoadingStatusContext.Provider value={value}>
      {children}
      <LoadingStatusViewport />
    </LoadingStatusContext.Provider>
  );
}

function LoadingStatusViewport() {
  const styles = useStyles();
  const context = useContext(LoadingStatusContext);
  const currentStatus = context?.currentStatus ?? null;

  if (!currentStatus) {
    return null;
  }

  return (
    <MotionPresence show={Boolean(currentStatus)} className={styles.viewport} mode="soft">
      <div className={`${styles.statusCard} app-motion-surface`}>
        <Spinner size="tiny" />
        <Text size={200} className={styles.statusText} title={currentStatus.message}>
          {currentStatus.message}
        </Text>
      </div>
    </MotionPresence>
  );
}

export function useLoadingStatus(message: string | null) {
  const context = useContext(LoadingStatusContext);
  if (!context) {
    throw new Error("useLoadingStatus must be used within LoadingStatusProvider");
  }

  const { pushStatus, removeStatus, updateStatus } = context;
  const statusIdRef = useRef<number | null>(null);

  useEffect(() => {
    if (!message) {
      if (statusIdRef.current != null) {
        removeStatus(statusIdRef.current);
        statusIdRef.current = null;
      }
      return;
    }

    if (statusIdRef.current == null) {
      statusIdRef.current = pushStatus(message);
      return;
    }

    updateStatus(statusIdRef.current, message);
  }, [message, pushStatus, removeStatus, updateStatus]);

  useEffect(
    () => () => {
      if (statusIdRef.current != null) {
        removeStatus(statusIdRef.current);
        statusIdRef.current = null;
      }
    },
    [removeStatus],
  );
}
