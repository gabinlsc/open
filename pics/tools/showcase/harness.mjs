// Fait tourner la vraie interface (serveur Vite) avec le cœur simulé par des scènes générées par le vrai moteur
// (`showcase`) : états d'interface exacts, canvas rendu par le vrai moteur de rendu, posé sur la zone réservée.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import net from "node:net";
import path from "node:path";

const require = createRequire(import.meta.url);
export const { chromium } = require("playwright");

export const UI_DIR = path.resolve(import.meta.dirname, "../../ui");

function waitPort(port, ms = 30000) {
  const end = Date.now() + ms;
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const s = net.connect(port, "127.0.0.1");
      s.on("connect", () => (s.destroy(), resolve()));
      s.on("error", () => (Date.now() > end ? reject(new Error("vite ne démarre pas")) : setTimeout(tryOnce, 250)));
    };
    tryOnce();
  });
}

export async function startVite() {
  const proc = spawn("npx", ["vite", "--port", "5173", "--strictPort"], { cwd: UI_DIR, stdio: "ignore" });
  await waitPort(5173);
  return () => proc.kill();
}

const SHIM = () => {
  const scene = window.__SCENE;
  const state = { scene };
  window.__state = state;
  const VOID = new Set(["view_set_canvas_visible", "theme_set", "guide_cancel", "record_start"]);
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    transformCallback: () => 1,
    unregisterCallback: () => {},
    convertFileSrc: (p) => p,
    invoke: async (cmd, args) => {
      const sc = window.__state.scene;
      if (cmd.startsWith("plugin:event|")) return 1;
      if (cmd === "app_status") return sc.status;
      if (cmd === "keymap_get") return sc.keymap;
      if (cmd === "recovery_list" || cmd === "ai_models" || cmd === "actions_list") return [];
      if (cmd === "fonts_list") return ["Inter", "DejaVu Sans"];
      if (VOID.has(cmd)) return null;
      return sc.snapshot; // toute commande qui répond par un état renvoie l'état de la scène
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
};

/** Ouvre une page sur la scène `dir/name` ; renvoie { page, host } où host = rectangle du canvas. */
export async function openScene(browser, dir, name, { width = 1920, height = 1080 } = {}) {
  const scene = JSON.parse(readFileSync(path.join(dir, `${name}.json`), "utf8"));
  const context = await browser.newContext({ viewport: { width, height }, deviceScaleFactor: 1, colorScheme: scene.status.theme, locale: "fr-FR" });
  const page = await context.newPage();
  await page.addInitScript(`window.__SCENE = ${JSON.stringify(scene)};`);
  await page.addInitScript(SHIM);
  await page.goto("http://localhost:5173/");
  await page.waitForSelector(".canvas-host", { timeout: 15000 });
  await page.waitForTimeout(300);
  const host = await page.evaluate(() => {
    const r = document.querySelector(".canvas-host").getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  });
  return { page, host, context };
}

/** Pose le rendu du canvas (PNG du vrai moteur) à la place du canvas natif. */
export async function paintCanvas(page, host, pngPath) {
  const b64 = readFileSync(pngPath).toString("base64");
  await page.evaluate(({ host, b64 }) => {
    document.getElementById("__canvas")?.remove();
    const img = document.createElement("img");
    img.id = "__canvas";
    img.src = `data:image/png;base64,${b64}`;
    Object.assign(img.style, { position: "fixed", left: `${host.x}px`, top: `${host.y}px`, width: `${host.width}px`, height: `${host.height}px`, zIndex: 1, pointerEvents: "none" });
    document.body.appendChild(img);
  }, { host, b64 });
  await page.waitForFunction(() => document.getElementById("__canvas")?.complete);
}

/** Accès au magasin d'état de l'interface (module chargé par Vite). */
export const store = (page, fn, arg) =>
  page.evaluate(async ([src, arg]) => {
    const m = await import("/src/store.ts");
    return new Function("m", "arg", `return (${src})(m, arg)`)(m, arg);
  }, [fn.toString(), arg]);
