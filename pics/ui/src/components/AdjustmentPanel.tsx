import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api, type AdjustmentDto, type LayerDto } from "../api";
import { addPoint, normalizePoints, removePoint, svgPath, type Point } from "../curves";
import { useStore } from "../store";
import { parseNumber } from "../styles";

function Slider({ layerId, p }: { layerId: number; p: AdjustmentDto["params"][number] }) {
  const run = useStore((s) => s.run);
  const [local, setLocal] = useState(p.value);
  useEffect(() => setLocal(p.value), [p.value]);
  const step = (p.max - p.min) / 400;
  const commit = () => local !== p.value && void run(() => api.adjustmentSetParam(layerId, p.id, local));
  return (
    <label className="slider">
      <span title={p.label}>{p.label}</span>
      <input type="range" min={p.min} max={p.max} step={step} value={local} onChange={(e) => setLocal(Number(e.target.value))} onPointerUp={commit} onKeyUp={commit} onBlur={commit} />
      <output>{Math.round(local * 100) / 100}</output>
    </label>
  );
}

function CurveEditor({ layerId, points }: { layerId: number; points: Point[] }) {
  const run = useStore((s) => s.run);
  const commit = (next: Point[]) => void run(() => api.adjustmentSetCurve(layerId, normalizePoints(next)));
  const pts = normalizePoints(points);
  return (
    <div className="curve">
      <svg viewBox="0 0 120 120" width="100%" role="img" aria-label="Aperçu de la courbe">
        <rect width="120" height="120" fill="none" stroke="currentColor" opacity="0.25" />
        <path d="M0 120 L120 0" stroke="currentColor" opacity="0.2" fill="none" />
        <path d={svgPath(pts, 120)} stroke="var(--accent)" strokeWidth="2" fill="none" />
        {pts.map(([x, y], i) => <circle key={i} cx={x * 120} cy={(1 - y) * 120} r="3" fill="var(--accent)" />)}
      </svg>
      {pts.map(([x, y], i) => (
        <div className="row" key={i}>
          <PointField label="Entrée" value={x} disabled={i === 0 || i === pts.length - 1} onCommit={(v) => commit(pts.map((p, k) => (k === i ? [v, p[1]] : p)))} />
          <PointField label="Sortie" value={y} onCommit={(v) => commit(pts.map((p, k) => (k === i ? [p[0], v] : p)))} />
          <button className="btn small ghost" aria-label={`Supprimer le point ${i + 1}`} disabled={i === 0 || i === pts.length - 1} onClick={() => commit(removePoint(pts, i))}>×</button>
        </div>
      ))}
      <button className="btn small secondary" onClick={() => commit(addPoint(pts))}>Ajouter un point</button>
    </div>
  );
}

function PointField({ label, value, disabled, onCommit }: { label: string; value: number; disabled?: boolean; onCommit: (v: number) => void }) {
  const [text, setText] = useState(String(Math.round(value * 1000) / 1000));
  useEffect(() => setText(String(Math.round(value * 1000) / 1000)), [value]);
  const commit = () => {
    const n = parseNumber(text, value, 0, 1);
    if (n !== value) onCommit(n);
  };
  return (
    <label className="num">
      <span>{label}</span>
      <input inputMode="decimal" value={text} disabled={disabled} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

function StopsEditor({ layerId, stops }: { layerId: number; stops: NonNullable<AdjustmentDto["stops"]> }) {
  const run = useStore((s) => s.run);
  const commit = (next: [number, string][]) => void run(() => api.adjustmentSetStops(layerId, next));
  const list: [number, string][] = stops.map((s) => [s.position, s.color]);
  return (
    <div className="curve">
      <div className="gradient-preview" style={{ background: `linear-gradient(90deg, ${stops.map((s) => `${s.color} ${Math.round(s.position * 100)}%`).join(", ")})` }} />
      {list.map(([pos, color], i) => (
        <div className="row" key={i}>
          <label className="num"><span>Couleur</span><input type="color" value={color} onChange={(e) => commit(list.map((s, k): [number, string] => (k === i ? [s[0], e.target.value] : s)))} /></label>
          <PointField label="Position" value={pos} onCommit={(v) => commit(list.map((s, k): [number, string] => (k === i ? [v, s[1]] : s)))} />
          <button className="btn small ghost" aria-label={`Supprimer l'arrêt ${i + 1}`} disabled={list.length <= 2} onClick={() => commit(list.filter((_, k) => k !== i))}>×</button>
        </div>
      ))}
      <button className="btn small secondary" disabled={list.length >= 16} onClick={() => commit([...list, [0.5, "#808080"]])}>Ajouter un arrêt</button>
    </div>
  );
}

/** Réglage du calque de réglage actif : curseurs générés depuis la table de paramètres du cœur. */
export function AdjustmentPanel({ layer, adj }: { layer: LayerDto; adj: AdjustmentDto }) {
  const run = useStore((s) => s.run);
  const layers = useStore((s) => s.snapshot?.layers ?? []);
  const rasters = layers.filter((l) => !l.isGroup && !l.adjustment);
  const [source, setSource] = useState<number | null>(null);
  const [reference, setReference] = useState<number | null>(null);
  const loadLut = async () => {
    const picked = await open({ multiple: false, filters: [{ name: "LUT 3D", extensions: ["cube"] }] });
    if (typeof picked === "string") await run(() => api.adjustmentLoadLut(layer.id, picked));
  };
  return (
    <fieldset className="checks adjustment">
      <legend>{adj.label}</legend>
      {adj.params.map((p) => <Slider key={`${layer.id}-${p.id}`} layerId={layer.id} p={p} />)}
      {adj.curve && <CurveEditor layerId={layer.id} points={adj.curve as Point[]} />}
      {adj.stops && <StopsEditor layerId={layer.id} stops={adj.stops} />}
      {adj.lutSize !== null && (
        <div className="row wrap">
          <button className="btn small secondary" onClick={() => void loadLut()}>Charger un fichier .cube…</button>
          <span className="muted">Table {adj.lutSize}³</span>
        </div>
      )}
      {adj.isColorMatch && (
        <>
          <label className="field">Calque à corriger
            <select value={source ?? ""} onChange={(e) => setSource(e.target.value === "" ? null : Number(e.target.value))}>
              <option value="">Choisir…</option>
              {rasters.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
            </select>
          </label>
          <label className="field">Calque de référence
            <select value={reference ?? ""} onChange={(e) => setReference(e.target.value === "" ? null : Number(e.target.value))}>
              <option value="">Choisir…</option>
              {rasters.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
            </select>
          </label>
          <button className="btn small secondary" disabled={source === null || reference === null} onClick={() => source !== null && reference !== null && void run(() => api.adjustmentMatch(layer.id, source, reference))}>Calculer la correspondance</button>
        </>
      )}
    </fieldset>
  );
}
