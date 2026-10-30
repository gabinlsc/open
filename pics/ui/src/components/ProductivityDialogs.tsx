import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import { ACTIONS, runAction } from "../actions";
import { api, type BatchReport, type RecoveryDto } from "../api";
import { t } from "../i18n";
import { prettyChord } from "../keys";
import { errorText, useStore } from "../store";
import { Modal } from "./Modal";
import { rank, type PaletteItem } from "../palette";

const close = () => useStore.getState().setDialog(null);

/** Ctrl+K : toutes les actions et tous les filtres, par saisie approximative. */
export function PaletteDialog() {
  const snapshot = useStore((s) => s.snapshot);
  const keymap = useStore((s) => s.keymap);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);

  const items = useMemo<(PaletteItem & { run: () => void })[]>(() => {
    const out: (PaletteItem & { run: () => void })[] = [];
    for (const [id, a] of Object.entries(ACTIONS)) {
      if (id === "app.palette" || (a.enabled && !a.enabled())) continue;
      const chord = keymap[id]?.[0];
      out.push({ id, label: t(a.label), hint: chord ? prettyChord(chord) : undefined, run: () => runAction(id) });
    }
    for (const f of snapshot?.filters ?? []) {
      out.push({ id: `filter:${f.id}`, label: `${f.category} › ${f.label}`, run: () => void import("../actions").then((m) => m.startFilter(f.id)) });
    }
    return out;
  }, [snapshot, keymap]);

  const results = useMemo(() => rank(query, items, 30), [query, items]);
  useEffect(() => setIndex(0), [query]);
  useEffect(() => {
    listRef.current?.children[index]?.scrollIntoView({ block: "nearest" });
  }, [index]);

  const choose = (id: string) => {
    const item = items.find((i) => i.id === id);
    close();
    // La boîte se ferme d'abord : l'action peut ouvrir sa propre boîte de dialogue.
    if (item) setTimeout(item.run, 0);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((i) => Math.min(i + 1, results.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((i) => Math.max(i - 1, 0));
    } else if (e.key === "Enter" && results[index]) {
      e.preventDefault();
      choose(results[index].id);
    }
  };

  return (
    <Modal title={t("Palette de commandes…")} onClose={close}>
      <div className="form" onKeyDown={onKeyDown}>
        <input
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={results[index] ? `palette-${index}` : undefined}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t("Palette de commandes…")}
          autoFocus
        />
        <ul id="palette-list" role="listbox" className="palette-list" ref={listRef}>
          {results.map((r, i) => (
            <li key={r.id} id={`palette-${i}`} role="option" aria-selected={i === index} className={i === index ? "active" : ""} onMouseEnter={() => setIndex(i)} onClick={() => choose(r.id)}>
              <span>{r.label}</span>
              {r.hint && <kbd>{r.hint}</kbd>}
            </li>
          ))}
          {results.length === 0 && <li className="empty">—</li>}
        </ul>
      </div>
    </Modal>
  );
}

/** Proposé au démarrage quand des copies de récupération ont survécu à un plantage. */
export function RecoveryDialog() {
  const [files, setFiles] = useState<RecoveryDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.recoveryList().then(setFiles).catch((e) => setError(errorText(e)));
  }, []);
  useEffect(() => {
    if (files && files.length === 0) close();
  }, [files]);
  const restoreAll = async () => {
    for (const f of files ?? []) {
      if (!(await useStore.getState().run(() => api.recoveryRestore(f.file)))) return;
    }
    close();
  };
  const discardAll = async () => {
    let left = files ?? [];
    for (const f of files ?? []) left = await api.recoveryDiscard(f.file);
    setFiles(left);
  };
  return (
    <Modal title="Documents non enregistrés" onClose={close}>
      <div className="form">
        <p>OpenPics s'est arrêté avant que ces documents soient enregistrés. Voulez-vous les récupérer ?</p>
        {error && <p role="alert" className="error">{error}</p>}
        <ul>{(files ?? []).map((f) => <li key={f.file}>{f.name}</li>)}</ul>
        <div className="actions">
          <button className="btn secondary" onClick={() => void discardAll()}>Abandonner</button>
          <button className="btn" onClick={() => void restoreAll()}>Récupérer</button>
        </div>
      </div>
    </Modal>
  );
}

/** Applique un filtre à tous les fichiers d'un dossier et exporte le résultat ailleurs. */
export function BatchDialog() {
  const filters = useStore((s) => s.snapshot?.filters ?? []);
  const [input, setInput] = useState("");
  const [output, setOutput] = useState("");
  const [filter, setFilter] = useState(filters[0]?.id ?? "");
  const [ext, setExt] = useState("png");
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<BatchReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const def = filters.find((f) => f.id === filter);

  const pick = async (set: (p: string) => void) => {
    const p = await open({ directory: true, multiple: false });
    if (typeof p === "string") set(p);
  };
  const run = async () => {
    if (!def) return;
    setBusy(true);
    setError(null);
    setReport(null);
    try {
      setReport(await api.batchRun(input, output, def.id, def.params.map((p) => p.value), ext, 90));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal title="Traitement par lot" onClose={close} wide>
      <div className="form">
        <p className="hint">Applique un filtre (avec ses valeurs par défaut) à toutes les images du dossier, sans toucher aux originaux.</p>
        <div className="row">
          <button className="btn small secondary" onClick={() => void pick(setInput)}>Dossier source…</button>
          <span>{input || "—"}</span>
        </div>
        <div className="row">
          <button className="btn small secondary" onClick={() => void pick(setOutput)}>Dossier de sortie…</button>
          <span>{output || "—"}</span>
        </div>
        <label>
          Filtre
          <select value={filter} onChange={(e) => setFilter(e.target.value)}>
            {filters.map((f) => <option key={f.id} value={f.id}>{f.category} › {f.label}</option>)}
          </select>
        </label>
        <label>
          Format
          <select value={ext} onChange={(e) => setExt(e.target.value)}>
            <option value="png">PNG</option>
            <option value="jpg">JPEG</option>
            <option value="webp">WebP</option>
          </select>
        </label>
        {error && <p role="alert" className="error">{error}</p>}
        {report && (
          <div role="status">
            <p>{report.done} fichier(s) traité(s), {report.failed.length} en erreur.</p>
            {report.failed.length > 0 && <ul>{report.failed.map((f) => <li key={f}>{f}</li>)}</ul>}
          </div>
        )}
        <div className="actions">
          <button className="btn secondary" onClick={close}>Fermer</button>
          <button className="btn" disabled={busy || !input || !output || !def} onClick={() => void run()}>{busy ? "Traitement…" : "Lancer"}</button>
        </div>
      </div>
    </Modal>
  );
}
