import {
  cloneElement,
  isValidElement,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactElement,
  type ReactNode,
} from "react";

const ROUTE_TRANSITION_MS = 280;
const PRESENCE_TRANSITION_MS = 220;

function prefersReducedMotion() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function motionDelayStyle(index: number, step = 44, base = 0, cap = 10): CSSProperties {
  if (prefersReducedMotion()) {
    return {};
  }

  return {
    "--motion-delay": `${base + Math.min(index, cap) * step}ms`,
  } as CSSProperties;
}

export function MotionPage({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return <section className={["app-motion-page", className].filter(Boolean).join(" ")}>{children}</section>;
}

type MotionPresenceProps = {
  show: boolean;
  children: ReactNode;
  className?: string;
  mode?: "float" | "soft";
  durationMs?: number;
};

export function MotionPresence({
  show,
  children,
  className,
  mode = "float",
  durationMs = PRESENCE_TRANSITION_MS,
}: MotionPresenceProps) {
  const reduced = prefersReducedMotion();
  const [isRendered, setIsRendered] = useState(show);
  const [state, setState] = useState(show ? "entered" : "exited");
  const timerRef = useRef<number | null>(null);

  useEffect(() => {
    if (timerRef.current != null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }

    if (show) {
      setIsRendered(true);
      setState(reduced ? "entered" : "entering");

      if (!reduced) {
        const frame = window.requestAnimationFrame(() => {
          setState("entered");
        });

        return () => {
          window.cancelAnimationFrame(frame);
        };
      }

      return;
    }

    if (!isRendered) {
      return;
    }

    if (reduced) {
      setState("exited");
      setIsRendered(false);
      return;
    }

    setState("exiting");
    timerRef.current = window.setTimeout(() => {
      setState("exited");
      setIsRendered(false);
      timerRef.current = null;
    }, durationMs);

    return () => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };
  }, [durationMs, isRendered, reduced, show]);

  useEffect(
    () => () => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
      }
    },
    [],
  );

  if (!isRendered) {
    return null;
  }

  return (
    <div
      className={[
        "app-motion-presence",
        `app-motion-presence--${mode}`,
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      data-state={state}
      style={
        reduced
          ? undefined
          : ({
              "--presence-duration": `${durationMs}ms`,
            } as CSSProperties)
      }
    >
      {children}
    </div>
  );
}

type RouteLayer = {
  key: string;
  node: ReactNode;
  state: "entering" | "entered" | "exiting";
};

function enhanceRouteNode(node: ReactNode, className: string) {
  if (!isValidElement(node)) {
    return <div className={className}>{node}</div>;
  }

  const typedNode = node as ReactElement<{ className?: string }>;
  const currentClassName =
    typeof typedNode.props === "object" && typedNode.props != null && "className" in typedNode.props
      ? String(typedNode.props.className ?? "")
      : "";

  return cloneElement(typedNode, {
    className: [currentClassName, className].filter(Boolean).join(" "),
  });
}

export function RoutedMotionOutlet({
  routeKey,
  outlet,
}: {
  routeKey: string;
  outlet: ReactNode;
}) {
  const reduced = prefersReducedMotion();
  const [layers, setLayers] = useState<RouteLayer[]>(() => [
    {
      key: routeKey,
      node: outlet,
      state: "entered",
    },
  ]);

  useEffect(() => {
    if (reduced) {
      setLayers([
        {
          key: routeKey,
          node: outlet,
          state: "entered",
        },
      ]);
      return;
    }

    setLayers((current) => {
      const existing = current.find((layer) => layer.key === routeKey);
      if (existing) {
        return current.map((layer) =>
          layer.key === routeKey ? { ...layer, node: outlet, state: "entered" } : layer,
        );
      }

      return [
        ...current.map((layer, index) =>
          index === current.length - 1 ? { ...layer, state: "exiting" as const } : layer,
        ),
        {
          key: routeKey,
          node: outlet,
          state: "entering",
        },
      ];
    });

    const frame = window.requestAnimationFrame(() => {
      setLayers((current) =>
        current.map((layer) =>
          layer.key === routeKey && layer.state === "entering"
            ? { ...layer, state: "entered" }
            : layer,
        ),
      );
    });

    const timer = window.setTimeout(() => {
      setLayers((current) => current.filter((layer) => layer.key === routeKey || layer.state !== "exiting"));
    }, ROUTE_TRANSITION_MS);

    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [outlet, reduced, routeKey]);

  const renderedLayers = useMemo(
    () =>
      layers.map((layer) => ({
        ...layer,
        node: enhanceRouteNode(layer.node, "app-route-page"),
      })),
    [layers],
  );

  return (
    <div className="app-route-stack">
      {renderedLayers.map((layer, index) => {
        const isOverlay = layer.state === "exiting" && index !== renderedLayers.length - 1;

        return (
          <div
            key={layer.key}
            className={[
              "app-route-layer",
              `app-route-layer--${layer.state}`,
              isOverlay ? "app-route-layer--overlay" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            style={{ "--route-duration": `${ROUTE_TRANSITION_MS}ms` } as CSSProperties}
          >
            {layer.node}
          </div>
        );
      })}
    </div>
  );
}
