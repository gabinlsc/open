import { useEffect, useRef } from "react";
import { api, type AxisDto } from "../api";
import { physicalRect } from "../canvasHost";
import { rulerTicks } from "../rulers";
import { useStore } from "../store";

export const RULER_SIZE = 22;

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || "#888";
}

/** Une règle dessinée sur un <canvas> 2D. Ce canvas ne contient que des graduations : jamais de pixels du document. */
function Ruler({ orientation, length }: { orientation: "top" | "left"; length: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  const view = useStore((s) => s.snapshot?.view);
  const guides = useStore((s) => s.snapshot?.guides);
  const cursor = useStore((s) => s.cursor);
  const theme = useStore((s) => s.themeChoice);
  const dragging = useRef<{ pos: number | null } | null>(null);
  // La règle du haut est une barre horizontale qui mesure les x ; glisser depuis elle crée un
  // repère horizontal (une ligne à y constant). Même logique, transposée, pour la règle de gauche.
  const horizontal = orientation === "top";

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas || !view) return;
    const dpr = window.devicePixelRatio || 1;
    const w = horizontal ? length : RULER_SIZE;
    const h = horizontal ? RULER_SIZE : length;
    canvas.width = Math.max(1, Math.round(w * dpr));
    canvas.height = Math.max(1, Math.round(h * dpr));
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    ctx.fillStyle = cssVar("--ruler-bg");
    ctx.fillRect(0, 0, w, h);
    const ink = cssVar("--ruler-ink");
    ctx.strokeStyle = ink;
    ctx.fillStyle = ink;
    ctx.font = "10px Inter, system-ui, sans-serif";
    ctx.lineWidth = 1;
    const origin = (horizontal ? view.originX : view.originY) / dpr;
    const ticks = rulerTicks(origin, view.zoom / dpr, length);
    ctx.beginPath();
    for (const t of ticks) {
      const p = Math.round(t.pos) + 0.5;
      const len = t.major ? RULER_SIZE * 0.6 : RULER_SIZE * 0.3;
      if (horizontal) {
        ctx.moveTo(p, RULER_SIZE);
        ctx.lineTo(p, RULER_SIZE - len);
      } else {
        ctx.moveTo(RULER_SIZE, p);
        ctx.lineTo(RULER_SIZE - len, p);
      }
    }
    ctx.stroke();
    for (const t of ticks) {
      if (!t.major) continue;
      const label = String(Math.round(t.value));
      if (horizontal) ctx.fillText(label, t.pos + 3, 10);
      else {
        ctx.save();
        ctx.translate(10, t.pos - 3);
        ctx.rotate(-Math.PI / 2);
        ctx.fillText(label, 0, 0);
        ctx.restore();
      }
    }
    // Repères existants et position du curseur.
    ctx.fillStyle = cssVar("--guide");
    for (const g of guides ?? []) {
      if ((g.axis === "vertical") !== horizontal) continue;
      const p = Math.round((g.position * view.zoom + (horizontal ? view.originX : view.originY)) / dpr);
      if (horizontal) ctx.fillRect(p, 0, 1, RULER_SIZE);
      else ctx.fillRect(0, p, RULER_SIZE, 1);
    }
    if (cursor) {
      const c = horizontal ? cursor.x : cursor.y;
      const p = Math.round((c * view.zoom + (horizontal ? view.originX : view.originY)) / dpr);
      ctx.fillStyle = cssVar("--accent");
      if (horizontal) ctx.fillRect(p, 0, 1, RULER_SIZE);
      else ctx.fillRect(0, p, RULER_SIZE, 1);
    }
  }, [view, guides, cursor, length, horizontal, theme]);

  /** Position du pointeur en pixels de document sur l'axe perpendiculaire à la règle. */
  const docPosition = (e: React.PointerEvent): number | null => {
    const rect = physicalRect();
    const v = useStore.getState().snapshot?.view;
    if (!rect || !v) return null;
    const dpr = window.devicePixelRatio || 1;
    return horizontal ? ((e.clientY * dpr - rect.y) - v.originY) / v.zoom : ((e.clientX * dpr - rect.x) - v.originX) / v.zoom;
  };

  const overRuler = (e: React.PointerEvent): boolean => {
    const r = ref.current?.getBoundingClientRect();
    return !!r && e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
  };

  const guideAxis: AxisDto = horizontal ? "horizontal" : "vertical";

  return (
    <canvas
      ref={ref}
      className={horizontal ? "ruler ruler-top" : "ruler ruler-left"}
      style={horizontal ? { width: length, height: RULER_SIZE } : { width: RULER_SIZE, height: length }}
      aria-label={horizontal ? "Règle horizontale : glisser vers le bas pour créer un repère" : "Règle verticale : glisser vers la droite pour créer un repère"}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        dragging.current = { pos: null };
      }}
      onPointerMove={(e) => {
        if (!dragging.current) return;
        const pos = overRuler(e) ? null : docPosition(e);
        dragging.current.pos = pos;
        if (pos === null) void api.guideCancel();
        else void api.guideDrag(guideAxis, pos).catch(() => undefined);
      }}
      onPointerUp={(e) => {
        const drag = dragging.current;
        dragging.current = null;
        if (!drag) return;
        const pos = overRuler(e) ? null : docPosition(e);
        if (pos === null) void api.guideCancel();
        else void useStore.getState().run(() => api.guideCommit(guideAxis, pos));
      }}
      onPointerCancel={() => {
        dragging.current = null;
        void api.guideCancel();
      }}
    />
  );
}

export function Rulers({ width, height }: { width: number; height: number }) {
  return (
    <>
      <div className="ruler-corner" style={{ width: RULER_SIZE, height: RULER_SIZE }} />
      <Ruler orientation="top" length={width} />
      <Ruler orientation="left" length={height} />
    </>
  );
}
