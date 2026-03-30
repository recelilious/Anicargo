import type { CSSProperties } from "react";

import { BrandLogo } from "./BrandLogo";

type CardCoverFallbackProps = {
  logoWidth?: string;
  logoMaxWidth?: number;
  style?: CSSProperties;
};

export function CardCoverFallback({
  logoWidth = "30%",
  logoMaxWidth = 84,
  style,
}: CardCoverFallbackProps) {
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "grid",
        placeItems: "center",
        background:
          "radial-gradient(circle at 50% 38%, rgba(255,255,255,0.1), rgba(255,255,255,0) 56%), var(--app-fallback-hero)",
        ...style,
      }}
    >
      <BrandLogo
        aria-hidden="true"
        style={{
          width: logoWidth,
          maxWidth: `${logoMaxWidth}px`,
          height: "auto",
          color: "rgba(255, 248, 241, 0.92)",
          filter: "drop-shadow(0 8px 18px rgba(0, 0, 0, 0.22))",
        }}
      />
    </div>
  );
}
