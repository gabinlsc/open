import { api, type Tool } from "../api";
import { prettyChord } from "../keys";
import { useStore } from "../store";
import { HandIcon, ZoomIcon } from "./Icons";

const TOOLS: { id: Tool; label: string; action: string; icon: JSX.Element }[] = [
  { id: "hand", label: "Main", action: "tool.hand", icon: <HandIcon /> },
  { id: "zoom", label: "Zoom", action: "tool.zoom", icon: <ZoomIcon /> },
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
