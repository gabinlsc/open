import { api } from "../api";
import { activeDoc, useStore } from "../store";

const PRESETS = [0.125, 0.25, 0.5, 1, 2, 4, 8];

export function StatusBar() {
  const snapshot = useStore((s) => s.snapshot);
  const doc = useStore(activeDoc);
  const cursor = useStore((s) => s.cursor);
  const run = useStore((s) => s.run);
  const version = useStore((s) => s.version);
  const zoom = snapshot?.view.zoom ?? 1;
  const percent = Math.round(zoom * 1000) / 10;
  return (
    <div className="statusbar" role="status">
      {doc ? (
        <>
          <label className="zoom-select">
            Zoom
            <select
              value={PRESETS.includes(zoom) ? String(zoom) : "custom"}
              onChange={(e) => e.target.value !== "custom" && void run(() => api.zoomTo(Number(e.target.value)))}
            >
              {!PRESETS.includes(zoom) && <option value="custom">{percent} %</option>}
              {PRESETS.map((p) => (
                <option key={p} value={p}>{p * 100} %</option>
              ))}
            </select>
          </label>
          <span>{doc.width} × {doc.height} px</span>
          {cursor?.inside && <span>X {Math.floor(cursor.x)} · Y {Math.floor(cursor.y)}</span>}
        </>
      ) : (
        <span>Aucun document</span>
      )}
      <span className="spacer" />
      <span className="muted">OpenPics {version}</span>
    </div>
  );
}
