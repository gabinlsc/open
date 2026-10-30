import { api } from "../api";
import { requestClose, ACTIONS } from "../actions";
import { useStore } from "../store";
import { CloseIcon, PlusIcon } from "./Icons";

export function Tabs() {
  const snapshot = useStore((s) => s.snapshot);
  const run = useStore((s) => s.run);
  if (!snapshot) return <div className="tabs" />;
  return (
    <div className="tabs" role="tablist" aria-label="Documents ouverts">
      {snapshot.docs.map((d) => {
        const active = d.id === snapshot.activeId;
        return (
          <div
            key={d.id}
            className={active ? "tab active" : "tab"}
            onMouseDown={(e) => {
              // Clic milieu = fermer, comme dans un navigateur.
              if (e.button === 1) {
                e.preventDefault();
                requestClose(d.id);
              }
            }}
          >
            <button role="tab" aria-selected={active} className="tab-main" title={d.path ?? d.name} onClick={() => !active && void run(() => api.docActivate(d.id))}>
              <span className="tab-name">{d.name}</span>
              {d.dirty && <span className="dirty" role="img" aria-label="modifié">●</span>}
            </button>
            <button className="tab-close" aria-label={`Fermer ${d.name}`} onClick={() => requestClose(d.id)}>
              <CloseIcon />
            </button>
          </div>
        );
      })}
      <button className="tab-new" aria-label="Nouveau document" title="Nouveau document" onClick={() => void ACTIONS["file.new"]?.run()}>
        <PlusIcon />
      </button>
    </div>
  );
}
