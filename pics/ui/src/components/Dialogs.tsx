import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { api, CommandError } from "../api";
import { ACTIONS } from "../actions";
import { chordFromEvent, prettyChord } from "../keys";
import { activeDoc, errorText, useStore } from "../store";
import { Modal } from "./Modal";

const MAX_SIZE = 30000;
const PRESETS: { label: string; w: number; h: number }[] = [
  { label: "Full HD (1920 × 1080)", w: 1920, h: 1080 },
  { label: "4K UHD (3840 × 2160)", w: 3840, h: 2160 },
  { label: "Carré (1080 × 1080)", w: 1080, h: 1080 },
  { label: "A4 à 300 ppp (2480 × 3508)", w: 2480, h: 3508 },
];

export function NewDocumentDialog() {
  const close = () => useStore.getState().setDialog(null);
  const [name, setName] = useState("");
  const [w, setW] = useState("1920");
  const [h, setH] = useState("1080");
  const width = Number(w);
  const height = Number(h);
  const valid = [width, height].every((n) => Number.isInteger(n) && n >= 1 && n <= MAX_SIZE);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!valid) return;
    if (await useStore.getState().run(() => api.docNew(name.trim() || null, width, height))) close();
  };

  return (
    <Modal title="Nouveau document" onClose={close}>
      <form onSubmit={submit} className="form">
        <label>Nom<input value={name} onChange={(e) => setName(e.target.value)} placeholder="Sans titre" /></label>
        <label>
          Modèle
          <select
            defaultValue=""
            onChange={(e) => {
              const p = PRESETS[Number(e.target.value)];
              if (p) {
                setW(String(p.w));
                setH(String(p.h));
              }
            }}
          >
            <option value="" disabled>Choisir…</option>
            {PRESETS.map((p, i) => <option key={p.label} value={i}>{p.label}</option>)}
          </select>
        </label>
        <div className="row">
          <label>Largeur (px)<input inputMode="numeric" value={w} onChange={(e) => setW(e.target.value)} aria-invalid={!valid} /></label>
          <label>Hauteur (px)<input inputMode="numeric" value={h} onChange={(e) => setH(e.target.value)} aria-invalid={!valid} /></label>
        </div>
        {!valid && <p className="error" role="alert">Les dimensions doivent être des entiers de 1 à {MAX_SIZE}.</p>}
        <div className="actions">
          <button type="button" className="btn secondary" onClick={close}>Annuler</button>
          <button type="submit" className="btn" disabled={!valid}>Créer</button>
        </div>
      </form>
    </Modal>
  );
}

const EXPORT_FORMATS = [
  { ext: "png", label: "PNG (sans perte)" },
  { ext: "jpg", label: "JPEG" },
  { ext: "webp", label: "WebP (sans perte)" },
  { ext: "tiff", label: "TIFF" },
  { ext: "bmp", label: "BMP" },
  { ext: "gif", label: "GIF" },
];

export function ExportDialog() {
  const close = () => useStore.getState().setDialog(null);
  const doc = useStore(activeDoc);
  const [ext, setExt] = useState("png");
  const [quality, setQuality] = useState(90);
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const path = await save({ defaultPath: `${doc?.name ?? "image"}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return;
    setBusy(true);
    try {
      await api.docExport(path, ext === "jpg" ? quality : null);
      useStore.getState().notify({ kind: "info", text: `Exporté : ${path}` });
      close();
    } catch (err) {
      useStore.getState().notify({ kind: "error", text: errorText(err) });
      setBusy(false);
    }
  };

  return (
    <Modal title="Exporter en image" onClose={close}>
      <form onSubmit={submit} className="form">
        <label>
          Format
          <select value={ext} onChange={(e) => setExt(e.target.value)}>
            {EXPORT_FORMATS.map((f) => <option key={f.ext} value={f.ext}>{f.label}</option>)}
          </select>
        </label>
        {ext === "jpg" && (
          <label>
            Qualité : {quality}
            <input type="range" min={1} max={100} value={quality} onChange={(e) => setQuality(Number(e.target.value))} />
          </label>
        )}
        <p className="muted">Les calques visibles sont aplatis. Le document n'est pas modifié.</p>
        <div className="actions">
          <button type="button" className="btn secondary" onClick={close}>Annuler</button>
          <button type="submit" className="btn" disabled={busy}>{busy ? "Export…" : "Exporter…"}</button>
        </div>
      </form>
    </Modal>
  );
}

export function ConfirmCloseDialog({ id, name }: { id: number; name: string }) {
  const close = () => useStore.getState().setDialog(null);
  const closeDoc = async () => {
    close();
    await useStore.getState().run(() => api.docClose(id));
  };
  const saveThenClose = async () => {
    const st = useStore.getState();
    if (st.snapshot?.activeId !== id && !(await st.run(() => api.docActivate(id)))) return;
    close();
    await Promise.resolve(ACTIONS["file.save"]?.run());
    // Si l'utilisateur a annulé « Enregistrer sous », le document est resté modifié : on ne le ferme pas.
    const doc = useStore.getState().snapshot?.docs.find((d) => d.id === id);
    if (doc && !doc.dirty) await useStore.getState().run(() => api.docClose(id));
  };
  return (
    <Modal title="Fermer sans enregistrer ?" onClose={close}>
      <p>« {name} » contient des modifications non enregistrées.</p>
      <div className="actions">
        <button className="btn secondary" onClick={close}>Annuler</button>
        <button className="btn secondary danger" onClick={closeDoc}>Ne pas enregistrer</button>
        <button className="btn" onClick={saveThenClose}>Enregistrer</button>
      </div>
    </Modal>
  );
}

export function ShortcutsDialog() {
  const close = () => useStore.getState().setDialog(null);
  const keymap = useStore((s) => s.keymap);
  const [capturing, setCapturing] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const apply = async (action: string, chords: string[]) => {
    try {
      const dto = action === "*" ? await api.keymapReset() : await api.keymapSet(action, chords);
      useStore.getState().setKeymap(dto.bindings);
      setError(null);
    } catch (e) {
      setError(e instanceof CommandError ? e.message : errorText(e));
    }
  };

  useEffect(() => {
    if (!capturing) return;
    // Phase de capture : on intercepte la prochaine combinaison avant le gestionnaire global.
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") return setCapturing(null);
      const chord = chordFromEvent(e);
      if (!chord) return;
      const action = capturing;
      setCapturing(null);
      void apply(action, [chord]);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing]);

  const actions = Object.keys(keymap).sort((a, b) => (ACTIONS[a]?.label ?? a).localeCompare(ACTIONS[b]?.label ?? b, "fr"));

  return (
    <Modal title="Raccourcis clavier" onClose={close} wide>
      {error && <p className="error" role="alert">{error}</p>}
      <table className="shortcuts">
        <thead><tr><th>Action</th><th>Raccourci</th><th /></tr></thead>
        <tbody>
          {actions.map((a) => (
            <tr key={a}>
              <td>{ACTIONS[a]?.label ?? a}</td>
              <td>{capturing === a ? <em>Appuyez sur une combinaison… (Échap pour annuler)</em> : (keymap[a] ?? []).map((c) => <kbd key={c}>{prettyChord(c)}</kbd>)}</td>
              <td className="cell-actions">
                <button className="btn small secondary" onClick={() => setCapturing(capturing === a ? null : a)}>Modifier</button>
                <button className="btn small ghost" disabled={(keymap[a] ?? []).length === 0} onClick={() => void apply(a, [])}>Retirer</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="actions">
        <button className="btn secondary" onClick={() => void apply("*", [])}>Tout réinitialiser</button>
        <button className="btn" onClick={close}>Fermer</button>
      </div>
    </Modal>
  );
}

export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  if (!dialog) return null;
  switch (dialog.kind) {
    case "new":
      return <NewDocumentDialog />;
    case "export":
      return <ExportDialog />;
    case "shortcuts":
      return <ShortcutsDialog />;
    case "confirmClose":
      return <ConfirmCloseDialog id={dialog.id} name={dialog.name} />;
  }
}
