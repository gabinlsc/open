import { api } from "../api";
import { useStore } from "../store";

/** Réglages de l'outil courant (pour l'instant : la baguette magique). */
export function ToolOptions() {
  const tool = useStore((s) => s.snapshot?.tool);
  const options = useStore((s) => s.snapshot?.toolOptions);
  const run = useStore((s) => s.run);
  if (tool !== "magic_wand" || !options) return null;
  const set = (patch: Partial<typeof options>) => void run(() => api.setToolOptions({ ...options, ...patch }));
  return (
    <section aria-labelledby="p-tool">
      <h3 id="p-tool">Baguette magique</h3>
      <label className="percent">
        <span>Tolérance</span>
        <input type="range" min={0} max={255} value={options.tolerance} onChange={(e) => set({ tolerance: Number(e.target.value) })} />
        <output>{options.tolerance}</output>
      </label>
      <label className="inline"><input type="checkbox" checked={options.contiguous} onChange={(e) => set({ contiguous: e.target.checked })} /> Pixels contigus</label>
      <label className="inline"><input type="checkbox" checked={options.sampleAllLayers} onChange={(e) => set({ sampleAllLayers: e.target.checked })} /> Tous les calques</label>
      <p className="muted">Maj : ajouter · Alt : soustraire · Maj+Alt : intersecter</p>
    </section>
  );
}
