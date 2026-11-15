import { t as tr } from "../i18n";
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
      <span>{tr("Taille (px)")}</span>
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
      <h3 id="p-paint">{tr("Peinture")}</h3>
      {COLOR_TOOLS.includes(tool) && (
        <div className="row">
          <label className="num"><span>{tr("Premier plan")}</span><input type="color" value={o.foreground} onChange={(e) => set({ foreground: e.target.value })} /></label>
          <label className="num"><span>{tr("Arrière-plan")}</span><input type="color" value={o.background} onChange={(e) => set({ background: e.target.value })} /></label>
          <button className="btn small ghost" aria-label={tr("Permuter les couleurs")} title={tr("Permuter (X)")} onClick={() => set({ foreground: o.background, background: o.foreground })}>⇄</button>
        </div>
      )}
      {brush && <Size value={o.size} onCommit={(size) => set({ size })} />}
      {brush && tool !== "pencil" && <Percent label={tr("Dureté")} value={o.hardness} onCommit={(hardness) => set({ hardness })} />}
      <Percent label={tr("Opacité")} value={o.opacity} onCommit={(opacity) => set({ opacity })} />
      {brush && <Percent label={tr("Flux")} value={o.flow} onCommit={(flow) => set({ flow })} />}
      {brush && <Percent label={tr("Espacement")} value={Math.min(1, o.spacing)} onCommit={(spacing) => set({ spacing: Math.max(0.02, spacing) })} />}
      {brush && <Percent label={tr("Lissage")} value={o.smoothing} onCommit={(smoothing) => set({ smoothing })} />}
      {brush && (
        <>
          <label className="inline"><input type="checkbox" checked={o.sizeFromPressure} onChange={(e) => set({ sizeFromPressure: e.target.checked })} /> {tr("Taille selon la pression")}</label>
          <label className="inline"><input type="checkbox" checked={o.opacityFromPressure} onChange={(e) => set({ opacityFromPressure: e.target.checked })} /> {tr("Opacité selon la pression")}</label>
        </>
      )}
      {tool === "gradient" && (
        <>
          <label className="field">
            {tr("Forme")}
            <select value={o.gradient} onChange={(e) => set({ gradient: e.target.value as PaintOptionsDto["gradient"] })}>
              <option value="linear">{tr("Linéaire")}</option>
              <option value="radial">{tr("Radial")}</option>
              <option value="angular">{tr("Angulaire")}</option>
              <option value="reflected">{tr("Réfléchi")}</option>
              <option value="diamond">{tr("Losange")}</option>
            </select>
          </label>
          <label className="inline"><input type="checkbox" checked={o.gradientToTransparent} onChange={(e) => set({ gradientToTransparent: e.target.checked })} /> {tr("Vers la transparence")}</label>
        </>
      )}
      {(tool === "dodge" || tool === "burn") && (
        <label className="field">
          {tr("Tons")}
          <select value={o.tonalRange} onChange={(e) => set({ tonalRange: e.target.value as PaintOptionsDto["tonalRange"] })}>
            <option value="shadows">{tr("Ombres")}</option>
            <option value="midtones">{tr("Tons moyens")}</option>
            <option value="highlights">{tr("Hautes lumières")}</option>
          </select>
        </label>
      )}
      {tool === "liquify" && (
        <label className="field">
          {tr("Mode")}
          <select value={o.liquifyMode} onChange={(e) => set({ liquifyMode: e.target.value as PaintOptionsDto["liquifyMode"] })}>
            <option value="push">{tr("Pousser")}</option>
            <option value="bloat">{tr("Gonfler")}</option>
            <option value="pucker">{tr("Pincer")}</option>
            <option value="twirl">{tr("Tourbillon")}</option>
            <option value="reconstruct">{tr("Reconstruire")}</option>
          </select>
        </label>
      )}
      {tool === "sponge" && (
        <label className="inline"><input type="checkbox" checked={o.saturate} onChange={(e) => set({ saturate: e.target.checked })} /> {tr("Saturer (sinon désaturer)")}</label>
      )}
      {(tool === "clone_stamp" || tool === "heal") && <p className="muted">{tr("Alt + clic : définir la source, puis peindre.")}</p>}
      {tool === "patch" && <p className="muted">{tr("Faites une sélection, puis glissez-la vers la zone à copier.")}</p>}
    </section>
  );
}
