import { describe, expect, it, vi } from "vitest";

// Le plugin de dialogue n'existe que dans Tauri : on le neutralise pour tester le registre seul.
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { ACTIONS } from "./actions";

// Copie de DEFAULT_BINDINGS (crates/core/src/shortcuts.rs) : si un identifiant change côté cœur,
// ce test échoue et rappelle de mettre à jour le registre.
const CORE_ACTIONS = [
  "file.new", "file.open", "file.save", "file.save_as", "file.export", "file.close", "app.palette",
  "edit.undo", "edit.redo", "view.zoom_in", "view.zoom_out", "view.fit", "view.actual_size",
  "view.toggle_rulers", "view.toggle_grid", "view.toggle_guides", "view.toggle_snap",
  "layer.new", "select.all", "select.none", "select.invert", "tool.rect_select", "tool.lasso", "tool.wand", "tool.finish", "tool.cancel", "tool.brush", "tool.pencil", "tool.eraser", "tool.gradient", "tool.bucket", "tool.clone", "tool.heal", "tool.blur", "paint.swap_colors", "tab.next", "tab.prev", "tool.hand", "tool.zoom",
];

describe("registre d'actions", () => {
  it("implémente toutes les actions raccourcissables du cœur", () => {
    for (const id of CORE_ACTIONS) expect(ACTIONS[id], id).toBeDefined();
  });
  it("donne un libellé à chaque action", () => {
    for (const [id, a] of Object.entries(ACTIONS)) expect(a.label.length, id).toBeGreaterThan(0);
  });
});
