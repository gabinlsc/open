import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import { ACTIONS } from "./actions";
import { detectLang, hasEnglish, setLang, t } from "./i18n";
import { rank, type PaletteItem } from "./palette";

const items: PaletteItem[] = [
  { id: "a", label: "Tout sélectionner" },
  { id: "b", label: "Désélectionner" },
  { id: "c", label: "Sélectionner le sujet…" },
  { id: "d", label: "Pinceau" },
];

describe("palette de commandes", () => {
  it("ignore casse et accents, et classe début de mot avant milieu", () => {
    // début de libellé (c) < début de mot (a) < milieu de mot (b)
    expect(rank("SELECTION", items).map((i) => i.id)).toEqual(["c", "a", "b"]);
  });
  it("accepte une suite de lettres dans l'ordre et rejette le reste", () => {
    expect(rank("pnc", items).map((i) => i.id)).toEqual(["d"]);
    expect(rank("zzz", items)).toEqual([]);
  });
  it("renvoie tout (borné) pour une requête vide", () => {
    expect(rank("", items)).toHaveLength(4);
    expect(rank("", items, 2)).toHaveLength(2);
  });
});

describe("langue", () => {
  it("choisit la langue mémorisée, sinon celle du navigateur", () => {
    expect(detectLang("en", "fr-FR")).toBe("en");
    expect(detectLang(null, "fr-CA")).toBe("fr");
    expect(detectLang("xx", "de-DE")).toBe("en");
  });
  it("traduit chaque action du registre et retombe sur le français sinon", () => {
    const missing = Object.entries(ACTIONS).filter(([, a]) => !hasEnglish(a.label)).map(([id, a]) => `${id}: ${a.label}`);
    expect(missing).toEqual([]);
    setLang("en");
    expect(t("Enregistrer")).toBe("Save");
    expect(t("Texte inconnu")).toBe("Texte inconnu");
    setLang("fr");
    expect(t("Enregistrer")).toBe("Enregistrer");
  });
});
