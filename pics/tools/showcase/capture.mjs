// Usage : node capture.mjs <dossier des scènes> <dossier des captures> [largeur hauteur du canvas]
import { chromium, openScene, paintCanvas, startVite, store } from "./harness.mjs";
import path from "node:path";
import { mkdirSync } from "node:fs";

const [scenes, out] = process.argv.slice(2);
mkdirSync(out, { recursive: true });

// Chaque capture : scène, état d'interface éventuel, nom de fichier.
const SHOTS = [
  { file: "editeur", scene: "01-editor" },
  { file: "editeur-sombre", scene: "01-editor-dark" },
  { file: "selection-sujet", scene: "02-subject" },
  { file: "arriere-plan", scene: "03-background" },
  { file: "vectoriel-texte", scene: "04-vector" },
  {
    file: "filtres",
    scene: "05-filters",
    ui: (page) => store(page, (m) => m.useStore.getState().setFilter({ id: "crystallize", values: [22, 3] })),
  },
  {
    file: "palette-commandes",
    scene: "01-editor",
    ui: async (page) => {
      await store(page, (m) => m.useStore.getState().setDialog({ kind: "palette" }));
      await page.keyboard.type("sel");
    },
  },
  { file: "scripts", scene: "01-editor", ui: (page) => store(page, (m) => m.useStore.getState().setDialog({ kind: "scripts" })) },
  { file: "ia-sujet", scene: "01-editor", ui: (page) => store(page, (m) => m.useStore.getState().setDialog({ kind: "aiSubject", remove: true })) },
  {
    file: "menu-filtre",
    scene: "01-editor",
    ui: async (page) => {
      await page.getByRole("menuitem", { name: "Filtre", exact: true }).click();
    },
  },
];

const stop = await startVite();
const browser = await chromium.launch();
for (const shot of SHOTS) {
  const { page, host, context } = await openScene(browser, scenes, shot.scene);
  await paintCanvas(page, host, path.join(scenes, `${shot.scene}.png`));
  if (shot.ui) {
    await shot.ui(page);
    await page.waitForTimeout(500);
  }
  await page.screenshot({ path: path.join(out, `${shot.file}.png`) });
  console.log("capture", shot.file);
  await context.close();
}
await browser.close();
stop();
