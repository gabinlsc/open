import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { api, type ModelDto } from "../api";
import { errorText, useStore } from "../store";
import { Modal } from "./Modal";

const close = () => useStore.getState().setDialog(null);

/** Sélection du sujet / suppression de l'arrière-plan : méthode classique, sans modèle. */
export function AiSubjectDialog({ remove }: { remove: boolean }) {
  const [tolerance, setTolerance] = useState(15);
  const [models, setModels] = useState<ModelDto[]>([]);
  const [model, setModel] = useState("");
  useEffect(() => {
    api.aiModels().then((m) => setModels(m.filter((x) => x.installed && x.isolatesSubject))).catch(() => undefined);
  }, []);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const t = tolerance / 100;
    const id = model || null;
    if (await useStore.getState().run(() => (remove ? api.aiRemoveBackground(t, id) : api.aiSelectSubject(t, id)))) close();
  };
  return (
    <Modal title={remove ? "Supprimer l'arrière-plan" : "Sélectionner le sujet"} onClose={close}>
      <form onSubmit={submit} className="form">
        <p className="hint">
          Le fond est estimé depuis les bords de l'image : cela marche bien sur un fond uni ou peu texturé, pas sur une scène
          complexe.{remove ? " Un masque est ajouté au calque ; le retirer rend le fond." : ""}
        </p>
        <label>
          Méthode
          <select value={model} onChange={(e) => setModel(e.target.value)}>
            <option value="">Classique (fond estimé depuis les bords)</option>
            {models.map((m) => <option key={m.id} value={m.id}>Modèle : {m.name}</option>)}
          </select>
        </label>
        {model === "" && (
          <label>
            Tolérance du fond : {tolerance} %
            <input type="range" min={1} max={60} value={tolerance} onChange={(e) => setTolerance(Number(e.target.value))} />
          </label>
        )}
        <div className="actions">
          <button type="button" className="btn secondary" onClick={close}>Annuler</button>
          <button type="submit" className="btn">Appliquer</button>
        </div>
      </form>
    </Modal>
  );
}

const mb = (n: number) => `${(n / 1_048_576).toFixed(1)} Mo`;

/** Catalogue local des modèles : import depuis un fichier, vérifié par empreinte SHA-256. */
export function AiModelsDialog() {
  const [models, setModels] = useState<ModelDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.aiModels().then(setModels).catch((e) => setError(errorText(e)));
  }, []);
  const act = async (f: () => Promise<ModelDto[]>) => {
    try {
      setModels(await f());
      setError(null);
    } catch (e) {
      setError(errorText(e));
    }
  };
  const importModel = async (m: ModelDto) => {
    const picked = await open({ multiple: false, filters: [{ name: "Modèle ONNX", extensions: ["onnx"] }] });
    if (typeof picked === "string") await act(() => api.aiModelImport(m.id, picked));
  };
  return (
    <Modal title="Modèles d'IA" onClose={close} wide>
      <div className="form">
        <p className="hint">
          Aucun modèle n'est livré ni téléchargé par le logiciel. Les modèles se déclarent dans le fichier
          <code> models.json </code> du dossier de configuration (identifiant, tâche, licence, source, empreinte SHA-256), puis
          s'importent depuis un fichier : une empreinte différente est refusée. Les modèles de suppression d'arrière-plan / de sujet (type U²-Net, entrée image, sortie carte de probabilité) s'exécutent avec
          ONNX Runtime, à fournir séparément (<code>onnxruntime.dll</code> à côté de l'exécutable ou dans le dossier des modèles) ; sans lui, le menu IA
          utilise les méthodes classiques.
        </p>
        {error && <p role="alert" className="error">{error}</p>}
        {models === null ? (
          <p>Chargement…</p>
        ) : models.length === 0 ? (
          <p>Le catalogue est vide.</p>
        ) : (
          <ul className="model-list">
            {models.map((m) => (
              <li key={m.id}>
                <strong>{m.name}</strong> — {m.task} · {m.licence} · {mb(m.sizeBytes)}
                <br />
                <small>{m.source}</small>
                <div className="row">
                  {m.installed ? (
                    <button className="btn small secondary" onClick={() => void act(() => api.aiModelRemove(m.id))}>Supprimer</button>
                  ) : (
                    <button className="btn small" onClick={() => void importModel(m)}>Importer depuis un fichier…</button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
        <div className="actions"><button className="btn" onClick={close}>Fermer</button></div>
      </div>
    </Modal>
  );
}
