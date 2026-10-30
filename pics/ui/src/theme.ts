import type { ResolvedTheme, ThemeChoice } from "./api";

export function resolveTheme(choice: ThemeChoice, prefersDark: boolean): ResolvedTheme {
  if (choice === "system") return prefersDark ? "dark" : "light";
  return choice;
}

/** Applique le thème au document ; renvoie le thème résolu pour que le cœur règle le fond du canvas. */
export function applyTheme(choice: ThemeChoice): ResolvedTheme {
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const resolved = resolveTheme(choice, prefersDark);
  document.documentElement.dataset.theme = resolved;
  return resolved;
}
