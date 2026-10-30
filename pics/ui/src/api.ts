// Seul point de contact avec le cœur Rust. Aucune donnée de pixels ne transite par ici : le
// frontend envoie des intentions et reçoit des métadonnées.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppStatus } from "./gen/AppStatus";
import type { ApiError } from "./gen/ApiError";
import type { AxisDto } from "./gen/AxisDto";
import type { BlendInfo } from "./gen/BlendInfo";
import type { CanvasRect } from "./gen/CanvasRect";
import type { CursorEvent } from "./gen/CursorEvent";
import type { GuideDrag } from "./gen/GuideDrag";
import type { ImageOp } from "./gen/ImageOp";
import type { KeymapDto } from "./gen/KeymapDto";
import type { LayerDto } from "./gen/LayerDto";
import type { LayerPropsDto } from "./gen/LayerPropsDto";
import type { OpenResult } from "./gen/OpenResult";
import type { ResolvedTheme } from "./gen/ResolvedTheme";
import type { SettingsDto } from "./gen/SettingsDto";
import type { StyleDto } from "./gen/StyleDto";
import type { Snapshot } from "./gen/Snapshot";
import type { ThemeChoice } from "./gen/ThemeChoice";
import type { TransformDto } from "./gen/TransformDto";
import type { Tool } from "./gen/Tool";

export type { AppStatus, ApiError, AxisDto, BlendInfo, CursorEvent, ImageOp, KeymapDto, LayerDto, LayerPropsDto, SettingsDto, Snapshot, StyleDto, ThemeChoice, Tool, TransformDto, ResolvedTheme };

/** Erreur typée côté TS : `code` est stable, `message` est affichable tel quel. */
export class CommandError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (isApiError(e)) throw new CommandError(e.code, e.message);
    throw new CommandError("unknown", String(e));
  }
}

export const api = {
  status: () => call<AppStatus>("app_status"),
  snapshot: () => call<Snapshot>("app_snapshot"),
  docNew: (name: string | null, width: number, height: number) => call<Snapshot>("doc_new", { name, width, height }),
  docOpen: (paths: string[]) => call<OpenResult>("doc_open", { paths }),
  docSave: (path: string | null) => call<Snapshot>("doc_save", { path }),
  docExport: (path: string, jpegQuality: number | null) => call<void>("doc_export", { path, jpegQuality }),
  docClose: (id: number) => call<Snapshot>("doc_close", { id }),
  docActivate: (id: number) => call<Snapshot>("doc_activate", { id }),
  setCanvas: (rect: CanvasRect) => call<Snapshot>("view_set_canvas", { rect }),
  setCanvasVisible: (visible: boolean) => call<void>("view_set_canvas_visible", { visible }),
  zoom: (zoomIn: boolean) => call<Snapshot>("view_zoom", { zoomIn, anchorX: null, anchorY: null }),
  zoomTo: (zoom: number) => call<Snapshot>("view_zoom_to", { zoom }),
  fit: () => call<Snapshot>("view_fit"),
  actualSize: () => call<Snapshot>("view_actual_size"),
  setSettings: (settings: SettingsDto) => call<Snapshot>("view_set_settings", { settings }),
  guideDrag: (axis: AxisDto, position: number) => call<GuideDrag>("guide_drag", { axis, position }),
  guideCommit: (axis: AxisDto, position: number) => call<Snapshot>("guide_commit", { axis, position }),
  guideCancel: () => call<void>("guide_cancel"),
  guideRemove: (id: number) => call<Snapshot>("guide_remove", { id }),
  guidesClear: () => call<Snapshot>("guides_clear"),
  layerSelect: (id: number) => call<Snapshot>("layer_select", { id }),
  layerAdd: () => call<Snapshot>("layer_add"),
  groupAdd: () => call<Snapshot>("group_add"),
  layerSetGroup: (id: number, group: number | null) => call<Snapshot>("layer_set_group", { id, group }),
  layerMoveStep: (id: number, up: boolean) => call<Snapshot>("layer_move_step", { id, up }),
  layerRemove: (id: number) => call<Snapshot>("layer_remove", { id }),
  layerMove: (id: number, to: number) => call<Snapshot>("layer_move", { id, to }),
  layerSetProps: (id: number, props: LayerPropsDto) => call<Snapshot>("layer_set_props", { id, props }),
  layerSetStyles: (id: number, styles: StyleDto[]) => call<Snapshot>("layer_set_styles", { id, styles }),
  layerTransform: (transform: TransformDto) => call<Snapshot>("layer_transform", { transform }),
  maskAdd: (id: number, reveal: boolean) => call<Snapshot>("mask_add", { id, reveal }),
  maskRemove: (id: number) => call<Snapshot>("mask_remove", { id }),
  maskSetEnabled: (id: number, enabled: boolean) => call<Snapshot>("mask_set_enabled", { id, enabled }),
  undo: () => call<Snapshot>("history_undo"),
  redo: () => call<Snapshot>("history_redo"),
  op: (op: ImageOp) => call<Snapshot>("op_apply", { op }),
  setTool: (tool: Tool) => call<Snapshot>("tool_set", { tool }),
  setPanModifier: (held: boolean) => call<void>("pan_modifier_set", { held }),
  setTheme: (choice: ThemeChoice, resolved: ResolvedTheme) => call<void>("theme_set", { choice, resolved }),
  keymap: () => call<KeymapDto>("keymap_get"),
  keymapSet: (action: string, chords: string[]) => call<KeymapDto>("keymap_set", { action, chords }),
  keymapReset: () => call<KeymapDto>("keymap_reset"),
};

export interface Events {
  onState(cb: (s: Snapshot) => void): Promise<UnlistenFn>;
  onCursor(cb: (c: CursorEvent) => void): Promise<UnlistenFn>;
  onGpuError(cb: (message: string) => void): Promise<UnlistenFn>;
  onOpenFailed(cb: (messages: string[]) => void): Promise<UnlistenFn>;
}

export const events: Events = {
  onState: (cb) => listen<Snapshot>("state:changed", (e) => cb(e.payload)),
  onCursor: (cb) => listen<CursorEvent>("cursor:moved", (e) => cb(e.payload)),
  onGpuError: (cb) => listen<string>("gpu:error", (e) => cb(e.payload)),
  onOpenFailed: (cb) => listen<string[]>("open:failed", (e) => cb(e.payload)),
};
