// Référence partagée vers l'élément qui réserve la place du canvas natif : les règles en ont
// besoin pour convertir la position du pointeur en coordonnées de document.
let host: HTMLElement | null = null;

export function setCanvasHost(el: HTMLElement | null): void {
  host = el;
}

/** Rectangle du canvas en pixels physiques, relatif à la fenêtre ; `null` s'il n'est pas affiché. */
export function physicalRect(): { x: number; y: number; width: number; height: number } | null {
  if (!host) return null;
  const r = host.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  return { x: Math.round(r.left * dpr), y: Math.round(r.top * dpr), width: Math.round(r.width * dpr), height: Math.round(r.height * dpr) };
}
