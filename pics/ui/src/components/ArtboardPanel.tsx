import { useEffect, useState } from "react";
import { api, type ArtboardDto } from "../api";
import { t as tr } from "../i18n";
import { activeDoc, useStore } from "../store";
import { parseNumber } from "../styles";

/** Champ numérique validé à la sortie (Entrée ou perte du focus). */
function Field({ label, value, min, max, onCommit }: { label: string; value: number; min: number; max: number; onCommit: (v: number) => void }) {
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const n = Math.round(parseNumber(text, value, min, max));
    setText(String(n));
    if (n !== value) onCommit(n);
  };
  return (
    <label className="num">
      <span>{label}</span>
      <input inputMode="numeric" value={text} onChange={(e) => setText(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
    </label>
  );
}

/** Plans de travail : zones du document exportées chacune comme une image (calques partagés). */
export function ArtboardPanel() {
  const snapshot = useStore((s) => s.snapshot);
  const doc = useStore(activeDoc);
  const run = useStore((s) => s.run);
  const [name, setName] = useState("");
  const list = snapshot?.artboards ?? [];
  const active = list.find((a) => a.id === snapshot?.activeArtboardId) ?? null;
  useEffect(() => setName(active?.name ?? ""), [active?.id, active?.name]);
  if (!snapshot || !doc || (list.length === 0 && snapshot.tool !== "artboard")) return null;
  const set = (patch: Partial<ArtboardDto>) => active && void run(() => api.artboardSet({ ...active, ...patch }));
  return (
    <section aria-labelledby="p-artboards">
      <h3 id="p-artboards">{tr("Plans de travail")}</h3>
      {list.length === 0 ? (
        <p className="muted">{tr("Glissez sur le document pour dessiner un plan de travail.")}</p>
      ) : (
        <ul className="layers" role="listbox" aria-label={tr("Plans de travail")}>
          {list.map((a) => (
            <li key={a.id} role="option" aria-selected={a.id === active?.id} className={a.id === active?.id ? "active" : ""}>
              <button className="layer-name" onClick={() => void run(() => api.artboardSelect(a.id))}>
                {a.name} <span className="muted">{a.w} × {a.h}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="row wrap">
        <button className="btn small secondary" onClick={() => void run(() => api.artboardAdd(0, 0, doc.width, doc.height, null))}>{tr("Document entier")}</button>
        {active && <button className="btn small secondary" onClick={() => void run(() => api.artboardRemove(active.id))}>{tr("Supprimer")}</button>}
      </div>
      {active && (
        <fieldset className="checks">
          <legend>{tr("Plan actif")}</legend>
          <label className="field">
            {tr("Nom")}
            <input value={name} onChange={(e) => setName(e.target.value)} onBlur={() => name !== active.name && set({ name })} onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()} />
          </label>
          <div className="row">
            <Field label="X" value={active.x} min={0} max={doc.width - 1} onCommit={(x) => set({ x })} />
            <Field label="Y" value={active.y} min={0} max={doc.height - 1} onCommit={(y) => set({ y })} />
          </div>
          <div className="row">
            <Field label={tr("Largeur")} value={active.w} min={1} max={doc.width} onCommit={(w) => set({ w })} />
            <Field label={tr("Hauteur")} value={active.h} min={1} max={doc.height} onCommit={(h) => set({ h })} />
          </div>
        </fieldset>
      )}
      {list.length > 0 && (
        <button className="btn small" onClick={() => useStore.getState().setDialog({ kind: "exportScales", artboards: true })}>{tr("Exporter les plans de travail…")}</button>
      )}
    </section>
  );
}
