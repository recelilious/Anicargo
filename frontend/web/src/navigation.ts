type LocationLike = {
  pathname: string;
  search: string;
  hash: string;
};

export type RouteState = {
  fromPath?: string;
  restoreScrollTop?: number;
};

type StoredReturnTarget = {
  fromPath: string;
  scrollTop: number;
};

const RETURN_TARGET_STORAGE_KEY = "anicargo:return-target";

export function buildRoutePath(location: LocationLike) {
  return `${location.pathname}${location.search}${location.hash}`;
}

export function rememberReturnTarget(fromPath: string, scrollTop: number) {
  if (typeof window === "undefined") {
    return;
  }

  const payload: StoredReturnTarget = {
    fromPath,
    scrollTop,
  };

  window.sessionStorage.setItem(RETURN_TARGET_STORAGE_KEY, JSON.stringify(payload));
}

export function resolveReturnScrollTop(fromPath: string) {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.sessionStorage.getItem(RETURN_TARGET_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const payload = JSON.parse(raw) as Partial<StoredReturnTarget>;
    if (payload.fromPath !== fromPath || typeof payload.scrollTop !== "number") {
      return null;
    }

    return Number.isFinite(payload.scrollTop) ? payload.scrollTop : null;
  } catch {
    return null;
  }
}
