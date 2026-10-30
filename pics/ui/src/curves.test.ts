import { describe, expect, it } from "vitest";
import { addPoint, normalizePoints, removePoint, svgPath } from "./curves";

describe("éditeur de courbes", () => {
  it("trie, borne et dédoublonne, avec au moins deux points", () => {
    expect(normalizePoints([[1, 2], [-1, 0.5], [0.5, NaN]])).toEqual([[0, 0.5], [0.5, 0], [1, 1]]);
    expect(normalizePoints([[0.3, 0.3], [0.3, 0.9]])).toEqual([[0, 0], [1, 1]]);
    expect(normalizePoints([])).toEqual([[0, 0], [1, 1]]);
  });
  it("ajoute un point sans changer la forme de la courbe", () => {
    const p = addPoint([[0, 0], [1, 0.5]], 0.5);
    expect(p).toEqual([[0, 0], [0.5, 0.25], [1, 0.5]]);
  });
  it("ne retire jamais les extrémités", () => {
    const p: [number, number][] = [[0, 0], [0.5, 0.7], [1, 1]];
    expect(removePoint(p, 0)).toBe(p);
    expect(removePoint(p, 2)).toBe(p);
    expect(removePoint(p, 1)).toEqual([[0, 0], [1, 1]]);
  });
  it("produit un chemin SVG orienté y vers le haut", () => {
    expect(svgPath([[0, 0], [1, 1]], 100)).toBe("M0.0 100.0 L100.0 0.0");
  });
});
