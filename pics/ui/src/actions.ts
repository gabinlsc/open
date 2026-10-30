import { open, save } from "@tauri-apps/plugin-dialog";
import { api, type ImageOp } from "./api";
import { activeDoc, errorText, useStore } from "./store";

export interface ActionDef {
  label: string;
  run: () => void | Promise<void>;
  enabled?: () => boolean;
}

const IMAGE_FILTERS = [
  { name: "Images et documents OpenPics", extensions: ["ocv", "png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"] },
  { name: "Tous les fichiers", extensions: ["*"] },
];

const st = () => useStore.getState();
const hasDoc = () => activeDoc(st()) !== null;
const hasSelection = () => st().snapshot?.hasSelection ?? false;

/** Ouvre des fichiers par chemin ; les échecs sont regroupés dans une seule notification. */
export async function openPaths(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  try {
    const result = await api.docOpen(paths);
    st().apply(result.snapshot);
    if (result.failures.length > 0) st().notify({ kind: "error", text: result.failures.join("\n") });
  } catch (e) {
    st().notify({ kind: "error", text: errorText(e) });
  }
}

async function saveAs(): Promise<void> {
  const doc = activeDoc(st());
  if (!doc) return;
  const path = await save({ defaultPath: `${doc.name}.ocv`, filters: [{ name: "Document OpenPics", extensions: ["ocv"] }] });
  if (path) await st().run(() => api.docSave(path));
}

async function saveCurrent(): Promise<void> {
  const doc = activeDoc(st());
  if (!doc) return;
  // Sans chemin connu, le cœur répond `needs_path` : on bascule alors sur « Enregistrer sous ».
  try {
    st().apply(await api.docSave(null));
  } catch (e) {
    if (e instanceof Error && "code" in e && (e as { code: string }).code === "needs_path") await saveAs();
    else st().notify({ kind: "error", text: errorText(e) });
  }
}

/** Démarre l'aperçu d'un filtre avec ses valeurs par défaut. */
export async function startFilter(id: string): Promise<void> {
  const info = st().snapshot?.filters.find((f) => f.id === id);
  if (!info) return;
  if (st().filter) await cancelFilter();
  const values = info.params.map((p) => p.value);
  st().setFilter({ id, values });
  if (!(await st().run(() => api.filterPreview(id, values)))) st().setFilter(null);
}

export async function cancelFilter(): Promise<void> {
  st().setFilter(null);
  await st().run(api.filterCancel);
}

export async function commitFilter(): Promise<void> {
  st().setFilter(null);
  await st().run(api.filterCommit);
}

export function requestClose(id: number): void {
  const doc = st().snapshot?.docs.find((d) => d.id === id);
  if (!doc) return;
  if (doc.dirty) st().setDialog({ kind: "confirmClose", id, name: doc.name });
  else void st().run(() => api.docClose(id));
}

function cycleTab(step: number): void {
  const snap = st().snapshot;
  if (!snap || snap.docs.length < 2) return;
  const idx = snap.docs.findIndex((d) => d.id === snap.activeId);
  const next = snap.docs[(idx + step + snap.docs.length) % snap.docs.length];
  if (next) void st().run(() => api.docActivate(next.id));
}

function activeLayer() {
  const snap = st().snapshot;
  return snap?.layers.find((l) => l.id === snap.activeLayerId) ?? null;
}
const canAddMask = () => activeLayer() !== null && !activeLayer()?.hasMask;
function maskAdd(reveal: boolean): void {
  const layer = activeLayer();
  if (layer) void st().run(() => api.maskAdd(layer.id, reveal));
}

const imageOp = (op: ImageOp) => () => void st().run(() => api.op(op));
const toggle = (key: "showRulers" | "showGrid" | "showGuides" | "snapEnabled") => () => {
  const s = st().snapshot?.settings;
  if (s) void st().setSetting(key, !s[key]);
};

/** Identifiants alignés sur `DEFAULT_BINDINGS` du cœur ; les actions sans raccourci n'y figurent pas. */
export const ACTIONS: Record<string, ActionDef> = {
  "file.new": { label: "Nouveau…", run: () => st().setDialog({ kind: "new" }) },
  "file.open": {
    label: "Ouvrir…",
    run: async () => {
      const picked = await open({ multiple: true, filters: IMAGE_FILTERS });
      if (picked) await openPaths(Array.isArray(picked) ? picked : [picked]);
    },
  },
  "file.save": { label: "Enregistrer", run: saveCurrent, enabled: hasDoc },
  "file.save_as": { label: "Enregistrer sous…", run: saveAs, enabled: hasDoc },
  "file.export": { label: "Exporter en image…", run: () => st().setDialog({ kind: "export" }), enabled: hasDoc },
  "file.close": {
    label: "Fermer le document",
    run: () => {
      const doc = activeDoc(st());
      if (doc) requestClose(doc.id);
    },
    enabled: hasDoc,
  },
  "edit.undo": { label: "Annuler", run: () => void st().run(api.undo), enabled: () => st().snapshot?.canUndo ?? false },
  "edit.redo": { label: "Rétablir", run: () => void st().run(api.redo), enabled: () => st().snapshot?.canRedo ?? false },
  "ai.subject": { label: "Sélectionner le sujet…", run: () => st().setDialog({ kind: "aiSubject", remove: false }), enabled: hasDoc },
  "ai.remove_background": { label: "Supprimer l'arrière-plan…", run: () => st().setDialog({ kind: "aiSubject", remove: true }), enabled: hasDoc },
  "ai.inpaint": { label: "Combler la sélection", run: () => void st().run(api.aiInpaint), enabled: hasDoc },
  "ai.models": { label: "Modèles d'IA…", run: () => st().setDialog({ kind: "aiModels" }) },
  "app.palette": { label: "Palette de commandes…", run: () => st().setDialog({ kind: "palette" }) },
  "file.batch": { label: "Traitement par lot…", run: () => st().setDialog({ kind: "batch" }) },
  "edit.shortcuts": { label: "Raccourcis clavier…", run: () => st().setDialog({ kind: "shortcuts" }) },
  "image.flip_h": { label: "Retourner horizontalement", run: imageOp("flipHorizontal"), enabled: hasDoc },
  "image.flip_v": { label: "Retourner verticalement", run: imageOp("flipVertical"), enabled: hasDoc },
  "image.invert": { label: "Inverser les couleurs", run: imageOp("invert"), enabled: hasDoc },
  "view.zoom_in": { label: "Zoom avant", run: () => void st().run(() => api.zoom(true)), enabled: hasDoc },
  "view.zoom_out": { label: "Zoom arrière", run: () => void st().run(() => api.zoom(false)), enabled: hasDoc },
  "view.fit": { label: "Ajuster à l'écran", run: () => void st().run(api.fit), enabled: hasDoc },
  "view.actual_size": { label: "Taille réelle (100 %)", run: () => void st().run(api.actualSize), enabled: hasDoc },
  "view.toggle_rulers": { label: "Règles", run: toggle("showRulers") },
  "view.toggle_guides": { label: "Repères", run: toggle("showGuides") },
  "view.toggle_grid": { label: "Grille", run: toggle("showGrid") },
  "view.toggle_snap": { label: "Aimantation", run: toggle("snapEnabled") },
  "view.clear_guides": {
    label: "Effacer les repères",
    run: () => void st().run(api.guidesClear),
    enabled: () => (st().snapshot?.guides.length ?? 0) > 0,
  },
  "layer.new": { label: "Nouveau calque", run: () => void st().run(api.layerAdd), enabled: hasDoc },
  "layer.delete": {
    label: "Supprimer le calque",
    run: () => {
      const id = st().snapshot?.activeLayerId;
      if (id != null) void st().run(() => api.layerRemove(id));
    },
    enabled: () => (st().snapshot?.layers.length ?? 0) > 1,
  },
  "layer.transform": { label: "Transformer…", run: () => st().setDialog({ kind: "transform" }), enabled: () => activeLayer() !== null },
  "layer.mask_reveal": { label: "Ajouter un masque (tout révéler)", run: () => maskAdd(true), enabled: canAddMask },
  "layer.mask_hide": { label: "Ajouter un masque (tout masquer)", run: () => maskAdd(false), enabled: canAddMask },
  "tab.next": { label: "Onglet suivant", run: () => cycleTab(1) },
  "tab.prev": { label: "Onglet précédent", run: () => cycleTab(-1) },
  "select.all": { label: "Tout sélectionner", run: () => void st().run(api.selectAll), enabled: hasDoc },
  "select.none": { label: "Désélectionner", run: () => void st().run(api.selectNone), enabled: () => st().snapshot?.hasSelection ?? false },
  "select.invert": { label: "Inverser la sélection", run: () => void st().run(api.selectInvert), enabled: hasDoc },
  "select.feather": { label: "Contour progressif…", run: () => st().setDialog({ kind: "selectOp", op: "feather" }), enabled: hasSelection },
  "select.grow": { label: "Agrandir…", run: () => st().setDialog({ kind: "selectOp", op: "grow" }), enabled: hasSelection },
  "select.shrink": { label: "Contracter…", run: () => st().setDialog({ kind: "selectOp", op: "shrink" }), enabled: hasSelection },
  "tool.rect_select": { label: "Sélection rectangulaire", run: () => void st().run(() => api.setTool("rect_select")) },
  "tool.ellipse_select": { label: "Sélection elliptique", run: () => void st().run(() => api.setTool("ellipse_select")) },
  "tool.lasso": { label: "Lasso", run: () => void st().run(() => api.setTool("lasso")) },
  "tool.polygon_lasso": { label: "Lasso polygonal", run: () => void st().run(() => api.setTool("polygon_lasso")) },
  "tool.wand": { label: "Baguette magique", run: () => void st().run(() => api.setTool("magic_wand")) },
  "tool.finish": { label: "Valider la forme en cours", run: () => void st().run(api.toolFinish) },
  "tool.cancel": {
    label: "Annuler la forme en cours",
    run: () => {
      // Échap ferme d'abord un filtre en aperçu, sinon abandonne le geste d'outil.
      if (st().filter) void cancelFilter();
      else void st().run(api.toolCancel);
    },
  },
  "tool.brush": { label: "Pinceau", run: () => void st().run(() => api.setTool("brush")) },
  "tool.pencil": { label: "Crayon", run: () => void st().run(() => api.setTool("pencil")) },
  "tool.eraser": { label: "Gomme", run: () => void st().run(() => api.setTool("eraser")) },
  "tool.gradient": { label: "Dégradé", run: () => void st().run(() => api.setTool("gradient")) },
  "tool.bucket": { label: "Pot de peinture", run: () => void st().run(() => api.setTool("bucket")) },
  "tool.clone": { label: "Tampon de duplication", run: () => void st().run(() => api.setTool("clone_stamp")) },
  "tool.heal": { label: "Correcteur", run: () => void st().run(() => api.setTool("heal")) },
  "tool.spot_heal": { label: "Correcteur localisé", run: () => void st().run(() => api.setTool("spot_heal")) },
  "tool.patch": { label: "Pièce", run: () => void st().run(() => api.setTool("patch")) },
  "tool.blur": { label: "Goutte d'eau (flou)", run: () => void st().run(() => api.setTool("blur")) },
  "tool.sharpen": { label: "Netteté", run: () => void st().run(() => api.setTool("sharpen")) },
  "tool.smudge": { label: "Doigt", run: () => void st().run(() => api.setTool("smudge")) },
  "tool.dodge": { label: "Densité −", run: () => void st().run(() => api.setTool("dodge")) },
  "tool.burn": { label: "Densité +", run: () => void st().run(() => api.setTool("burn")) },
  "tool.sponge": { label: "Éponge", run: () => void st().run(() => api.setTool("sponge")) },
  "paint.swap_colors": {
    label: "Permuter les couleurs",
    run: () => {
      const o = st().snapshot?.paintOptions;
      if (o) void st().run(() => api.setPaintOptions({ ...o, foreground: o.background, background: o.foreground }));
    },
  },
  "tool.liquify": { label: "Fluidité", run: () => void st().run(() => api.setTool("liquify")) },
  "tool.text": { label: "Texte", run: () => void st().run(() => api.setTool("text")) },
  "tool.shape_rect": { label: "Rectangle", run: () => void st().run(() => api.setTool("shape_rect")) },
  "tool.shape_rounded": { label: "Rectangle arrondi", run: () => void st().run(() => api.setTool("shape_rounded_rect")) },
  "tool.shape_ellipse": { label: "Ellipse", run: () => void st().run(() => api.setTool("shape_ellipse")) },
  "tool.shape_line": { label: "Ligne", run: () => void st().run(() => api.setTool("shape_line")) },
  "tool.pen": { label: "Plume", run: () => void st().run(() => api.setTool("pen")) },
  "layer.rasterize": {
    label: "Rastériser le calque",
    run: () => {
      const l = activeLayer();
      if (l) void st().run(() => api.layerRasterize(l.id));
    },
    enabled: () => {
      const l = activeLayer();
      return !!l && (!!l.text || !!l.shape);
    },
  },
  "file.import_svg": {
    label: "Importer un SVG…",
    run: async () => {
      const picked = await open({ multiple: false, filters: [{ name: "SVG", extensions: ["svg"] }] });
      if (typeof picked === "string") await st().run(() => api.svgImport(picked));
    },
    enabled: hasDoc,
  },
  "tool.hand": { label: "Outil Main", run: () => void st().run(() => api.setTool("hand")) },
  "tool.zoom": { label: "Outil Zoom", run: () => void st().run(() => api.setTool("zoom")) },
};

export function runAction(id: string): void {
  const action = ACTIONS[id];
  if (!action || action.enabled?.() === false) return;
  void Promise.resolve(action.run()).catch((e) => st().notify({ kind: "error", text: errorText(e) }));
}
