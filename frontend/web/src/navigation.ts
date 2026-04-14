type LocationLike = {
  pathname: string;
  search: string;
  hash: string;
};

export type RouteState = {
  restoreScrollTop?: number;
};

type StoredReturnTarget = {
  fromPath: string;
  toPath: string;
  scrollTop: number;
};

const RETURN_TARGET_STORAGE_KEY = "anicargo:return-stack";
const MAX_RETURN_TARGETS = 64;

export function buildRoutePath(location: LocationLike) {
  return `${location.pathname}${location.search}${location.hash}`;
}

function readReturnTargetStack() {
  if (typeof window === "undefined") {
    return [] as StoredReturnTarget[];
  }

  const raw = window.sessionStorage.getItem(RETURN_TARGET_STORAGE_KEY);
  if (!raw) {
    return [] as StoredReturnTarget[];
  }

  try {
    const payload = JSON.parse(raw);
    if (!Array.isArray(payload)) {
      return [] as StoredReturnTarget[];
    }

    return payload.filter((item): item is StoredReturnTarget => {
      return (
        item != null &&
        typeof item === "object" &&
        typeof item.fromPath === "string" &&
        typeof item.toPath === "string" &&
        typeof item.scrollTop === "number" &&
        Number.isFinite(item.scrollTop)
      );
    });
  } catch {
    return [] as StoredReturnTarget[];
  }
}

function writeReturnTargetStack(stack: StoredReturnTarget[]) {
  if (typeof window === "undefined") {
    return;
  }

  window.sessionStorage.setItem(RETURN_TARGET_STORAGE_KEY, JSON.stringify(stack.slice(-MAX_RETURN_TARGETS)));
}

export function rememberReturnTarget(fromPath: string, toPath: string, scrollTop: number) {
  if (typeof window === "undefined" || fromPath === toPath) {
    return;
  }

  const stack = readReturnTargetStack();
  stack.push({
    fromPath,
    toPath,
    scrollTop: Number.isFinite(scrollTop) ? scrollTop : 0,
  });
  writeReturnTargetStack(stack);
}

export function consumeReturnTarget(currentPath: string) {
  if (typeof window === "undefined") {
    return null;
  }

  const stack = readReturnTargetStack();
  const nextStack = [...stack];

  while (nextStack.length > 0) {
    const target = nextStack.pop();
    if (!target) {
      break;
    }

    if (target.toPath !== currentPath) {
      continue;
    }

    writeReturnTargetStack(nextStack);
    return {
      fromPath: target.fromPath,
      scrollTop: target.scrollTop,
    };
  }

  writeReturnTargetStack(nextStack);
  return null;
}
