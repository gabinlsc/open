import type { SVGProps } from "react";

const base: SVGProps<SVGSVGElement> = {
  width: 20,
  height: 20,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": true,
  focusable: false,
};

export const HandIcon = () => (
  <svg {...base}>
    <path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V11m0-1V4.5a1.5 1.5 0 0 1 3 0V11m0-4.5a1.5 1.5 0 0 1 3 0V13m0-4.5a1.5 1.5 0 0 1 3 0V15a6 6 0 0 1-6 6h-1.5a6 6 0 0 1-4.7-2.3L4 14.5a1.5 1.5 0 0 1 2.3-1.9L8 15" />
  </svg>
);

export const ZoomIcon = () => (
  <svg {...base}>
    <circle cx="10.5" cy="10.5" r="6.5" />
    <path d="m20 20-4.9-4.9M10.5 8v5M8 10.5h5" />
  </svg>
);

export const PlusIcon = () => (
  <svg {...base} width={16} height={16}>
    <path d="M12 5v14M5 12h14" />
  </svg>
);

export const CloseIcon = () => (
  <svg {...base} width={14} height={14}>
    <path d="m6 6 12 12M18 6 6 18" />
  </svg>
);

export const RectSelectIcon = () => (
  <svg {...base}>
    <rect x="4" y="5" width="16" height="14" strokeDasharray="3 2.5" />
  </svg>
);

export const EllipseSelectIcon = () => (
  <svg {...base}>
    <ellipse cx="12" cy="12" rx="8" ry="6" strokeDasharray="3 2.5" />
  </svg>
);

export const LassoIcon = () => (
  <svg {...base}>
    <path d="M5 11c0-3.5 3.6-6 8-6s7 2.2 7 5-3.2 4.6-7.5 4.6c-1.2 0-2.1-.2-2.9-.5M9 14.2c-1.4 1.8-1 4.3 1.5 4.8" strokeDasharray="3 2.5" />
  </svg>
);

export const PolygonLassoIcon = () => (
  <svg {...base}>
    <path d="m4 15 4-9 6 3 6-3-2 12-8 1z" strokeDasharray="3 2.5" />
  </svg>
);

export const WandIcon = () => (
  <svg {...base}>
    <path d="m5 19 9-9M14 10l-1.5-3.5L16 8l3.5-1.5L18 10l1.5 3.5L16 12l-3.5 1.5z" />
    <path d="M6 5v3M4.5 6.5h3" />
  </svg>
);
