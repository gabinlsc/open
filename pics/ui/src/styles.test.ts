import { describe, expect, it } from "vitest";
import { defaultStyle, parseNumber, replaceStyle, STYLE_KINDS, toggleStyle } from "./styles";

describe("styles de calque", () => {
  it("propose des valeurs par défaut pour chaque type", () => {
    for (const { kind } of STYLE_KINDS) expect(defaultStyle(kind).kind).toBe(kind);
  });
  it("active et désactive un type sans toucher aux autres", () => {
    let list = toggleStyle([], "stroke", true);
    list = toggleStyle(list, "drop_shadow", true);
    expect(list.map((s) => s.kind)).toEqual(["stroke", "drop_shadow"]);
    expect(toggleStyle(list, "stroke", false).map((s) => s.kind)).toEqual(["drop_shadow"]);
    // Réactiver un type déjà présent ne le duplique pas.
    expect(toggleStyle(list, "stroke", true).filter((s) => s.kind === "stroke")).toHaveLength(1);
  });
  it("remplace uniquement le style du même type", () => {
    const list = toggleStyle(toggleStyle([], "outer_glow", true), "stroke", true);
    const next = { ...defaultStyle("outer_glow"), opacity: 0.1 };
    const out = replaceStyle(list, next);
    expect(out.find((s) => s.kind === "outer_glow")).toEqual(next);
    expect(out.find((s) => s.kind === "stroke")).toEqual(defaultStyle("stroke"));
  });
  it("borne les saisies et garde l'ancienne valeur si illisible", () => {
    expect(parseNumber("12,5", 1, 0, 100)).toBe(12.5);
    expect(parseNumber("999", 1, 0, 100)).toBe(100);
    expect(parseNumber("-4", 1, 0, 100)).toBe(0);
    expect(parseNumber("abc", 7, 0, 100)).toBe(7);
    expect(parseNumber("  ", 7, 0, 100)).toBe(7);
  });
});
