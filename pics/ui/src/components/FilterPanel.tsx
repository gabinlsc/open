import { useEffect, useRef } from "react";
import { api } from "../api";
import { cancelFilter, commitFilter } from "../actions";
import { useStore } from "../store";

/** Panneau du filtre en cours : les curseurs relancent l'aperçu (sans historique) ; « Appliquer » valide. */
export function FilterPanel() {
  const filter = useStore((s) => s.filter);
  const info = useStore((s) => s.snapshot?.filters.find((f) => f.id === filter?.id));
  const run = useStore((s) => s.run);
  const layerId = useStore((s) => s.snapshot?.activeLayerId);
  const docId = useStore((s) => s.snapshot?.activeId);
  const origin = useRef<{ layer: unknown; doc: unknown } | null>(null);
  const timer = useRef<number>(0);
  const busy = useRef(false);
  const pending = useRef<number[] | null>(null);

  // Un seul aperçu à la fois : pendant un calcul, on garde seulement la dernière demande.
  const flush = async (id: string) => {
    if (busy.current) return;
    busy.current = true;
    try {
      while (pending.current) {
        const values = pending.current;
        pending.current = null;
        await run(() => api.filterPreview(id, values));
      }
    } finally {
      busy.current = false;
    }
  };

  useEffect(() => () => window.clearTimeout(timer.current), []);

  // Changer de calque ou de document abandonne l'aperçu côté cœur : on referme aussi le panneau.
  useEffect(() => {
    if (!filter) {
      origin.current = null;
      return;
    }
    if (origin.current === null) origin.current = { layer: layerId, doc: docId };
    else if (origin.current.layer !== layerId || origin.current.doc !== docId) useStore.getState().setFilter(null);
  }, [filter, layerId, docId]);

  if (!filter || !info) return null;

  const setValue = (i: number, v: number) => {
    const values = filter.values.map((x, k) => (k === i ? v : x));
    useStore.getState().setFilter({ id: filter.id, values });
    pending.current = values;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void flush(filter.id), 120);
  };

  return (
    <section aria-labelledby="p-filter" className="filter-panel">
      <h3 id="p-filter">{info.category} › {info.label}</h3>
      {info.params.length === 0 && <p className="muted">Ce filtre n'a pas de réglage.</p>}
      {info.params.map((p, i) => {
        const v = filter.values[i] ?? p.value;
        const step = p.max - p.min > 20 ? 1 : (p.max - p.min) / 200;
        return (
          <label className="slider" key={p.id}>
            <span title={p.label}>{p.label}</span>
            <input type="range" min={p.min} max={p.max} step={step} value={v} onChange={(e) => setValue(i, Number(e.target.value))} />
            <output>{Math.round(v * 100) / 100}</output>
          </label>
        );
      })}
      <div className="row wrap">
        <button className="btn small" onClick={() => void commitFilter()}>Appliquer</button>
        <button className="btn small secondary" onClick={() => void cancelFilter()}>Annuler</button>
      </div>
      <p className="muted">Aperçu en direct sur le calque actif (et la sélection, s'il y en a une).</p>
    </section>
  );
}
