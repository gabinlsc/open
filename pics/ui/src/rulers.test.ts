import { describe, expect, it } from "vitest";
import { niceStep, rulerTicks } from "./rulers";

describe("niceStep", () => {
  it("choisit un pas rond assez espacé", () => {
    expect(niceStep(1)).toBe(100); // 100 px de doc = 100 px écran ≥ 60
    expect(niceStep(10)).toBe(10);
    expect(niceStep(0.1)).toBe(1000);
  });
  it("reste borné pour des entrées dégénérées", () => {
    expect(niceStep(0)).toBe(50000);
    expect(niceStep(NaN)).toBe(50000);
    expect(niceStep(1e-9)).toBe(50000);
  });
});

describe("rulerTicks", () => {
  it("place le 0 à l'origine et marque les graduations principales", () => {
    const t = rulerTicks(100, 1, 400);
    const zero = t.find((x) => x.value === 0);
    expect(zero).toEqual({ pos: 100, value: 0, major: true });
    expect(t.find((x) => x.value === 100)?.major).toBe(true);
    expect(t.find((x) => x.value === 20)?.major).toBe(false);
  });
  it("couvre la zone visible quand l'origine est hors écran", () => {
    const t = rulerTicks(-500, 1, 300);
    expect(t[0]!.pos).toBeLessThanOrEqual(0);
    expect(t[t.length - 1]!.pos).toBeGreaterThanOrEqual(250);
  });
  it("ne produit rien d'absurde pour des entrées invalides ni trop de graduations", () => {
    expect(rulerTicks(0, 0, 100)).toEqual([]);
    expect(rulerTicks(NaN, 1, 100)).toEqual([]);
    expect(rulerTicks(0, 1e-9, 1e9).length).toBeLessThanOrEqual(2000);
  });
});
