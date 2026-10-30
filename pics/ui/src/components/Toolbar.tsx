import { api, type Tool } from "../api";
import { prettyChord } from "../keys";
import { useStore } from "../store";
import { EllipseSelectIcon, HandIcon, LassoIcon, PolygonLassoIcon, RectSelectIcon, WandIcon, ZoomIcon } from "./Icons";

const TOOLS: { id: Tool; label: string; action: string; icon: JSX.Element }[] = [
  { id: "hand", label: "Main", action: "tool.hand", icon: <HandIcon /> },
  { id: "zoom", label: "Zoom", action: "tool.zoom", icon: <ZoomIcon /> },
  { id: "rect_select", label: "Sélection rectangulaire", action: "tool.rect_select", icon: <RectSelectIcon /> },
  { id: "ellipse_select", label: "Sélection elliptique", action: "tool.ellipse_select", icon: <EllipseSelectIcon /> },
  { id: "lasso", label: "Lasso", action: "tool.lasso", icon: <LassoIcon /> },
  { id: "polygon_lasso", label: "Lasso polygonal", action: "tool.polygon_lasso", icon: <PolygonLassoIcon /> },
  { id: "magic_wand", label: "Baguette magique", action: "tool.wand", icon: <WandIcon /> },
];

export function Toolbar() {
  const tool = useStore((s) => s.snapshot?.tool);
  const keymap = useStore((s) => s.keymap);
  const run = useStore((s) => s.run);
  return (
    <div className="toolbar" role="toolbar" aria-label="Outils" aria-orientation="vertical">
      {TOOLS.map((t) => {
        const chord = keymap[t.action]?.[0];
        return (
          <button
            key={t.id}
            className={tool === t.id ? "tool active" : "tool"}
            aria-pressed={tool === t.id}
            aria-label={t.label}
            title={chord ? `${t.label} (${prettyChord(chord)})` : t.label}
            onClick={() => void run(() => api.setTool(t.id))}
          >
            {t.icon}
          </button>
        );
      })}
    </div>
  );
}
