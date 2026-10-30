import { api } from "../api";
import { activeDoc, useStore } from "../store";

export function Sidebar() {
  const snapshot = useStore((s) => s.snapshot);
  const doc = useStore(activeDoc);
  const run = useStore((s) => s.run);
  if (!snapshot) return <aside className="sidebar" />;
  const { history, guides } = snapshot;
  return (
    <aside className="sidebar" aria-label="Panneaux">
      <section aria-labelledby="p-info">
        <h3 id="p-info">Informations</h3>
        {doc ? (
          <dl className="info">
            <dt>Nom</dt><dd>{doc.name}</dd>
            <dt>Taille</dt><dd>{doc.width} × {doc.height} px</dd>
            <dt>Zoom</dt><dd>{Math.round(snapshot.view.zoom * 1000) / 10} %</dd>
            <dt>Fichier</dt><dd className="path">{doc.path ?? "Non enregistré"}</dd>
          </dl>
        ) : (
          <p className="muted">Aucun document.</p>
        )}
      </section>
      <section aria-labelledby="p-hist">
        <h3 id="p-hist">Historique</h3>
        {history.names.length === 0 ? (
          <p className="muted">Aucune action.</p>
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
          <button className="btn small secondary" disabled={!snapshot.canUndo} onClick={() => void run(api.undo)}>Annuler</button>
          <button className="btn small secondary" disabled={!snapshot.canRedo} onClick={() => void run(api.redo)}>Rétablir</button>
        </div>
      </section>
      <section aria-labelledby="p-guides">
        <h3 id="p-guides">Repères</h3>
        {guides.length === 0 ? (
          <p className="muted">Glissez depuis une règle pour créer un repère.</p>
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
        {guides.length > 0 && <button className="btn small secondary" onClick={() => void run(api.guidesClear)}>Tout effacer</button>}
      </section>
    </aside>
  );
}
