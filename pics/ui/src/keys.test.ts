import { describe, expect, it } from "vitest";
import { buildLookup, chordFromEvent } from "./keys";

const ev = (key: string, mods: Partial<{ ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }> = {}, code = "") => ({
  key,
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("chordFromEvent", () => {
  it("normalise lettres et modificateurs dans l'ordre Ctrl, Alt, Shift", () => {
    expect(chordFromEvent(ev("z", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+Z");
    expect(chordFromEvent(ev("S", { ctrlKey: true, altKey: true, shiftKey: true }))).toBe("Ctrl+Alt+Shift+S");
  });
  it("traite Cmd comme Ctrl", () => {
    expect(chordFromEvent(ev("o", { metaKey: true }))).toBe("Ctrl+O");
  });
  it("ignore les modificateurs seuls", () => {
    expect(chordFromEvent(ev("Control", { ctrlKey: true }))).toBeNull();
    expect(chordFromEvent(ev("Shift", { shiftKey: true }))).toBeNull();
  });
  it("utilise le code physique pour la ponctuation", () => {
    expect(chordFromEvent(ev(":", { ctrlKey: true, shiftKey: true }, "Semicolon"))).toBe("Ctrl+Shift+;");
    expect(chordFromEvent(ev("=", { ctrlKey: true }, "Equal"))).toBe("Ctrl+=");
    expect(chordFromEvent(ev("-", { ctrlKey: true }, "Minus"))).toBe("Ctrl+-");
  });
  it("garde les touches nommées", () => {
    expect(chordFromEvent(ev("Tab", { ctrlKey: true }))).toBe("Ctrl+Tab");
    expect(chordFromEvent(ev("F5"))).toBe("F5");
    expect(chordFromEvent(ev("ArrowLeft", { altKey: true }))).toBe("Alt+ArrowLeft");
  });
  it("accepte les chiffres et l'espace", () => {
    expect(chordFromEvent(ev("0", { ctrlKey: true }))).toBe("Ctrl+0");
    expect(chordFromEvent(ev(" "))).toBe("Space");
  });
});

describe("buildLookup", () => {
  it("renvoie la première action en cas de doublon", () => {
    const m = buildLookup({ a: ["Ctrl+X"], b: ["Ctrl+X", "Ctrl+Y"] });
    expect(m.get("Ctrl+X")).toBe("a");
    expect(m.get("Ctrl+Y")).toBe("b");
  });
});
