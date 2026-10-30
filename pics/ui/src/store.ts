import { create } from "zustand";
import { api, CommandError, type CursorEvent, type KeymapDto, type SettingsDto, type Snapshot, type ThemeChoice } from "./api";

export type Dialog =
  | { kind: "new" }
  | { kind: "export" }
  | { kind: "transform" }
  | { kind: "shortcuts" }
  | { kind: "confirmClose"; id: number; name: string }
  | null;

export interface Notice {
  kind: "error" | "info";
  text: string;
}

interface UiState {
  ready: boolean;
  snapshot: Snapshot | null;
  keymap: KeymapDto["bindings"];
  themeChoice: ThemeChoice;
  gpuError: string | null;
  cursor: CursorEvent | null;
  dialog: Dialog;
  /** Un menu déroulant est ouvert : il recouvre la zone du canvas natif. */
  menuOpen: boolean;
  notice: Notice | null;
  version: string;
  portable: boolean;

  init(): Promise<void>;
  /** Remplace l'état venu du cœur (réponse de commande ou événement). */
  apply(snapshot: Snapshot): void;
  setCursor(c: CursorEvent | null): void;
  setDialog(d: Dialog): void;
  setMenuOpen(open: boolean): void;
  notify(n: Notice | null): void;
  /** Exécute une commande IPC et applique son résultat ; toute erreur devient une notification. */
  run(task: () => Promise<Snapshot>): Promise<boolean>;
  setSetting<K extends keyof SettingsDto>(key: K, value: SettingsDto[K]): Promise<void>;
  setKeymap(bindings: KeymapDto["bindings"]): void;
  setThemeChoice(choice: ThemeChoice): void;
  setGpuError(message: string | null): void;
}

export function errorText(e: unknown): string {
  return e instanceof CommandError ? e.message : e instanceof Error ? e.message : String(e);
}

export const useStore = create<UiState>()((set, get) => ({
  ready: false,
  snapshot: null,
  keymap: {},
  themeChoice: "system",
  gpuError: null,
  cursor: null,
  dialog: null,
  menuOpen: false,
  notice: null,
  version: "",
  portable: false,

  async init() {
    try {
      const [status, snapshot, keymap] = await Promise.all([api.status(), api.snapshot(), api.keymap()]);
      set({
        ready: true,
        snapshot,
        keymap: keymap.bindings,
        themeChoice: status.theme,
        gpuError: status.gpuError,
        version: status.version,
        portable: status.portable,
      });
    } catch (e) {
      set({ ready: true, notice: { kind: "error", text: `Démarrage impossible : ${errorText(e)}` } });
    }
  },

  apply(snapshot) {
    set({ snapshot });
  },

  setCursor(cursor) {
    set({ cursor });
  },

  setDialog(dialog) {
    set({ dialog });
  },

  setMenuOpen(menuOpen) {
    set({ menuOpen });
  },

  notify(notice) {
    set({ notice });
  },

  async run(task) {
    try {
      set({ snapshot: await task() });
      return true;
    } catch (e) {
      set({ notice: { kind: "error", text: errorText(e) } });
      return false;
    }
  },

  async setSetting(key, value) {
    const current = get().snapshot?.settings;
    if (!current) return;
    await get().run(() => api.setSettings({ ...current, [key]: value }));
  },

  setKeymap(keymap) {
    set({ keymap });
  },

  setThemeChoice(themeChoice) {
    set({ themeChoice });
  },

  setGpuError(gpuError) {
    set({ gpuError });
  },
}));

export function activeDoc(s: { snapshot: Snapshot | null }) {
  const snap = s.snapshot;
  return snap?.docs.find((d) => d.id === snap.activeId) ?? null;
}
