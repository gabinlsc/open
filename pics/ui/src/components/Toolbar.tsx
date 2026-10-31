import { t as tr } from "../i18n";
import { api, type Tool } from "../api";
import { prettyChord } from "../keys";
import { useStore } from "../store";
import {
  BlurIcon, BrushIcon, BucketIcon, BurnIcon, CloneIcon, DodgeIcon, EllipseIcon, EraserIcon, GradientIcon, HealIcon, LineIcon, LiquifyIcon,
  PatchIcon, PenIcon, PencilIcon, RectIcon, RoundedRectIcon, SharpenIcon, SmudgeIcon, SpongeIcon, SpotHealIcon, TextIcon,
  EllipseSelectIcon, HandIcon, LassoIcon, PolygonLassoIcon, RectSelectIcon, WandIcon, ZoomIcon } from "./Icons";

const TOOLS: { id: Tool; label: string; action: string; icon: JSX.Element }[] = [
  { id: "hand", label: "Main", action: "tool.hand", icon: <HandIcon /> },
  { id: "zoom", label: "Zoom", action: "tool.zoom", icon: <ZoomIcon /> },
  { id: "rect_select", label: "Sélection rectangulaire", action: "tool.rect_select", icon: <RectSelectIcon /> },
  { id: "ellipse_select", label: "Sélection elliptique", action: "tool.ellipse_select", icon: <EllipseSelectIcon /> },
  { id: "lasso", label: "Lasso", action: "tool.lasso", icon: <LassoIcon /> },
  { id: "polygon_lasso", label: "Lasso polygonal", action: "tool.polygon_lasso", icon: <PolygonLassoIcon /> },
  { id: "magic_wand", label: "Baguette magique", action: "tool.wand", icon: <WandIcon /> },
  { id: "brush", label: "Pinceau", action: "tool.brush", icon: <BrushIcon /> },
  { id: "pencil", label: "Crayon", action: "tool.pencil", icon: <PencilIcon /> },
  { id: "eraser", label: "Gomme", action: "tool.eraser", icon: <EraserIcon /> },
  { id: "bucket", label: "Pot de peinture", action: "tool.bucket", icon: <BucketIcon /> },
  { id: "gradient", label: "Dégradé", action: "tool.gradient", icon: <GradientIcon /> },
  { id: "clone_stamp", label: "Tampon de duplication", action: "tool.clone", icon: <CloneIcon /> },
  { id: "heal", label: "Correcteur", action: "tool.heal", icon: <HealIcon /> },
  { id: "spot_heal", label: "Correcteur localisé", action: "tool.spot_heal", icon: <SpotHealIcon /> },
  { id: "patch", label: "Pièce", action: "tool.patch", icon: <PatchIcon /> },
  { id: "blur", label: "Goutte d'eau (flou)", action: "tool.blur", icon: <BlurIcon /> },
  { id: "sharpen", label: "Netteté", action: "tool.sharpen", icon: <SharpenIcon /> },
  { id: "smudge", label: "Doigt", action: "tool.smudge", icon: <SmudgeIcon /> },
  { id: "dodge", label: "Densité −", action: "tool.dodge", icon: <DodgeIcon /> },
  { id: "burn", label: "Densité +", action: "tool.burn", icon: <BurnIcon /> },
  { id: "sponge", label: "Éponge", action: "tool.sponge", icon: <SpongeIcon /> },
  { id: "liquify", label: "Fluidité", action: "tool.liquify", icon: <LiquifyIcon /> },
  { id: "text", label: "Texte", action: "tool.text", icon: <TextIcon /> },
  { id: "pen", label: "Plume", action: "tool.pen", icon: <PenIcon /> },
  { id: "shape_rect", label: "Rectangle", action: "tool.shape_rect", icon: <RectIcon /> },
  { id: "shape_rounded_rect", label: "Rectangle arrondi", action: "tool.shape_rounded", icon: <RoundedRectIcon /> },
  { id: "shape_ellipse", label: "Ellipse", action: "tool.shape_ellipse", icon: <EllipseIcon /> },
  { id: "shape_line", label: "Ligne", action: "tool.shape_line", icon: <LineIcon /> },
];

export function Toolbar() {
  useStore((s) => s.lang); // re-rendu au changement de langue
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
            aria-label={tr(t.label)}
            title={chord ? `${tr(t.label)} (${prettyChord(chord)})` : tr(t.label)}
            onClick={() => void run(() => api.setTool(t.id))}
          >
            {t.icon}
          </button>
        );
      })}
    </div>
  );
}
