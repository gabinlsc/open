import type { BlendInfo, StyleDto } from "./api";

export type StyleKind = StyleDto["kind"];

export const STYLE_KINDS: { kind: StyleKind; label: string }[] = [
  { kind: "drop_shadow", label: "Ombre portée" },
  { kind: "outer_glow", label: "Lueur externe" },
  { kind: "stroke", label: "Contour" },
  { kind: "color_overlay", label: "Incrustation de couleur" },
  { kind: "gradient_overlay", label: "Incrustation de dégradé" },
];

/** Valeurs proposées à l'activation d'un style : visibles tout de suite sur un fond clair ou sombre. */
export function defaultStyle(kind: StyleKind): StyleDto {
  switch (kind) {
    case "drop_shadow":
      return { kind, color: "#000000", opacity: 0.6, angleDeg: 45, distance: 8, size: 6 };
    case "outer_glow":
      return { kind, color: "#ffd24a", opacity: 0.8, size: 10 };
    case "stroke":
      return { kind, color: "#2447d6", opacity: 1, size: 3, position: "outside" };
    case "color_overlay":
      return { kind, color: "#2447d6", opacity: 1, blend: "normal" };
    case "gradient_overlay":
      return { kind, from: "#2447d6", to: "#ffffff", angleDeg: 90, opacity: 1, blend: "normal" };
  }
}

/** Ajoute ou retire le style d'un type donné ; l'ordre des autres styles est conservé. */
export function toggleStyle(styles: StyleDto[], kind: StyleKind, enabled: boolean): StyleDto[] {
  const without = styles.filter((s) => s.kind !== kind);
  return enabled ? [...without, defaultStyle(kind)] : without;
}

export function replaceStyle(styles: StyleDto[], next: StyleDto): StyleDto[] {
  return styles.map((s) => (s.kind === next.kind ? next : s));
}

/** Borne une saisie numérique : un champ vide ou illisible garde la valeur précédente. */
export function parseNumber(text: string, previous: number, min: number, max: number): number {
  if (text.trim() === "") return previous;
  const n = Number(text.replace(",", "."));
  return Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : previous;
}

export type { BlendInfo };
