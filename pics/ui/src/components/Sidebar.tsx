import { t as tr } from "../i18n";
import { api } from "../api";
import { activeDoc, useStore } from "../store";
import { FilterPanel } from "./FilterPanel";
import { LayerPanel } from "./LayerPanel";
import { PaintOptions } from "./PaintOptions";
import { ToolOptions } from "./ToolOptions";

export function Sidebar() {
  const snapshot = useStore((s) => s.snapshot);
  const doc = useStore(activeDoc);
  const run = useStore((s) => s.run);
  if (!snapshot) return <aside className="sidebar" />;
  const { history, guides } = snapshot;
  return (
    <aside className="sidebar" aria-label={tr("Panneaux")}>
      <section aria-labelledby="p-info">
        <h3 id="p-info">{tr("Informations")}</h3>
        {doc ? (
          <dl className="info">
            <dt>{tr("Nom")}</dt><dd>{doc.name}</dd>
            <dt>{tr("Taille")}</dt><dd>{doc.width} × {doc.height} px</dd>
            <dt>{tr("Zoom")}</dt><dd>{Math.round(snapshot.view.zoom * 1000) / 10} %</dd>
            <dt>{tr("Fichier")}</dt><dd className="path">{doc.path ?? tr("Non enregistré")}</dd>
          </dl>
        ) : (
          <p className="muted">{tr("Aucun document.")}</p>
        )}
      </section>
      <FilterPanel />
      <ToolOptions />
      <PaintOptions />
      <LayerPanel />
      <section aria-labelledby="p-hist">
        <h3 id="p-hist">{tr("Historique")}</h3>
        {history.names.length === 0 ? (
          <p className="muted">{tr("Aucune action.")}</p>
        ) : (
          <ol className="history">
            {history.names.map((name, i) => (
              <li key={i} className={i < history.position ? "done" : "undone"} aria-current={i === history.position - 1 ? "step" : undefined}>
                {name}
              </li>
            ))}
          </ol>
        )}
        <div className="row">
          <button className="btn small secondary" disabled={!snapshot.canUndo} onClick={() => void run(api.undo)}>{tr("Annuler", "action")}</button>
          <button className="btn small secondary" disabled={!snapshot.canRedo} onClick={() => void run(api.redo)}>{tr("Rétablir")}</button>
        </div>
      </section>
      <section aria-labelledby="p-guides">
        <h3 id="p-guides">{tr("Repères")}</h3>
        {guides.length === 0 ? (
          <p className="muted">{tr("Glissez depuis une règle pour créer un repère.")}</p>
        ) : (
          <ul className="guides">
            {guides.map((g) => (
              <li key={g.id}>
                <span>{g.axis === "vertical" ? "Vertical" : "Horizontal"} · {Math.round(g.position * 100) / 100} px</span>
                <button className="btn small ghost" aria-label={`Supprimer le repère ${g.axis} à ${g.position}`} onClick={() => void run(() => api.guideRemove(g.id))}>×</button>
              </li>
            ))}
          </ul>
        )}
        {guides.length > 0 && <button className="btn small secondary" onClick={() => void run(api.guidesClear)}>{tr("Tout effacer")}</button>}
      </section>
    </aside>
  );
}
