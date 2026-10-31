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

/** Pictogramme textuel : pour les outils qui n'ont pas encore de dessin dédié. */
export const Glyph = ({ children }: { children: string }) => (
  <span aria-hidden="true" style={{ fontSize: 18, lineHeight: 1 }}>{children}</span>
);

// Icônes d'outils : même grille 24×24, même trait, pour que la barre d'outils soit homogène.
export const BrushIcon = () => (
  <svg {...base}>
    <path d="M20.3 3.7a1.6 1.6 0 0 0-2.3 0L10 11.7l2.3 2.3 8-8a1.6 1.6 0 0 0 0-2.3Z" />
    <path d="M9.5 12.5C7 12.5 5.5 14 5.5 16.2c0 1.2-.7 2.2-1.5 2.8 3.5 1 7.5-.2 7.5-3.3" />
  </svg>
);
export const PencilIcon = () => (
  <svg {...base}>
    <path d="M4 20l1-4L16.5 4.5a2.1 2.1 0 0 1 3 3L8 19l-4 1Z" />
    <path d="m14.5 6.5 3 3" />
  </svg>
);
export const EraserIcon = () => (
  <svg {...base}>
    <path d="M7 20h11" />
    <path d="m5.2 14.8 8.1-8.1a2 2 0 0 1 2.8 0l2.2 2.2a2 2 0 0 1 0 2.8L11.5 18H8.6l-3.4-3.4a1 1 0 0 1 0-1.4Z" />
    <path d="m9.5 10.5 5 5" />
  </svg>
);
export const BucketIcon = () => (
  <svg {...base}>
    <path d="M5 11.2 11.6 4.6a1.5 1.5 0 0 1 2.1 0l4.7 4.7a1.5 1.5 0 0 1 0 2.1L12.8 17a1.5 1.5 0 0 1-2.1 0L5 11.2Z" />
    <path d="M5.5 11.5h12.8" />
    <path d="M20.2 14.6c1 1.4 1.5 2.3 1.5 3a1.5 1.5 0 0 1-3 0c0-.7.5-1.6 1.5-3Z" />
  </svg>
);
export const GradientIcon = () => (
  <svg {...base}>
    <rect x="3.5" y="3.5" width="17" height="17" rx="2.5" />
    <path d="M8 3.5v17" opacity=".9" />
    <path d="M12 3.5v17" opacity=".6" />
    <path d="M16 3.5v17" opacity=".3" />
  </svg>
);
export const CloneIcon = () => (
  <svg {...base}>
    <path d="M12 3a3 3 0 0 1 3 3c0 1.6-1 2.3-1.5 3.4-.2.6.1 1.1.8 1.1H17a3 3 0 0 1 3 3v1.5H4V13.5a3 3 0 0 1 3-3h1.7c.7 0 1-.5.8-1.1C9 8.3 9 7.6 9 6a3 3 0 0 1 3-3Z" />
    <path d="M5 18h14v3H5z" />
  </svg>
);
export const HealIcon = () => (
  <svg {...base}>
    <rect x="3" y="9" width="18" height="6" rx="3" transform="rotate(-45 12 12)" />
    <path d="M10.6 10.6h.01M13.4 13.4h.01M10.6 13.4h.01M13.4 10.6h.01" strokeWidth={2.4} />
  </svg>
);
export const SpotHealIcon = () => (
  <svg {...base}>
    <circle cx="12" cy="12" r="8" strokeDasharray="2.6 3" />
    <path d="M12 8.5v7M8.5 12h7" />
  </svg>
);
export const PatchIcon = () => (
  <svg {...base}>
    <rect x="4" y="4" width="16" height="16" rx="2.5" strokeDasharray="3 3" />
    <path d="m9 15 6-6M10.5 9H15v4.5" />
  </svg>
);
export const BlurIcon = () => (
  <svg {...base}>
    <path d="M12 3.5c3.5 4 6 7 6 10a6 6 0 0 1-12 0c0-3 2.5-6 6-10Z" />
  </svg>
);
export const SharpenIcon = () => (
  <svg {...base}>
    <path d="m12 4 8.5 15h-17L12 4Z" />
    <path d="M12 10v4" />
  </svg>
);
export const SmudgeIcon = () => (
  <svg {...base}>
    <path d="M3.5 15.5c2.5-6 5 4 8-1.5s5.5-3.5 9-1" />
    <path d="M3.5 19.500c2.5-3 5 1 8-1.500s5-1.5 9 0" opacity=".55" />
  </svg>
);
export const DodgeIcon = () => (
  <svg {...base}>
    <circle cx="12" cy="12" r="3.8" />
    <path d="M12 3v2.200M12 18.800V21M3 12h2.200M18.8 12H21M5.6 5.600l1.5 1.500M16.9 16.900l1.5 1.500M5.6 18.400l1.5-1.500M16.9 7.100l1.5-1.5" />
  </svg>
);
export const BurnIcon = () => (
  <svg {...base}>
    <path d="M20 14.500A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.500Z" />
  </svg>
);
export const SpongeIcon = () => (
  <svg {...base}>
    <rect x="3.5" y="8" width="17" height="10" rx="3.5" />
    <path d="M8 12h.01M12 14h.01M16 12h.01M10.5 16h.01M14.5 10.500h.01" strokeWidth={2.4} />
  </svg>
);
export const LiquifyIcon = () => (
  <svg {...base}>
    <path d="M3 8.500c3-3 6 3 9 0s6 3 9 0" />
    <path d="M3 15.500c3-3 6 3 9 0s6 3 9 0" />
  </svg>
);
export const TextIcon = () => (
  <svg {...base}>
    <path d="M5 7V4.500h14V7M12 4.500v15M9 19.500h6" />
  </svg>
);
export const PenIcon = () => (
  <svg {...base}>
    <path d="m12 2.5 5.5 9.5-3.2 2.400H9.700L6.5 12 12 2.500Z" />
    <path d="M12 14.400V21" />
    <circle cx="12" cy="10.5" r="1.1" />
  </svg>
);
export const RectIcon = () => (
  <svg {...base}>
    <rect x="4" y="6" width="16" height="12" rx=".8" />
  </svg>
);
export const RoundedRectIcon = () => (
  <svg {...base}>
    <rect x="4" y="6" width="16" height="12" rx="4" />
  </svg>
);
export const EllipseIcon = () => (
  <svg {...base}>
    <ellipse cx="12" cy="12" rx="9" ry="6.5" />
  </svg>
);
export const LineIcon = () => (
  <svg {...base}>
    <path d="M5 19 19 5" />
    <circle cx="5" cy="19" r="1.2" />
    <circle cx="19" cy="5" r="1.2" />
  </svg>
);
