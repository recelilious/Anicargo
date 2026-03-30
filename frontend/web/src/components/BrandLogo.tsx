import type { SVGProps } from "react";

export function BrandLogo(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 481 600" fill="none" xmlns="http://www.w3.org/2000/svg" {...props}>
      <g clipPath="url(#anicargo-brand-logo-clip)">
        <circle cx="164.5" cy="435.5" r="130" stroke="currentColor" strokeWidth="69" />
        <rect
          x="280.707"
          y="317"
          width="60"
          height="158.344"
          transform="rotate(55 280.707 317)"
          fill="currentColor"
        />
        <rect
          x="11"
          y="34.4146"
          width="60"
          height="732.555"
          transform="rotate(-35 11 34.4146)"
          fill="currentColor"
        />
        <rect width="60" height="600" fill="currentColor" />
      </g>
      <defs>
        <clipPath id="anicargo-brand-logo-clip">
          <rect width="481" height="600" fill="white" />
        </clipPath>
      </defs>
    </svg>
  );
}
