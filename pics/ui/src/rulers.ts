export interface Tick {
  /** Position en pixels CSS le long de la règle. */
  pos: number;
  /** Valeur en pixels de document. */
  value: number;
  major: boolean;
}

const STEPS = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000];

/** Plus petit pas « rond » dont l'écartement à l'écran reste lisible (≥ `minPx` pixels CSS). */
export function niceStep(pxPerUnit: number, minPx = 60): number {
  if (!(pxPerUnit > 0)) return STEPS[STEPS.length - 1]!;
  return STEPS.find((s) => s * pxPerUnit >= minPx) ?? STEPS[STEPS.length - 1]!;
}

/**
 * Graduations d'une règle.
 * @param origin position de l'origine du document en pixels CSS, relative au début de la règle
 * @param pxPerUnit pixels CSS par pixel de document (zoom ÷ densité d'écran)
 * @param length longueur visible de la règle en pixels CSS
 */
export function rulerTicks(origin: number, pxPerUnit: number, length: number): Tick[] {
  if (!(pxPerUnit > 0) || !(length > 0) || !Number.isFinite(origin)) return [];
  const major = niceStep(pxPerUnit);
  const minor = major / (major % 5 === 0 && major >= 10 ? 5 : 2);
  const startValue = Math.floor((0 - origin) / pxPerUnit / minor) * minor;
  const endValue = (length - origin) / pxPerUnit;
  const ticks: Tick[] = [];
  // Garde-fou : jamais plus de 2000 graduations, même si le zoom est extrême.
  for (let v = startValue, n = 0; v <= endValue && n < 2000; v += minor, n++) {
    const value = Math.round(v * 1e6) / 1e6;
    ticks.push({ pos: origin + value * pxPerUnit, value, major: Math.abs(value % major) < 1e-6 });
  }
  return ticks;
}
