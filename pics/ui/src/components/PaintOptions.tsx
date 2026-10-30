import { useEffect, useState } from "react";
import { api, type PaintOptionsDto, type Tool } from "../api";
import { useStore } from "../store";
import { parseNumber } from "../styles";
import { Percent } from "./LayerPanel";

const BRUSH_TOOLS: Tool[] = ["brush", "pencil", "eraser", "clone_stamp", "heal", "blur", "sharpen", "smudge", "dodge", "burn", "sponge", "spot_heal", "liquify"];
const OPACITY_TOOLS: Tool[] = [...BRUSH_TOOLS, "gradient", "bucket"];
const COLOR_TOOLS: Tool[] = ["brush", "pencil", "bucket", "gradient"];

function Size({ value, onCommit }: { value: number; onCommit: (v: number) => void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const n = parseNumber(text, value, 1, 2000);
    setText(String(n));
    if (n !== value) onCommit(n);
  };
  return (
    <label className="num">
      <span>Taille (px)</span>
      <input inputMode="decimal" value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

/** Couleurs et réglages des outils de peinture et de retouche. */
export function PaintOptions() {
  const tool = useStore((s) => s.snapshot?.tool);
  const o = useStore((s) => s.snapshot?.paintOptions);
  const run = useStore((s) => s.run);
  if (!tool || !o || !OPACITY_TOOLS.includes(tool)) return null;
  const set = (patch: Partial<PaintOptionsDto>) => void run(() => api.setPaintOptions({ ...o, ...patch }));
  const brush = BRUSH_TOOLS.includes(tool);
  return (
    <section aria-labelledby="p-paint">
      <h3 id="p-paint">Peinture</h3>
      {COLOR_TOOLS.includes(tool) && (
        <div className="row">
          <label className="num"><span>Premier plan</span><input type="color" value={o.foreground} onChange={(e) => set({ foreground: e.target.value })} /></label>
          <label className="num"><span>Arrière-plan</span><input type="color" value={o.background} onChange={(e) => set({ background: e.target.value })} /></label>
          <button className="btn small ghost" aria-label="Permuter les couleurs" title="Permuter (X)" onClick={() => set({ foreground: o.background, background: o.foreground })}>⇄</button>
        </div>
      )}
      {brush && <Size value={o.size} onCommit={(size) => set({ size })} />}
      {brush && tool !== "pencil" && <Percent label="Dureté" value={o.hardness} onCommit={(hardness) => set({ hardness })} />}
      <Percent label="Opacité" value={o.opacity} onCommit={(opacity) => set({ opacity })} />
      {brush && <Percent label="Flux" value={o.flow} onCommit={(flow) => set({ flow })} />}
      {brush && <Percent label="Espacement" value={Math.min(1, o.spacing)} onCommit={(spacing) => set({ spacing: Math.max(0.02, spacing) })} />}
      {brush && <Percent label="Lissage" value={o.smoothing} onCommit={(smoothing) => set({ smoothing })} />}
      {brush && (
        <>
          <label className="inline"><input type="checkbox" checked={o.sizeFromPressure} onChange={(e) => set({ sizeFromPressure: e.target.checked })} /> Taille selon la pression</label>
          <label className="inline"><input type="checkbox" checked={o.opacityFromPressure} onChange={(e) => set({ opacityFromPressure: e.target.checked })} /> Opacité selon la pression</label>
        </>
      )}
      {tool === "gradient" && (
        <>
          <label className="field">
            Forme
            <select value={o.gradient} onChange={(e) => set({ gradient: e.target.value as PaintOptionsDto["gradient"] })}>
              <option value="linear">Linéaire</option>
              <option value="radial">Radial</option>
              <option value="angular">Angulaire</option>
              <option value="reflected">Réfléchi</option>
              <option value="diamond">Losange</option>
            </select>
          </label>
          <label className="inline"><input type="checkbox" checked={o.gradientToTransparent} onChange={(e) => set({ gradientToTransparent: e.target.checked })} /> Vers la transparence</label>
        </>
      )}
      {(tool === "dodge" || tool === "burn") && (
        <label className="field">
          Tons
          <select value={o.tonalRange} onChange={(e) => set({ tonalRange: e.target.value as PaintOptionsDto["tonalRange"] })}>
            <option value="shadows">Ombres</option>
            <option value="midtones">Tons moyens</option>
            <option value="highlights">Hautes lumières</option>
          </select>
        </label>
      )}
      {tool === "liquify" && (
        <label className="field">
          Mode
          <select value={o.liquifyMode} onChange={(e) => set({ liquifyMode: e.target.value as PaintOptionsDto["liquifyMode"] })}>
            <option value="push">Pousser</option>
            <option value="bloat">Gonfler</option>
            <option value="pucker">Pincer</option>
            <option value="twirl">Tourbillon</option>
            <option value="reconstruct">Reconstruire</option>
          </select>
        </label>
      )}
      {tool === "sponge" && (
        <label className="inline"><input type="checkbox" checked={o.saturate} onChange={(e) => set({ saturate: e.target.checked })} /> Saturer (sinon désaturer)</label>
      )}
      {(tool === "clone_stamp" || tool === "heal") && <p className="muted">Alt + clic : définir la source, puis peindre.</p>}
      {tool === "patch" && <p className="muted">Faites une sélection, puis glissez-la vers la zone à copier.</p>}
    </section>
  );
}
