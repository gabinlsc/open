import { useEffect, useState } from "react";
import { api, type BlendInfo, type LayerDto, type StyleDto } from "../api";
import { useStore } from "../store";
import { parseNumber, replaceStyle, STYLE_KINDS, toggleStyle } from "../styles";

/** Champ numérique : n'envoie la valeur (donc un état d'historique) qu'à la validation. */
function Num({ label, value, min, max, step = 1, onCommit }: { label: string; value: number; min: number; max: number; step?: number; onCommit: (v: number) => void }) {
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
      <input inputMode="decimal" step={step} value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

function Color({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <label className="num">
      <span>{label}</span>
      <input type="color" value={value} onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}

function Blend({ value, modes, onChange }: { value: string; modes: BlendInfo[]; onChange: (v: string) => void }) {
  return (
    <label className="num wide">
      <span>Mode</span>
      <select value={value} onChange={(e) => onChange(e.target.value)}>
        {modes.map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
      </select>
    </label>
  );
}

function Editor({ style, modes, onChange }: { style: StyleDto; modes: BlendInfo[]; onChange: (s: StyleDto) => void }) {
  const opacity = <Num label="Opacité %" value={Math.round(style.opacity * 100)} min={0} max={100} onCommit={(v) => onChange({ ...style, opacity: v / 100 })} />;
  switch (style.kind) {
    case "drop_shadow":
      return (
        <div className="style-fields">
          <Color label="Couleur" value={style.color} onChange={(color) => onChange({ ...style, color })} />
          {opacity}
          <Num label="Angle °" value={style.angleDeg} min={-360} max={360} onCommit={(angleDeg) => onChange({ ...style, angleDeg })} />
          <Num label="Distance" value={style.distance} min={0} max={500} onCommit={(distance) => onChange({ ...style, distance })} />
          <Num label="Flou" value={style.size} min={0} max={250} onCommit={(size) => onChange({ ...style, size })} />
        </div>
      );
    case "outer_glow":
      return (
        <div className="style-fields">
          <Color label="Couleur" value={style.color} onChange={(color) => onChange({ ...style, color })} />
          {opacity}
          <Num label="Taille" value={style.size} min={0} max={250} onCommit={(size) => onChange({ ...style, size })} />
        </div>
      );
    case "stroke":
      return (
        <div className="style-fields">
          <Color label="Couleur" value={style.color} onChange={(color) => onChange({ ...style, color })} />
          {opacity}
          <Num label="Épaisseur" value={style.size} min={0} max={250} onCommit={(size) => onChange({ ...style, size })} />
          <label className="num wide">
            <span>Position</span>
            <select value={style.position} onChange={(e) => onChange({ ...style, position: e.target.value as typeof style.position })}>
              <option value="outside">Extérieur</option>
              <option value="center">Centré</option>
              <option value="inside">Intérieur</option>
            </select>
          </label>
        </div>
      );
    case "color_overlay":
      return (
        <div className="style-fields">
          <Color label="Couleur" value={style.color} onChange={(color) => onChange({ ...style, color })} />
          {opacity}
          <Blend value={style.blend} modes={modes} onChange={(blend) => onChange({ ...style, blend })} />
        </div>
      );
    case "gradient_overlay":
      return (
        <div className="style-fields">
          <Color label="Début" value={style.from} onChange={(from) => onChange({ ...style, from })} />
          <Color label="Fin" value={style.to} onChange={(to) => onChange({ ...style, to })} />
          {opacity}
          <Num label="Angle °" value={style.angleDeg} min={-360} max={360} onCommit={(angleDeg) => onChange({ ...style, angleDeg })} />
          <Blend value={style.blend} modes={modes} onChange={(blend) => onChange({ ...style, blend })} />
        </div>
      );
  }
}

export function StylePanel({ layer }: { layer: LayerDto }) {
  const run = useStore((s) => s.run);
  const modes = useStore((s) => s.snapshot?.blendModes ?? []);
  const set = (styles: StyleDto[]) => void run(() => api.layerSetStyles(layer.id, styles));
  return (
    <fieldset className="checks styles">
      <legend>Styles de calque</legend>
      {STYLE_KINDS.map(({ kind, label }) => {
        const style = layer.styles.find((s) => s.kind === kind);
        return (
          <div key={kind} className="style-row">
            <label>
              <input type="checkbox" checked={style !== undefined} onChange={(e) => set(toggleStyle(layer.styles, kind, e.target.checked))} /> {label}
            </label>
            {style && <Editor style={style} modes={modes} onChange={(next) => set(replaceStyle(layer.styles, next))} />}
          </div>
        );
      })}
    </fieldset>
  );
}
