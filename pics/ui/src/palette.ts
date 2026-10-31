export interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
}

const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** Score d'une suite de lettres dans l'ordre (« tgm » trouve « Tout sélectionner » ? non : « sel » oui). Plus petit = mieux ; null = absent. */
function score(query: string, text: string): number | null {
  if (!query) return 0;
  const idx = text.indexOf(query);
  if (idx >= 0) return idx === 0 ? 0 : text[idx - 1] === " " ? 1 : 2 + idx / 100;
  // sous-suite : chaque lettre apparaît dans l'ordre
  let pos = -1;
  let gaps = 0;
  for (const ch of query) {
    const next = text.indexOf(ch, pos + 1);
    if (next < 0) return null;
    gaps += next - pos - 1;
    pos = next;
  }
  return 10 + gaps / 100;
}

export function rank(query: string, items: PaletteItem[], limit = 50): PaletteItem[] {
  const q = fold(query.trim());
  const scored = items.map((item) => ({ item, s: score(q, fold(item.label)) }));
  // Une recherche approximative (lettres dans l'ordre) ne sert que si rien ne contient la saisie telle quelle :
  // sinon « sel » remonterait « Stylisation › Relief ».
  const exact = scored.some((x) => x.s !== null && x.s < 10);
  return scored
    .filter((x): x is { item: PaletteItem; s: number } => x.s !== null && (!exact || x.s < 10))
    .sort((a, b) => a.s - b.s || a.item.label.localeCompare(b.item.label))
    .slice(0, limit)
    .map((x) => x.item);
}
