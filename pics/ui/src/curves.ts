/** Utilitaires de l'éditeur de courbes (purs, testés). */

export type Point = [number, number];

const clamp01 = (v: number) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);

/** Points triés par x, bornés à 0..1, sans doublon de x (le premier gagne) ; jamais moins de 2 points. */
export function normalizePoints(points: Point[]): Point[] {
  const sorted = points.map(([x, y]): Point => [clamp01(x), clamp01(y)]).sort((a, b) => a[0] - b[0]);
  const out: Point[] = [];
  for (const p of sorted) if (out.length === 0 || p[0] - out[out.length - 1]![0] > 1e-4) out.push(p);
  return out.length >= 2 ? out : [[0, 0], [1, 1]];
}

export function addPoint(points: Point[], x = 0.5): Point[] {
  const sorted = normalizePoints(points);
  // y initial sur la courbe actuelle (interpolation linéaire) : l'ajout ne change pas la forme.
  let y = x;
  for (let i = 0; i < sorted.length - 1; i++) {
    const [a, b] = [sorted[i]!, sorted[i + 1]!];
    if (x >= a[0] && x <= b[0]) y = a[1] + ((x - a[0]) / Math.max(b[0] - a[0], 1e-6)) * (b[1] - a[1]);
  }
  return normalizePoints([...sorted, [x, y]]);
}

export function removePoint(points: Point[], index: number): Point[] {
  // On garde les deux extrémités : une courbe doit toujours couvrir 0..1.
  if (index <= 0 || index >= points.length - 1) return points;
  return points.filter((_, i) => i !== index);
}

/** Chemin SVG d'un tracé de la courbe dans un carré `size` (y vers le haut). */
export function svgPath(points: Point[], size: number): string {
  return normalizePoints(points)
    .map(([x, y], i) => `${i === 0 ? "M" : "L"}${(x * size).toFixed(1)} ${((1 - y) * size).toFixed(1)}`)
    .join(" ");
}
