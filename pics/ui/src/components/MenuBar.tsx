import { useEffect, useRef, useState } from "react";
import { api, type ThemeChoice } from "../api";
import { ACTIONS, runAction, startFilter } from "../actions";
import { prettyChord } from "../keys";
import { useStore } from "../store";
import { applyTheme } from "../theme";

type Item =
  | { sep: true }
  | { action: string; checked?: boolean }
  | { label: string; onSelect: () => void; checked?: boolean };

export function MenuBar() {
  const snapshot = useStore((s) => s.snapshot);
  const keymap = useStore((s) => s.keymap);
  const themeChoice = useStore((s) => s.themeChoice);
  const [open, setOpen] = useState<number | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const settings = snapshot?.settings;

  const chooseTheme = (choice: ThemeChoice) => {
    useStore.getState().setThemeChoice(choice);
    void api.setTheme(choice, applyTheme(choice));
  };

  const filterItems: Item[] = (snapshot?.filters ?? []).map((f) => ({ label: `${f.category} › ${f.label}…`, onSelect: () => void startFilter(f.id) }));
  const adjustmentKinds = snapshot?.adjustmentKinds ?? [];
  const hasDoc = (snapshot?.docs.length ?? 0) > 0;
  const menus: { label: string; items: Item[] }[] = [
    {
      label: "Fichier",
      items: [
        { action: "file.new" },
        { action: "file.open" },
        { action: "file.import_svg" },
        { sep: true },
        { action: "file.save" },
        { action: "file.save_as" },
        { action: "file.export" },
        { sep: true },
        { action: "file.close" },
      ],
    },
    { label: "Édition", items: [{ action: "edit.undo" }, { action: "edit.redo" }, { sep: true }, { action: "edit.shortcuts" }] },
    {
      label: "Calque",
      items: [
        { action: "layer.new" },
        { action: "layer.delete" },
        { action: "layer.transform" },
        { sep: true },
        { action: "layer.mask_reveal" },
        { action: "layer.mask_hide" },
        { action: "layer.rasterize" },
        ...(hasDoc ? [{ sep: true } as Item, ...adjustmentKinds.map((k): Item => ({ label: `Réglage : ${k.label}`, onSelect: () => void useStore.getState().run(() => api.adjustmentAdd(k.id)) }))] : []),
      ],
    },
    {
      label: "Sélection",
      items: [
        { action: "select.all" },
        { action: "select.none" },
        { action: "select.invert" },
        { sep: true },
        { action: "select.feather" },
        { action: "select.grow" },
        { action: "select.shrink" },
      ],
    },
    { label: "Filtre", items: filterItems },
    { label: "Image", items: [{ action: "image.flip_h" }, { action: "image.flip_v" }, { action: "image.invert" }] },
    {
      label: "Affichage",
      items: [
        { action: "view.zoom_in" },
        { action: "view.zoom_out" },
        { action: "view.fit" },
        { action: "view.actual_size" },
        { sep: true },
        { action: "view.toggle_rulers", checked: settings?.showRulers },
        { action: "view.toggle_guides", checked: settings?.showGuides },
        { action: "view.toggle_grid", checked: settings?.showGrid },
        { action: "view.toggle_snap", checked: settings?.snapEnabled },
        { action: "view.clear_guides" },
        { sep: true },
        { label: "Thème : système", onSelect: () => chooseTheme("system"), checked: themeChoice === "system" },
        { label: "Thème : clair", onSelect: () => chooseTheme("light"), checked: themeChoice === "light" },
        { label: "Thème : sombre", onSelect: () => chooseTheme("dark"), checked: themeChoice === "dark" },
      ],
    },
  ];

  // Les menus recouvrent le canvas natif : on le signale pour qu'il se masque le temps de l'ouverture.
  useEffect(() => {
    useStore.getState().setMenuOpen(open !== null);
    return () => useStore.getState().setMenuOpen(false);
  }, [open]);

  useEffect(() => {
    if (open === null) return;
    const onDown = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) setOpen(null);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  useEffect(() => {
    if (open !== null) barRef.current?.querySelector<HTMLElement>(`[data-menu="${open}"] [role=menuitem]:not([aria-disabled=true])`)?.focus();
  }, [open]);

  const onBarKey = (e: React.KeyboardEvent) => {
    if (open === null) return;
    const items = [...(barRef.current?.querySelectorAll<HTMLElement>(`[data-menu="${open}"] [role=menuitem]:not([aria-disabled=true])`) ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") items[(at + 1) % items.length]?.focus();
    else if (e.key === "ArrowUp") items[(at - 1 + items.length) % items.length]?.focus();
    else if (e.key === "ArrowRight") setOpen((open + 1) % menus.length);
    else if (e.key === "ArrowLeft") setOpen((open - 1 + menus.length) % menus.length);
    else if (e.key === "Escape") {
      setOpen(null);
      barRef.current?.querySelectorAll<HTMLElement>("[data-top]")[open]?.focus();
    } else return;
    e.preventDefault();
  };

  return (
    <div className="menubar" role="menubar" aria-label="Menu principal" ref={barRef} onKeyDown={onBarKey}>
      {menus.map((menu, i) => (
        <div key={menu.label} className="menu" data-menu={i}>
          <button
            data-top
            role="menuitem"
            aria-haspopup="menu"
            aria-expanded={open === i}
            className={open === i ? "top active" : "top"}
            onClick={() => setOpen(open === i ? null : i)}
            onMouseEnter={() => open !== null && setOpen(i)}
          >
            {menu.label}
          </button>
          {open === i && (
            <div className="dropdown" role="menu" aria-label={menu.label}>
              {menu.items.map((item, k) => {
                if ("sep" in item) return <hr key={k} role="separator" />;
                const isAction = "action" in item;
                const def = isAction ? ACTIONS[item.action] : undefined;
                const label = isAction ? (def?.label ?? item.action) : item.label;
                const disabled = isAction ? def?.enabled?.() === false : false;
                const chord = isAction ? keymap[item.action]?.[0] : undefined;
                const checked = item.checked;
                return (
                  <button
                    key={k}
                    role={checked === undefined ? "menuitem" : "menuitemcheckbox"}
                    aria-checked={checked}
                    aria-disabled={disabled}
                    tabIndex={-1}
                    className="item"
                    onClick={() => {
                      if (disabled) return;
                      setOpen(null);
                      if (isAction) runAction(item.action);
                      else item.onSelect();
                    }}
                  >
                    <span className="check" aria-hidden="true">{checked ? "✓" : ""}</span>
                    <span className="label">{label}</span>
                    {chord && <kbd>{prettyChord(chord)}</kbd>}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
