/** Traduction minimale FR → EN des libellés de l'interface (menus, actions, outils, boîtes de l'IA/récupération/lot). */
export type Lang = "fr" | "en";

const EN: Record<string, string> = {
  // menus
  "Fichier": "File", "Édition": "Edit", "Calque": "Layer", "Sélection": "Select", "IA": "AI", "Filtre": "Filter",
  "Image": "Image", "Affichage": "View", "Thème : système": "Theme: system", "Thème : clair": "Theme: light", "Thème : sombre": "Theme: dark",
  "Langue : français": "Language: French", "Langue : anglais": "Language: English",
  // fichier / édition
  "Nouveau…": "New…", "Ouvrir…": "Open…", "Importer un SVG…": "Import SVG…", "Enregistrer": "Save", "Enregistrer sous…": "Save as…",
  "Exporter en image…": "Export as image…", "Fermer le document": "Close document", "Traitement par lot…": "Batch process…",
  "Annuler": "Undo", "Rétablir": "Redo", "Raccourcis clavier…": "Keyboard shortcuts…", "Palette de commandes…": "Command palette…",
  // calque
  "Nouveau calque": "New layer", "Supprimer le calque": "Delete layer", "Rastériser le calque": "Rasterize layer",
  "Ajouter un masque (tout révéler)": "Add mask (reveal all)", "Ajouter un masque (tout masquer)": "Add mask (hide all)",
  // sélection
  "Tout sélectionner": "Select all", "Désélectionner": "Deselect", "Inverser la sélection": "Invert selection",
  "Contour progressif…": "Feather…", "Agrandir…": "Grow…", "Contracter…": "Shrink…",
  "Sélectionner le sujet…": "Select subject…", "Supprimer l'arrière-plan…": "Remove background…",
  "Combler la sélection": "Fill selection", "Modèles d'IA…": "AI models…",
  // image
  "Retourner horizontalement": "Flip horizontally", "Retourner verticalement": "Flip vertically", "Inverser les couleurs": "Invert colors",
  "Transformer…": "Transform…",
  // affichage
  "Zoom avant": "Zoom in", "Zoom arrière": "Zoom out", "Ajuster à l'écran": "Fit on screen", "Taille réelle (100 %)": "Actual size (100%)",
  "Règles": "Rulers", "Grille": "Grid", "Repères": "Guides", "Aimantation": "Snapping", "Effacer les repères": "Clear guides",
  "Onglet suivant": "Next tab", "Onglet précédent": "Previous tab", "Permuter les couleurs": "Swap colors",
  // outils
  "Outil Main": "Hand tool", "Outil Zoom": "Zoom tool", "Sélection rectangulaire": "Rectangular marquee", "Sélection elliptique": "Elliptical marquee",
  "Lasso": "Lasso", "Lasso polygonal": "Polygonal lasso", "Baguette magique": "Magic wand", "Pinceau": "Brush", "Crayon": "Pencil",
  "Gomme": "Eraser", "Dégradé": "Gradient", "Pot de peinture": "Paint bucket", "Tampon de duplication": "Clone stamp",
  "Correcteur": "Healing brush", "Correcteur localisé": "Spot healing", "Pièce": "Patch", "Goutte d'eau (flou)": "Blur",
  "Netteté": "Sharpen", "Doigt": "Smudge", "Densité −": "Dodge", "Densité +": "Burn", "Éponge": "Sponge", "Fluidité": "Liquify",
  "Texte": "Text", "Plume": "Pen", "Rectangle": "Rectangle", "Rectangle arrondi": "Rounded rectangle", "Ellipse": "Ellipse", "Ligne": "Line",
  "Valider la forme en cours": "Commit current shape", "Annuler la forme en cours": "Cancel current shape",
};

let current: Lang = "fr";

export function detectLang(stored: string | null, browser: string): Lang {
  if (stored === "fr" || stored === "en") return stored;
  return browser.toLowerCase().startsWith("fr") ? "fr" : "en";
}

export function initLang(): Lang {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem("openpics.lang");
  } catch {
    /* stockage indisponible : on retombe sur la langue du navigateur */
  }
  current = detectLang(stored, typeof navigator === "undefined" ? "fr" : navigator.language);
  return current;
}

export function setLang(lang: Lang): void {
  current = lang;
  try {
    localStorage.setItem("openpics.lang", lang);
  } catch {
    /* préférence non mémorisée */
  }
}

/** Libellé dans la langue courante ; sans traduction, le français d'origine (jamais une clé vide). */
export function t(fr: string): string {
  return current === "en" ? (EN[fr] ?? fr) : fr;
}

export const hasEnglish = (fr: string): boolean => fr in EN;
