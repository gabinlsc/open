import { useEffect, useState } from "react";
import { api, type LayerDto, type LayerPropsDto } from "../api";
import { useStore } from "../store";
import { StylePanel } from "./StylePanel";

function propsOf(l: LayerDto, patch: Partial<LayerPropsDto> = {}): LayerPropsDto {
  return { name: l.name, visible: l.visible, opacity: l.opacity, fill: l.fill, blend: l.blend, locks: l.locks, clipToBelow: l.clipToBelow, ...patch };
}

/** Curseur 0–100 % : n'envoie au cœur (donc à l'historique) qu'à la fin du geste, pas à chaque pas. */
function Percent({ label, value, onCommit }: { label: string; value: number; onCommit: (v: number) => void }) {
  const [local, setLocal] = useState(Math.round(value * 100));
  useEffect(() => setLocal(Math.round(value * 100)), [value]);
  const commit = () => local !== Math.round(value * 100) && onCommit(local / 100);
  return (
    <label className="percent">
      <span>{label}</span>
      <input type="range" min={0} max={100} value={local} onChange={(e) => setLocal(Number(e.target.value))} onPointerUp={commit} onKeyUp={commit} onBlur={commit} />
      <output>{local} %</output>
    </label>
  );
}

function RenameField({ layer }: { layer: LayerDto }) {
  const run = useStore((s) => s.run);
  const [name, setName] = useState(layer.name);
  useEffect(() => setName(layer.name), [layer.name, layer.id]);
  const commit = () => {
    if (name.trim() && name.trim() !== layer.name) void run(() => api.layerSetProps(layer.id, propsOf(layer, { name })));
    else setName(layer.name);
  };
  return (
    <label className="field">
      Nom
      <input value={name} onChange={(e) => setName(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

export function LayerPanel() {
  const snapshot = useStore((s) => s.snapshot);
  const run = useStore((s) => s.run);
  if (!snapshot || snapshot.docs.length === 0) return null;
  const { layers, activeLayerId, blendModes } = snapshot;
  const active = layers.find((l) => l.id === activeLayerId) ?? null;
  const stackIndex = (id: number) => layers.length - 1 - layers.findIndex((l) => l.id === id);
  const patch = (l: LayerDto, p: Partial<LayerPropsDto>) => void run(() => api.layerSetProps(l.id, propsOf(l, p)));

  return (
    <section aria-labelledby="p-layers">
      <h3 id="p-layers">Calques</h3>
      <ul className="layers" role="listbox" aria-label="Pile de calques, du haut vers le bas">
        {layers.map((l) => (
          <li key={l.id} role="option" aria-selected={l.id === activeLayerId} className={l.id === activeLayerId ? "layer active" : "layer"}>
            <button
              className="eye"
              aria-pressed={l.visible}
              aria-label={l.visible ? `Masquer ${l.name}` : `Afficher ${l.name}`}
              title={l.visible ? "Masquer" : "Afficher"}
              onClick={() => patch(l, { visible: !l.visible })}
            >
              {l.visible ? "◉" : "○"}
            </button>
            <button className="layer-name" onClick={() => void run(() => api.layerSelect(l.id))}>
              {l.clipToBelow && <span aria-label="écrêté" title="Masque d'écrêtage">↳ </span>}
              {l.name}
              {l.hasMask && <span className={l.maskEnabled ? "badge" : "badge off"} title="Masque de calque">masque</span>}
              {(l.locks.all || l.locks.pixels || l.locks.position) && <span className="badge" title="Verrouillé">verrou</span>}
            </button>
          </li>
        ))}
      </ul>
      <div className="row wrap">
        <button className="btn small secondary" onClick={() => void run(api.layerAdd)}>Nouveau</button>
        <button className="btn small secondary" disabled={!active || layers.length <= 1} onClick={() => active && void run(() => api.layerRemove(active.id))}>Supprimer</button>
        <button className="btn small ghost" aria-label="Monter le calque" disabled={!active || stackIndex(active.id) >= layers.length - 1} onClick={() => active && void run(() => api.layerMove(active.id, stackIndex(active.id) + 1))}>↑</button>
        <button className="btn small ghost" aria-label="Descendre le calque" disabled={!active || stackIndex(active.id) <= 0} onClick={() => active && void run(() => api.layerMove(active.id, stackIndex(active.id) - 1))}>↓</button>
      </div>

      {active && (
        <div className="layer-props">
          <RenameField layer={active} />
          <label className="field">
            Mode de fusion
            <select value={active.blend} onChange={(e) => patch(active, { blend: e.target.value })}>
              {blendModes.map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
            </select>
          </label>
          <Percent label="Opacité" value={active.opacity} onCommit={(v) => patch(active, { opacity: v })} />
          <Percent label="Remplissage" value={active.fill} onCommit={(v) => patch(active, { fill: v })} />
          <fieldset className="checks">
            <legend>Verrous et écrêtage</legend>
            <label><input type="checkbox" checked={active.locks.pixels} onChange={(e) => patch(active, { locks: { ...active.locks, pixels: e.target.checked } })} /> Pixels</label>
            <label><input type="checkbox" checked={active.locks.position} onChange={(e) => patch(active, { locks: { ...active.locks, position: e.target.checked } })} /> Position</label>
            <label><input type="checkbox" checked={active.locks.all} onChange={(e) => patch(active, { locks: { ...active.locks, all: e.target.checked } })} /> Tout</label>
            <label><input type="checkbox" checked={active.clipToBelow} onChange={(e) => patch(active, { clipToBelow: e.target.checked })} /> Écrêter sur le calque du dessous</label>
          </fieldset>
          <StylePanel layer={active} />
          <div className="row wrap">
            {active.hasMask ? (
              <>
                <button className="btn small secondary" onClick={() => void run(() => api.maskSetEnabled(active.id, !active.maskEnabled))}>{active.maskEnabled ? "Désactiver le masque" : "Activer le masque"}</button>
                <button className="btn small ghost" onClick={() => void run(() => api.maskRemove(active.id))}>Supprimer le masque</button>
              </>
            ) : (
              <>
                <button className="btn small secondary" onClick={() => void run(() => api.maskAdd(active.id, true))}>Masque : tout révéler</button>
                <button className="btn small secondary" onClick={() => void run(() => api.maskAdd(active.id, false))}>Masque : tout masquer</button>
              </>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
