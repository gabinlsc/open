// Usage : node render.mjs <dossier de sortie des images> [premier] [dernier] [pas]
// Rend chaque image de la chronologie (capture d'écran pilotée par seek(frame)), pour assemblage par ffmpeg.
import { createRequire } from "node:module";
import path from "node:path";
import { mkdirSync } from "node:fs";
const { chromium } = createRequire(import.meta.url)("playwright");
const [out, first = "0", last = "1439", step = "1"] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
await page.goto("file://" + path.resolve(import.meta.dirname, "index.html"));
await page.evaluate(() => window.__ready);
for (let f = Number(first); f <= Number(last); f += Number(step)) {
  await page.evaluate((f) => window.seek(f), f);
  await page.screenshot({ path: path.join(out, `f${String(f).padStart(5, "0")}.png`) });
}
await browser.close();
