import React from "react";

interface LogoMarkProps {
  size?: number;
  stroke?: string;
  strokeWidth?: number;
}

export default function LogoMark({ size = 120, stroke = "currentColor", strokeWidth = 30 }: LogoMarkProps) {
  const viewBox = "0 0 408 460";
  return (
    <svg
      width={size}
      height={(size * 460) / 408}
      viewBox={viewBox}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-label="Anicargo logo"
    >
      <g clipPath="url(#clip0_0_1)">
        <line x1="15" x2="15" y2="460" stroke={stroke} strokeWidth={strokeWidth} />
        <circle cx="132" cy="325" r="117" stroke={stroke} strokeWidth={strokeWidth} />
        <line x1="11.6581" y1="-9.43867" x2="398.658" y2="468.561" stroke={stroke} strokeWidth={strokeWidth} />
        <line x1="140" y1="324" x2="244" y2="324" stroke={stroke} strokeWidth={strokeWidth} />
      </g>
      <defs>
        <clipPath id="clip0_0_1">
          <rect width="408" height="460" fill="white" />
        </clipPath>
      </defs>
    </svg>
  );
}
