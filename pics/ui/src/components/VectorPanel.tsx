import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api, type LayerDto, type ShapeDto, type TextDto } from "../api";
import { errorText, useStore } from "../store";
import { parseNumber } from "../styles";

let fontCache: string[] | null = null;

function useFonts(): string[] {
  const [fonts, setFonts] = useState<string[]>(fontCache ?? []);
  useEffect(() => {
    if (fontCache) return;
    void api.fonts().then((f) => {
      fontCache = f;
      setFonts(f);
    }).catch(() => undefined);
  }, []);
  return fonts;
}

function Num({ label, value, min, max, onCommit }: { label: string; value: number; min: number; max: number; onCommit: (v: number) => void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const n = parseNumber(text, value, min, max);
    setText(String(n));
    if (n !== value) onCommit(n);
  };
  return (
    <label className="num">
      <span>{label}</span>
      <input inputMode="decimal" value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

/** Édition d'un calque de texte : le texte est la source, le rendu est recalculé à chaque validation. */
export function TextPanel({ layer, text }: { layer: LayerDto; text: TextDto }) {
  const run = useStore((s) => s.run);
  const layers = useStore((s) => s.snapshot?.layers ?? []);
  const fonts = useFonts();
  const [draft, setDraft] = useState(text.text);
  useEffect(() => setDraft(text.text), [text.text, layer.id]);
  const set = (patch: Partial<TextDto>) => void run(() => api.textSet(layer.id, { ...text, ...patch }));
  const shapes = layers.filter((l) => l.shape);
  return (
    <fieldset className="checks">
      <legend>Texte</legend>
      <label className="field">
        Contenu
        <textarea rows={3} value={draft} onChange={(e) => setDraft(e.target.value)} onBlur={() => draft !== text.text && set({ text: draft })} />
      </label>
      <label className="field">
        Police
        <input list="fonts-list" value={text.font} placeholder="Police par défaut" onChange={() => undefined} onBlur={(e) => e.target.value !== text.font && set({ font: e.target.value })} defaultValue={text.font} key={`${layer.id}-${text.font}`} />
        <datalist id="fonts-list">{fonts.map((f) => <option key={f} value={f} />)}</datalist>
      </label>
      <div className="row">
        <Num label="Corps (px)" value={text.size} min={1} max={2000} onCommit={(size) => set({ size })} />
        <Num label="Interligne" value={text.lineHeight} min={0.5} max={5} onCommit={(lineHeight) => set({ lineHeight })} />
      </div>
      <div className="row">
        <label className="num"><span>Couleur</span><input type="color" value={text.color} onChange={(e) => set({ color: e.target.value })} /></label>
        <label className="inline"><input type="checkbox" checked={text.bold} onChange={(e) => set({ bold: e.target.checked })} /> Gras</label>
        <label className="inline"><input type="checkbox" checked={text.italic} onChange={(e) => set({ italic: e.target.checked })} /> Italique</label>
      </div>
      <label className="field">
        Alignement
        <select value={text.align} onChange={(e) => set({ align: e.target.value as TextDto["align"] })}>
          <option value="left">Gauche</option>
          <option value="center">Centré</option>
          <option value="right">Droite</option>
          <option value="justify">Justifié</option>
        </select>
      </label>
      <label className="inline">
        <input type="checkbox" checked={text.boxWidth !== null} onChange={(e) => set({ boxWidth: e.target.checked ? 400 : null })} /> Texte de paragraphe (retour à la ligne)
      </label>
      {text.boxWidth !== null && <Num label="Largeur du bloc (px)" value={text.boxWidth} min={1} max={100000} onCommit={(boxWidth) => set({ boxWidth })} />}
      <div className="row">
        <Num label="Position X" value={text.originX} min={-1e6} max={1e6} onCommit={(originX) => set({ originX })} />
        <Num label="Position Y" value={text.originY} min={-1e6} max={1e6} onCommit={(originY) => set({ originY })} />
      </div>
      <label className="field">
        Texte sur tracé
        <select value={text.hasPath ? "path" : ""} onChange={(e) => void run(() => api.textSetPath(layer.id, e.target.value === "" ? null : Number(e.target.value)))}>
          <option value="">{text.hasPath ? "Retirer le tracé" : "Aucun"}</option>
          {shapes.map((s) => <option key={s.id} value={s.id}>Suivre « {s.name} »</option>)}
        </select>
      </label>
    </fieldset>
  );
}

/** Édition d'une forme vectorielle : remplissage, contour, règle, opérations booléennes, sélection, SVG. */
export function ShapePanel({ layer, shape }: { layer: LayerDto; shape: ShapeDto }) {
  const run = useStore((s) => s.run);
  const layers = useStore((s) => s.snapshot?.layers ?? []);
  const [other, setOther] = useState<number | null>(null);
  const set = (patch: Partial<ShapeDto>) => void run(() => api.shapeSetStyle(layer.id, { ...shape, ...patch }));
  const others = layers.filter((l) => l.shape && l.id !== layer.id);
  const exportSvg = async () => {
    const path = await save({ defaultPath: `${layer.name}.svg`, filters: [{ name: "SVG", extensions: ["svg"] }] });
    if (!path) return;
    try {
      await api.shapeExportSvg(layer.id, path);
      useStore.getState().notify({ kind: "info", text: `Exporté : ${path}` });
    } catch (e) {
      useStore.getState().notify({ kind: "error", text: errorText(e) });
    }
  };
  return (
    <fieldset className="checks">
      <legend>Forme</legend>
      <label className="inline">
        <input type="checkbox" checked={shape.fill !== null} onChange={(e) => set({ fill: e.target.checked ? { color: "#000000", opacity: 1 } : null })} /> Remplissage
      </label>
      {shape.fill && (
        <div className="row">
          <label className="num"><span>Couleur</span><input type="color" value={shape.fill.color} onChange={(e) => set({ fill: { ...shape.fill!, color: e.target.value } })} /></label>
          <Num label="Opacité %" value={Math.round(shape.fill.opacity * 100)} min={0} max={100} onCommit={(v) => set({ fill: { ...shape.fill!, opacity: v / 100 } })} />
        </div>
      )}
      <label className="inline">
        <input type="checkbox" checked={shape.stroke !== null} onChange={(e) => set({ stroke: e.target.checked ? { color: "#2447d6", opacity: 1, width: 3, roundCaps: true } : null })} /> Contour
      </label>
      {shape.stroke && (
        <div className="row">
          <label className="num"><span>Couleur</span><input type="color" value={shape.stroke.color} onChange={(e) => set({ stroke: { ...shape.stroke!, color: e.target.value } })} /></label>
          <Num label="Épaisseur" value={shape.stroke.width} min={0} max={2000} onCommit={(width) => set({ stroke: { ...shape.stroke!, width } })} />
          <label className="inline"><input type="checkbox" checked={shape.stroke.roundCaps} onChange={(e) => set({ stroke: { ...shape.stroke!, roundCaps: e.target.checked } })} /> Bouts ronds</label>
        </div>
      )}
      <label className="inline"><input type="checkbox" checked={shape.evenOdd} onChange={(e) => set({ evenOdd: e.target.checked })} /> Règle pair-impair (trous)</label>
      <label className="field">
        Combiner avec
        <select value={other ?? ""} onChange={(e) => setOther(e.target.value === "" ? null : Number(e.target.value))}>
          <option value="">Choisir une forme…</option>
          {others.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
        </select>
      </label>
      <div className="row wrap">
        {([["union", "Union"], ["intersect", "Intersection"], ["subtract", "Soustraction"], ["xor", "Exclusion"]] as const).map(([op, label]) => (
          <button key={op} className="btn small secondary" disabled={other === null} onClick={() => other !== null && void run(() => api.shapeBoolean(layer.id, other, op))}>{label}</button>
        ))}
      </div>
      <div className="row wrap">
        <button className="btn small secondary" onClick={() => void run(() => api.shapeToSelection(layer.id))}>Tracé → sélection</button>
        <button className="btn small secondary" onClick={() => void exportSvg()}>Exporter en SVG…</button>
      </div>
    </fieldset>
  );
}
