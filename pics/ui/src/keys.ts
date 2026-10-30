// Doit rester cohérent avec `Chord` de openpics-core (forme canonique « Ctrl+Alt+Shift+Touche »).

const CODE_KEYS: Record<string, string> = {
  Semicolon: ";",
  Quote: "'",
  Equal: "=",
  Minus: "-",
  NumpadAdd: "+",
  NumpadSubtract: "-",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  BracketLeft: "[",
  BracketRight: "]",
  Backquote: "`",
};

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph"]);

export interface KeyLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/** `null` pour une simple touche modificatrice ou une touche que le cœur n'accepterait pas. */
export function chordFromEvent(e: KeyLike): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;
  let key: string;
  const byCode = CODE_KEYS[e.code];
  if (e.key.length === 1 && /[a-z0-9]/i.test(e.key)) {
    key = e.key.toUpperCase();
  } else if (byCode) {
    key = byCode;
  } else if (e.key === " ") {
    key = "Space";
  } else if (e.key.length === 1) {
    key = e.key;
  } else if (/^[A-Za-z][A-Za-z0-9]*$/.test(e.key)) {
    key = e.key[0]!.toUpperCase() + e.key.slice(1);
  } else {
    return null;
  }
  const mods = [e.ctrlKey || e.metaKey ? "Ctrl" : "", e.altKey ? "Alt" : "", e.shiftKey ? "Shift" : ""].filter(Boolean);
  return [...mods, key].join("+");
}

/** Table inverse « combinaison → action » (la première action déclarée gagne en cas de doublon). */
export function buildLookup(bindings: Record<string, string[] | undefined>): Map<string, string> {
  const map = new Map<string, string>();
  for (const [action, chords] of Object.entries(bindings)) {
    for (const chord of chords ?? []) if (!map.has(chord)) map.set(chord, action);
  }
  return map;
}

/** Affichage lisible : « Ctrl+Shift+Z » reste tel quel, on se contente de remplacer les noms de touches verbeux. */
export function prettyChord(chord: string): string {
  return chord.replace("ArrowLeft", "←").replace("ArrowRight", "→").replace("ArrowUp", "↑").replace("ArrowDown", "↓");
}
