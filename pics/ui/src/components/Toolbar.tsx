import { api, type Tool } from "../api";
import { prettyChord } from "../keys";
import { useStore } from "../store";
import { Glyph, EllipseSelectIcon, HandIcon, LassoIcon, PolygonLassoIcon, RectSelectIcon, WandIcon, ZoomIcon } from "./Icons";

const TOOLS: { id: Tool; label: string; action: string; icon: JSX.Element }[] = [
  { id: "hand", label: "Main", action: "tool.hand", icon: <HandIcon /> },
  { id: "zoom", label: "Zoom", action: "tool.zoom", icon: <ZoomIcon /> },
  { id: "rect_select", label: "Sélection rectangulaire", action: "tool.rect_select", icon: <RectSelectIcon /> },
  { id: "ellipse_select", label: "Sélection elliptique", action: "tool.ellipse_select", icon: <EllipseSelectIcon /> },
  { id: "lasso", label: "Lasso", action: "tool.lasso", icon: <LassoIcon /> },
  { id: "polygon_lasso", label: "Lasso polygonal", action: "tool.polygon_lasso", icon: <PolygonLassoIcon /> },
  { id: "magic_wand", label: "Baguette magique", action: "tool.wand", icon: <WandIcon /> },
  { id: "brush", label: "Pinceau", action: "tool.brush", icon: <Glyph>✎</Glyph> },
  { id: "pencil", label: "Crayon", action: "tool.pencil", icon: <Glyph>✏</Glyph> },
  { id: "eraser", label: "Gomme", action: "tool.eraser", icon: <Glyph>⌫</Glyph> },
  { id: "bucket", label: "Pot de peinture", action: "tool.bucket", icon: <Glyph>⬣</Glyph> },
  { id: "gradient", label: "Dégradé", action: "tool.gradient", icon: <Glyph>◧</Glyph> },
  { id: "clone_stamp", label: "Tampon de duplication", action: "tool.clone", icon: <Glyph>◎</Glyph> },
  { id: "heal", label: "Correcteur", action: "tool.heal", icon: <Glyph>✚</Glyph> },
  { id: "spot_heal", label: "Correcteur localisé", action: "tool.spot_heal", icon: <Glyph>✦</Glyph> },
  { id: "patch", label: "Pièce", action: "tool.patch", icon: <Glyph>▤</Glyph> },
  { id: "blur", label: "Goutte d'eau (flou)", action: "tool.blur", icon: <Glyph>◌</Glyph> },
  { id: "sharpen", label: "Netteté", action: "tool.sharpen", icon: <Glyph>◆</Glyph> },
  { id: "smudge", label: "Doigt", action: "tool.smudge", icon: <Glyph>☞</Glyph> },
  { id: "dodge", label: "Densité −", action: "tool.dodge", icon: <Glyph>◐</Glyph> },
  { id: "burn", label: "Densité +", action: "tool.burn", icon: <Glyph>◑</Glyph> },
  { id: "sponge", label: "Éponge", action: "tool.sponge", icon: <Glyph>▒</Glyph> },
  { id: "liquify", label: "Fluidité", action: "tool.liquify", icon: <Glyph>≋</Glyph> },
  { id: "text", label: "Texte", action: "tool.text", icon: <Glyph>T</Glyph> },
  { id: "pen", label: "Plume", action: "tool.pen", icon: <Glyph>✒</Glyph> },
  { id: "shape_rect", label: "Rectangle", action: "tool.shape_rect", icon: <Glyph>▭</Glyph> },
  { id: "shape_rounded_rect", label: "Rectangle arrondi", action: "tool.shape_rounded", icon: <Glyph>▢</Glyph> },
  { id: "shape_ellipse", label: "Ellipse", action: "tool.shape_ellipse", icon: <Glyph>◯</Glyph> },
  { id: "shape_line", label: "Ligne", action: "tool.shape_line", icon: <Glyph>╱</Glyph> },
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
