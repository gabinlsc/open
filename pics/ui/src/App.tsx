import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef } from "react";
import { api, events, type CursorEvent, type Snapshot } from "./api";
import { openPaths, runAction } from "./actions";
import { Dialogs } from "./components/Dialogs";
import { CanvasArea } from "./components/CanvasArea";
import { MenuBar } from "./components/MenuBar";
import { Sidebar } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { Tabs } from "./components/Tabs";
import { Toolbar } from "./components/Toolbar";
import { buildLookup, chordFromEvent } from "./keys";
import { useStore } from "./store";
import { applyTheme } from "./theme";

const isTyping = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

export function App() {
  const ready = useStore((s) => s.ready);
  const themeChoice = useStore((s) => s.themeChoice);
  const gpuError = useStore((s) => s.gpuError);
  const notice = useStore((s) => s.notice);
  const keymap = useStore((s) => s.keymap);
  const lookup = useRef(new Map<string, string>());
  lookup.current = buildLookup(keymap);

  useEffect(() => {
    void useStore.getState().init();
  }, []);

  // Thème : appliqué à l'interface et transmis au cœur (fond du canvas natif).
  useEffect(() => {
    if (!ready) return;
    const push = () => void api.setTheme(themeChoice, applyTheme(themeChoice)).catch(() => undefined);
    push();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", push);
    return () => mq.removeEventListener("change", push);
  }, [ready, themeChoice]);

  // Événements du cœur. Les mises à jour de vue peuvent arriver plus vite que l'affichage
  // (molette, glisser) : on ne garde que la dernière par image.
  useEffect(() => {
    let frame = 0;
    let pendingState: Snapshot | null = null;
    let pendingCursor: CursorEvent | null = null;
    const flush = () => {
      frame = 0;
      if (pendingState) useStore.getState().apply(pendingState);
      if (pendingCursor) useStore.getState().setCursor(pendingCursor);
      pendingState = pendingCursor = null;
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(flush);
    };
    const subs = [
      events.onState((s) => ((pendingState = s), schedule())),
      events.onCursor((c) => ((pendingCursor = c), schedule())),
      events.onGpuError((m) => useStore.getState().setGpuError(m)),
      events.onOpenFailed((m) => useStore.getState().notify({ kind: "error", text: m.join("\n") })),
    ];
    return () => {
      if (frame) cancelAnimationFrame(frame);
      subs.forEach((p) => void p.then((off) => off()));
    };
  }, []);

  // Glisser-déposer de fichiers sur l'interface (le canvas natif gère le sien).
  useEffect(() => {
    const sub = getCurrentWebview().onDragDropEvent((e) => {
      if (e.payload.type === "drop") void openPaths(e.payload.paths);
    });
    return () => void sub.then((off) => off());
  }, []);

  // Raccourcis clavier + barre d'espace (main temporaire).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (isTyping(e.target) || useStore.getState().dialog) return;
      if (e.code === "Space" && !e.repeat) void api.setPanModifier(true).catch(() => undefined);
      const chord = chordFromEvent(e);
      const action = chord ? lookup.current.get(chord) : undefined;
      if (action) {
        e.preventDefault();
        runAction(action);
      } else if (e.code === "Space") {
        e.preventDefault();
      }
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") void api.setPanModifier(false).catch(() => undefined);
    };
    const onBlur = () => void api.setPanModifier(false).catch(() => undefined);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  useEffect(() => {
    if (notice?.kind !== "info") return;
    const t = setTimeout(() => useStore.getState().notify(null), 4000);
    return () => clearTimeout(t);
  }, [notice]);

  if (!ready) return null;
  return (
    <div className="app">
      <header className="appbar">
        <img className="brand" src="/favicon.svg" alt="" width={24} height={24} />
        <MenuBar />
      </header>
      {gpuError && (
        <div className="banner" role="alert">
          Le rendu n'a pas pu démarrer : {gpuError}
        </div>
      )}
      <Tabs />
      <main className="main">
        <Toolbar />
        <CanvasArea />
        <Sidebar />
      </main>
      <StatusBar />
      <Dialogs />
      {notice && (
        <div className={`toast ${notice.kind}`} role={notice.kind === "error" ? "alert" : "status"}>
          <span className="toast-text">{notice.text}</span>
          <button className="btn small ghost" aria-label="Fermer la notification" onClick={() => useStore.getState().notify(null)}>×</button>
        </div>
      )}
    </div>
  );
}
