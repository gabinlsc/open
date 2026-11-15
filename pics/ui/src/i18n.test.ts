import { describe, expect, it } from "vitest";
import { hasEnglish, setLang, t } from "./i18n";

// Chaque texte d'interface passé à t()/tr() doit avoir sa traduction anglaise : ce test échoue sinon,
// pour qu'une nouvelle boîte de dialogue ne reste pas en français en mode anglais.
const sources = import.meta.glob(["./components/*.tsx", "./App.tsx", "./store.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

function literals(): string[] {
  const out = new Set<string>();
  const one = /\bt(?:r)?\(\s*"((?:[^"\\]|\\.)*)"\s*[,)]/g;
  const ternary = /\bt(?:r)?\([^()]*?\?\s*"((?:[^"\\]|\\.)*)"\s*:\s*"((?:[^"\\]|\\.)*)"\s*\)/g;
  for (const src of Object.values(sources)) {
    for (const m of src.matchAll(one)) out.add(JSON.parse(`"${m[1]}"`));
    for (const m of src.matchAll(ternary)) {
      out.add(JSON.parse(`"${m[1]}"`));
      out.add(JSON.parse(`"${m[2]}"`));
    }
  }
  return [...out].filter((s) => /[A-Za-zÀ-ÿ]{2,}/.test(s));
}

describe("traduction des boîtes de dialogue", () => {
  it("trouve bien les textes de l'interface", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(15);
    expect(literals().length).toBeGreaterThan(200);
  });
  it("a une version anglaise pour chaque texte affiché", () => {
    expect(literals().filter((s) => !hasEnglish(s))).toEqual([]);
  });
  it("garde les espaces de bord et distingue le contexte", () => {
    setLang("en");
    expect(t(" Gras")).toBe(" Bold");
    expect(t("Les dimensions doivent être des entiers de 1 à ")).toBe("Dimensions must be whole numbers from 1 to ");
    expect(t("Annuler")).toBe("Cancel");
    expect(t("Annuler", "action")).toBe("Undo");
    expect(t("Enregistrer", "action")).toBe("Save");
    setLang("fr");
    expect(t(" Gras")).toBe(" Gras");
    expect(t("Annuler", "action")).toBe("Annuler");
  });
});
