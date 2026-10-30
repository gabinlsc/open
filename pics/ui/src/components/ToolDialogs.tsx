import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api } from "../api";
import { openPaths } from "../actions";
import { activeDoc, errorText, useStore } from "../store";
import { Modal } from "./Modal";

const close = () => useStore.getState().setDialog(null);
const MAX = 65536;

/** Taille de l'image : tous les calques sont rééchantillonnés (Lanczos), en une étape d'historique. */
export function ResizeDialog() {
  const doc = useStore(activeDoc);
  const [w, setW] = useState(String(doc?.width ?? 1));
  const [h, setH] = useState(String(doc?.height ?? 1));
  const [lock, setLock] = useState(true);
  if (!doc) return null;
  const ratio = doc.width / doc.height;
  const valid = [w, h].every((v) => Number.isInteger(Number(v)) && Number(v) >= 1 && Number(v) <= MAX);
  const changeW = (v: string) => {
    setW(v);
    if (lock && Number.isFinite(Number(v)) && Number(v) > 0) setH(String(Math.max(1, Math.round(Number(v) / ratio))));
  };
  const changeH = (v: string) => {
    setH(v);
    if (lock && Number.isFinite(Number(v)) && Number(v) > 0) setW(String(Math.max(1, Math.round(Number(v) * ratio))));
  };
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (valid && (await useStore.getState().run(() => api.imageResize(Number(w), Number(h))))) close();
  };
  return (
    <Modal title="Taille de l'image" onClose={close}>
      <form onSubmit={submit} className="form">
        <label>Largeur (px)<input inputMode="numeric" value={w} onChange={(e) => changeW(e.target.value)} /></label>
        <label>Hauteur (px)<input inputMode="numeric" value={h} onChange={(e) => changeH(e.target.value)} /></label>
        <label className="inline"><input type="checkbox" checked={lock} onChange={(e) => setLock(e.target.checked)} /> Conserver les proportions</label>
        <p className="muted">Les textes et formes vectorielles deviennent des calques de pixels. Les styles de calque (ombres…) ne sont pas mis à l'échelle.</p>
        <div className="actions">
          <button type="button" className="btn secondary" onClick={close}>Annuler</button>
          <button type="submit" className="btn" disabled={!valid}>Redimensionner</button>
        </div>
      </form>
    </Modal>
  );
}

const SCALES = [0.5, 1, 2, 3, 4];

/** Exporte nom@1x.png, nom@2x.png… en une fois. */
export function ExportScalesDialog() {
  const doc = useStore(activeDoc);
  const [ext, setExt] = useState("png");
  const [chosen, setChosen] = useState<number[]>([1, 2]);
  const [busy, setBusy] = useState(false);
  const toggle = (s: number) => setChosen((c) => (c.includes(s) ? c.filter((x) => x !== s) : [...c, s].sort((a, b) => a - b)));
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const path = await save({ defaultPath: `${doc?.name ?? "image"}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!path) return;
    setBusy(true);
    try {
      const files = await api.exportScales(path, chosen, ext === "jpg" ? 90 : null);
      useStore.getState().notify({ kind: "info", text: `${files.length} fichier(s) exporté(s).` });
      close();
    } catch (err) {
      useStore.getState().notify({ kind: "error", text: errorText(err) });
      setBusy(false);
    }
  };
  return (
    <Modal title="Exporter à plusieurs échelles" onClose={close}>
      <form onSubmit={submit} className="form">
        <fieldset className="checks">
          <legend>Échelles</legend>
          {SCALES.map((s) => (
            <label key={s} className="inline"><input type="checkbox" checked={chosen.includes(s)} onChange={() => toggle(s)} /> {s}×</label>
          ))}
        </fieldset>
        <label>
          Format
          <select value={ext} onChange={(e) => setExt(e.target.value)}>
            <option value="png">PNG</option>
            <option value="jpg">JPEG</option>
            <option value="webp">WebP</option>
          </select>
        </label>
        <p className="muted">Les fichiers sont nommés <code>nom@2x.png</code>, <code>nom@0.5x.png</code>… à côté du fichier choisi.</p>
        <div className="actions">
          <button type="button" className="btn secondary" onClick={close}>Annuler</button>
          <button type="submit" className="btn" disabled={busy || chosen.length === 0}>{busy ? "Export…" : "Exporter…"}</button>
        </div>
      </form>
    </Modal>
  );
}

const EXAMPLE = `// Exemple : noir et blanc, puis réduction à la moitié
filter("threshold", [0.5]);
resize(width / 2, height / 2);`;

/** Éditeur de scripts (Rhai), enregistreur d'actions et actions sauvegardées. */
export function ScriptsDialog() {
  const [source, setSource] = useState(EXAMPLE);
  const [recording, setRecording] = useState(false);
  const [saved, setSaved] = useState<string[]>([]);
  const [name, setName] = useState("");
  const [message, setMessage] = useState<{ kind: "error" | "info"; text: string } | null>(null);
  useEffect(() => {
    api.actionsList().then(setSaved).catch(() => undefined);
  }, []);
  const guard = async (f: () => Promise<void>) => {
    try {
      await f();
    } catch (e) {
      setMessage({ kind: "error", text: errorText(e) });
    }
  };
  const run = () => guard(async () => {
    if (await useStore.getState().run(() => api.scriptRun(source))) setMessage({ kind: "info", text: "Script exécuté." });
  });
  const toggleRecord = () => guard(async () => {
    if (recording) {
      const script = await api.recordStop();
      setRecording(false);
      if (script) setSource(script);
      setMessage({ kind: "info", text: script ? "Enregistrement terminé : le script est dans l'éditeur." : "Aucune action enregistrée." });
    } else {
      await api.recordStart();
      setRecording(true);
      setMessage({ kind: "info", text: "Enregistrement en cours : faites vos actions, puis revenez ici." });
    }
  });
  return (
    <Modal title="Scripts et actions" onClose={close} wide>
      <div className="form">
        <p className="hint">
          Fonctions : <code>filter("id", [valeurs])</code>, <code>op("invert" | "flip_h" | "flip_v")</code>, <code>select_all()</code>,{" "}
          <code>deselect()</code>, <code>invert_selection()</code>, <code>select_subject(tolérance)</code>, <code>remove_background(tolérance)</code>,{" "}
          <code>inpaint()</code>, <code>resize(l, h)</code>, <code>new_layer()</code>. Variables : <code>width</code>, <code>height</code>. Le script n'a
          accès ni aux fichiers ni au réseau.
        </p>
        <textarea rows={9} value={source} onChange={(e) => setSource(e.target.value)} spellCheck={false} className="code" aria-label="Script" />
        {message && <p role={message.kind === "error" ? "alert" : "status"} className={message.kind === "error" ? "error" : "muted"}>{message.text}</p>}
        <div className="row wrap">
          <button className="btn" onClick={() => void run()} disabled={recording}>Exécuter</button>
          <button className="btn secondary" onClick={() => void toggleRecord()}>{recording ? "Arrêter l'enregistrement" : "Enregistrer mes actions"}</button>
        </div>
        <fieldset className="checks">
          <legend>Actions sauvegardées</legend>
          <div className="row">
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Nom de l'action" aria-label="Nom de l'action" />
            <button className="btn small secondary" disabled={!name.trim()} onClick={() => void guard(async () => setSaved(await api.actionsSave(name.trim(), source)))}>Sauvegarder</button>
          </div>
          <ul className="model-list">
            {saved.map((n) => (
              <li key={n}>
                <strong>{n}</strong>
                <div className="row">
                  <button className="btn small secondary" onClick={() => void guard(async () => { setSource(await api.actionsLoad(n)); setName(n); })}>Charger</button>
                  <button className="btn small secondary" onClick={() => void guard(async () => setSaved(await api.actionsDelete(n)))}>Supprimer</button>
                </div>
              </li>
            ))}
          </ul>
        </fieldset>
        <div className="actions"><button className="btn" onClick={close}>Fermer</button></div>
      </div>
    </Modal>
  );
}

const baseName = (p: string) => p.split(/[\\/]/).pop() ?? p;

/** Explorateur simple : les images d'un dossier, à ouvrir en un clic. */
export function BrowserDialog() {
  const [dir, setDir] = useState<string | null>(null);
  const [files, setFiles] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const pick = async () => {
    const p = await open({ directory: true, multiple: false });
    if (typeof p !== "string") return;
    try {
      setFiles(await api.folderImages(p));
      setDir(p);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  };
  return (
    <Modal title="Explorateur de dossier" onClose={close} wide>
      <div className="form">
        <div className="row">
          <button className="btn small secondary" onClick={() => void pick()}>Choisir un dossier…</button>
          <span>{dir ?? "—"}</span>
        </div>
        {error && <p role="alert" className="error">{error}</p>}
        {dir && files.length === 0 && !error && <p>Aucune image dans ce dossier.</p>}
        <ul className="model-list">
          {files.map((f) => (
            <li key={f}>
              <strong>{baseName(f)}</strong>
              <div className="row">
                <button className="btn small" onClick={() => { close(); void openPaths([f]); }}>Ouvrir</button>
              </div>
            </li>
          ))}
        </ul>
        {files.length > 1 && (
          <button className="btn secondary" onClick={() => { close(); void openPaths(files); }}>Tout ouvrir ({files.length})</button>
        )}
        <div className="actions"><button className="btn" onClick={close}>Fermer</button></div>
      </div>
    </Modal>
  );
}
