import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../api";
import { ACTIONS } from "../actions";
import { physicalRect, setCanvasHost } from "../canvasHost";
import { decodeFrame } from "../frozenFrame";
import { useStore } from "../store";
import { RULER_SIZE, Rulers } from "./Rulers";

/**
 * Réserve la place du canvas natif. Le canvas n'est PAS dans le DOM : une fenêtre enfant
 * Win32 est posée par-dessus ce <div>, d'où la contrainte « aucun élément HTML ne recouvre le canvas ».
 */
export function CanvasArea() {
  const hasDoc = useStore((s) => (s.snapshot?.docs.length ?? 0) > 0);
  // Fenêtre native toujours au-dessus du HTML : on la cache pendant un menu ou un dialogue.
  const overlay = useStore((s) => s.dialog !== null || s.menuOpen);
  const showRulers = useStore((s) => s.snapshot?.settings.showRulers ?? true);
  const hostRef = useRef<HTMLDivElement>(null);
  const frozenRef = useRef<HTMLCanvasElement>(null);
  const [frozen, setFrozen] = useState(false);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const last = useRef("");

  const measure = useCallback(() => {
    const el = hostRef.current;
    if (el) setSize({ width: el.clientWidth, height: el.clientHeight });
    const rect = hasDoc ? physicalRect() : { x: 0, y: 0, width: 0, height: 0 };
    if (!rect) return;
    const key = JSON.stringify(rect);
    if (key === last.current) return;
    last.current = key;
    void api.setCanvas(rect).then((s) => useStore.getState().apply(s)).catch(() => undefined);
  }, [hasDoc]);

  useLayoutEffect(() => {
    setCanvasHost(hasDoc ? hostRef.current : null);
    measure();
    return () => setCanvasHost(null);
  }, [hasDoc, showRulers, measure]);

  // La fenêtre native passe toujours au-dessus du HTML : pendant un menu ou une boîte de dialogue, on la cache
  // et on affiche à sa place la dernière image rendue (image figée), sinon le document disparaîtrait.
  useEffect(() => {
    if (!hasDoc) return;
    let cancelled = false;
    if (overlay) {
      void (async () => {
        try {
          const frame = decodeFrame(await api.canvasSnapshot());
          const el = frozenRef.current;
          const ctx = el?.getContext("2d");
          if (!cancelled && frame && el && ctx) {
            el.width = frame.width;
            el.height = frame.height;
            ctx.putImageData(new ImageData(frame.data, frame.width, frame.height), 0, 0);
            setFrozen(true);
          }
        } catch {
          // Sans image figée, la zone reste vide le temps du menu : comportement d'avant, rien de cassé.
        }
        if (!cancelled) await api.setCanvasVisible(false).catch(() => undefined);
      })();
    } else {
      void api
        .setCanvasVisible(true)
        .catch(() => undefined)
        // La fenêtre native réapparaît d'abord, l'image figée est retirée ensuite : pas de clignotement.
        .then(() => setTimeout(() => !cancelled && setFrozen(false), 60));
    }
    return () => {
      cancelled = true;
    };
  }, [overlay, hasDoc]);

  useEffect(() => {
    const el = hostRef.current;
    if (!el) return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    // Un changement de densité d'écran (déplacement vers un autre moniteur) change la taille physique sans changer la taille CSS.
    const mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    mq.addEventListener("change", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
      mq.removeEventListener("change", measure);
    };
  }, [measure]);

  if (!hasDoc) {
    return (
      <div className="welcome">
        <h1>Bienvenue dans OpenPics</h1>
        <p>Créez un document ou ouvrez une image. Vous pouvez aussi glisser des fichiers dans cette fenêtre.</p>
        <div className="row">
          <button className="btn" onClick={() => void ACTIONS["file.new"]?.run()}>Nouveau document</button>
          <button className="btn secondary" onClick={() => void ACTIONS["file.open"]?.run()}>Ouvrir une image…</button>
        </div>
      </div>
    );
  }

  const r = showRulers ? RULER_SIZE : 0;
  return (
    <div className="center" style={{ gridTemplateColumns: `${r}px 1fr`, gridTemplateRows: `${r}px 1fr` }}>
      {showRulers && <Rulers width={size.width} height={size.height} />}
      <div ref={hostRef} className="canvas-host" style={{ gridColumn: 2, gridRow: 2 }} aria-label="Zone de dessin">
        <canvas ref={frozenRef} className="canvas-frozen" hidden={!frozen} aria-hidden="true" />
      </div>
    </div>
  );
}
