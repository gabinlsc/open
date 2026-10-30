//! Commandes IPC. Chaque commande modifie la session puis renvoie l'état complet : le frontend
//! n'a jamais à deviner l'effet d'une action.

use std::path::PathBuf;

use openpics_core::Vec2;
use tauri::State;

use super::Shared;
use crate::dto::{
    AppStatus, AxisDto, CanvasRect, GuideDrag, ImageOp, KeymapDto, LayerPropsDto, OpenResult, PaintOptionsDto,
    ResolvedTheme, SettingsDto, Snapshot, StyleDto, ThemeChoice, Tool, ToolOptionsDto, TransformDto,
};
use crate::error::{ApiError, SessionError};
use crate::session::Session;

type ApiResult<T> = Result<T, ApiError>;

/// Applique `f` à la session, redessine, et renvoie l'état.
fn mutate(shared: &Shared, f: impl FnOnce(&mut Session) -> Result<(), SessionError>) -> ApiResult<Snapshot> {
    let snapshot = {
        let mut session = shared.lock()?;
        f(&mut session)?;
        session.snapshot()
    };
    shared.redraw();
    Ok(snapshot)
}

fn anchor(x: Option<f64>, y: Option<f64>) -> Option<Vec2> {
    x.zip(y).map(|(x, y)| Vec2::new(x, y))
}

#[tauri::command]
pub fn app_status(shared: State<'_, Shared>) -> ApiResult<AppStatus> {
    Ok(AppStatus {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        portable: shared.portable,
        gpu_error: shared.gpu_error.lock().ok().and_then(|g| g.clone()),
        theme: shared.theme_choice(),
    })
}

#[tauri::command]
pub fn app_snapshot(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    Ok(shared.lock()?.snapshot())
}

#[tauri::command]
pub fn doc_new(shared: State<'_, Shared>, name: Option<String>, width: u32, height: u32) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.new_document(name, width, height).map(|_| ()))
}

#[tauri::command]
pub async fn doc_open(shared: State<'_, Shared>, paths: Vec<String>) -> ApiResult<OpenResult> {
    // `async` : Tauri exécute alors la commande hors du thread principal, l'UI ne gèle pas
    // pendant le décodage d'une grande image.
    let mut failures = Vec::new();
    for path in paths.into_iter().map(PathBuf::from) {
        let result = shared.lock()?.open_path(&path);
        if let Err(e) = result {
            failures.push(format!("{} : {e}", path.display()));
        }
    }
    shared.redraw();
    Ok(OpenResult { snapshot: shared.lock()?.snapshot(), failures })
}

#[tauri::command]
pub fn doc_save(shared: State<'_, Shared>, path: Option<String>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.save_active(path.map(PathBuf::from)))
}

#[tauri::command]
pub async fn doc_export(shared: State<'_, Shared>, path: String, jpeg_quality: Option<u8>) -> ApiResult<()> {
    // Aplatissement et encodage sous le verrou : l'UI reste fluide car la commande est asynchrone,
    // mais le document ne peut pas changer pendant l'export, ce qui garantit un fichier cohérent.
    Ok(shared.lock()?.export_active(&PathBuf::from(path), jpeg_quality)?)
}

#[tauri::command]
pub fn doc_close(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.close(id))
}

#[tauri::command]
pub fn doc_activate(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.activate(id))
}

#[tauri::command]
pub fn view_set_canvas(shared: State<'_, Shared>, rect: CanvasRect) -> ApiResult<Snapshot> {
    let snapshot = {
        let mut session = shared.lock()?;
        session.set_canvas(rect.width, rect.height);
        session.snapshot()
    };
    if let Ok(guard) = shared.canvas.lock() {
        if let Some(canvas) = guard.as_ref() {
            canvas.set_bounds(rect);
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub fn view_set_canvas_visible(shared: State<'_, Shared>, visible: bool) -> ApiResult<()> {
    if let Ok(guard) = shared.canvas.lock() {
        if let Some(canvas) = guard.as_ref() {
            canvas.set_visible(visible);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn view_zoom(
    shared: State<'_, Shared>,
    zoom_in: bool,
    anchor_x: Option<f64>,
    anchor_y: Option<f64>,
) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.zoom_step(zoom_in, anchor(anchor_x, anchor_y)))
}

#[tauri::command]
pub fn view_zoom_to(shared: State<'_, Shared>, zoom: f64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.zoom_to(zoom, None))
}

#[tauri::command]
pub fn view_fit(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::fit)
}

#[tauri::command]
pub fn view_actual_size(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::actual_size)
}

#[tauri::command]
pub fn view_set_settings(shared: State<'_, Shared>, settings: SettingsDto) -> ApiResult<Snapshot> {
    let snapshot = mutate(&shared, |s| {
        s.settings = settings.into();
        Ok(())
    })?;
    shared.save_prefs();
    Ok(snapshot)
}

#[tauri::command]
pub fn guide_drag(shared: State<'_, Shared>, axis: AxisDto, position: f64) -> ApiResult<GuideDrag> {
    let drag = shared.lock()?.guide_drag(axis.into(), position)?;
    shared.redraw();
    Ok(drag)
}

#[tauri::command]
pub fn guide_commit(shared: State<'_, Shared>, axis: AxisDto, position: f64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.guide_commit(axis.into(), position).map(|_| ()))
}

#[tauri::command]
pub fn guide_cancel(shared: State<'_, Shared>) -> ApiResult<()> {
    shared.lock()?.guide_cancel();
    shared.redraw();
    Ok(())
}

#[tauri::command]
pub fn guide_remove(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.guide_remove(id))
}

#[tauri::command]
pub fn guides_clear(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::guides_clear)
}

#[tauri::command]
pub fn layer_select(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_select(id))
}

#[tauri::command]
pub fn layer_add(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_add().map(|_| ()))
}

#[tauri::command]
pub fn group_add(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.group_add().map(|_| ()))
}

#[tauri::command]
pub fn layer_set_group(shared: State<'_, Shared>, id: u64, group: Option<u64>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_set_group(id, group))
}

#[tauri::command]
pub fn layer_move_step(shared: State<'_, Shared>, id: u64, up: bool) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_move_step(id, up))
}

#[tauri::command]
pub fn layer_remove(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_remove(id))
}

#[tauri::command]
pub fn layer_move(shared: State<'_, Shared>, id: u64, to: usize) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_move(id, to))
}

#[tauri::command]
pub fn layer_set_props(shared: State<'_, Shared>, id: u64, props: LayerPropsDto) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_set_props(id, &props))
}

#[tauri::command]
pub fn layer_set_styles(shared: State<'_, Shared>, id: u64, styles: Vec<StyleDto>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.layer_set_styles(id, &styles))
}

#[tauri::command]
pub async fn layer_transform(shared: State<'_, Shared>, transform: TransformDto) -> ApiResult<Snapshot> {
    // `async` : le rééchantillonnage d'un grand calque ne doit pas figer le thread principal.
    mutate(&shared, |s| s.layer_transform(&transform).map(|_| ()))
}

#[tauri::command]
pub fn mask_add(shared: State<'_, Shared>, id: u64, reveal: bool) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.mask_add(id, reveal))
}

#[tauri::command]
pub fn mask_remove(shared: State<'_, Shared>, id: u64) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.mask_remove(id))
}

#[tauri::command]
pub fn mask_set_enabled(shared: State<'_, Shared>, id: u64, enabled: bool) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.mask_set_enabled(id, enabled))
}

#[tauri::command]
pub fn history_undo(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.undo().map(|_| ()))
}

#[tauri::command]
pub fn history_redo(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.redo().map(|_| ()))
}

#[tauri::command]
pub async fn op_apply(shared: State<'_, Shared>, op: ImageOp) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.apply_op(op).map(|_| ()))
}

#[tauri::command]
pub fn tool_set(shared: State<'_, Shared>, tool: Tool) -> ApiResult<Snapshot> {
    mutate(&shared, |s| {
        s.set_tool(tool);
        Ok(())
    })
}

#[tauri::command]
pub fn select_all(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::select_all)
}

#[tauri::command]
pub fn select_none(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::deselect)
}

#[tauri::command]
pub fn select_invert(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::select_invert)
}

#[tauri::command]
pub async fn select_feather(shared: State<'_, Shared>, radius: f32) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.select_feather(radius))
}

#[tauri::command]
pub async fn select_grow(shared: State<'_, Shared>, radius: f32, grow: bool) -> ApiResult<Snapshot> {
    mutate(&shared, |s| s.select_grow(radius, grow))
}

#[tauri::command]
pub fn tool_finish(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, Session::tool_finish)
}

#[tauri::command]
pub fn tool_cancel(shared: State<'_, Shared>) -> ApiResult<Snapshot> {
    mutate(&shared, |s| {
        s.tool_cancel();
        Ok(())
    })
}

#[tauri::command]
pub fn tool_options_set(shared: State<'_, Shared>, options: ToolOptionsDto) -> ApiResult<Snapshot> {
    mutate(&shared, |s| {
        s.set_tool_options(options);
        Ok(())
    })
}

#[tauri::command]
pub fn paint_options_set(shared: State<'_, Shared>, options: PaintOptionsDto) -> ApiResult<Snapshot> {
    mutate(&shared, |s| {
        s.set_paint_options(&options);
        Ok(())
    })
}

#[tauri::command]
pub fn pan_modifier_set(shared: State<'_, Shared>, held: bool) -> ApiResult<()> {
    shared.lock()?.pan_modifier = held;
    Ok(())
}

#[tauri::command]
pub fn theme_set(shared: State<'_, Shared>, choice: ThemeChoice, resolved: ResolvedTheme) -> ApiResult<()> {
    shared.lock()?.theme = resolved;
    shared.save_theme_choice(choice);
    shared.redraw();
    Ok(())
}

#[tauri::command]
pub fn keymap_get(shared: State<'_, Shared>) -> ApiResult<KeymapDto> {
    Ok(shared.lock()?.keymap_dto())
}

#[tauri::command]
pub fn keymap_set(shared: State<'_, Shared>, action: String, chords: Vec<String>) -> ApiResult<KeymapDto> {
    let dto = {
        let mut session = shared.lock()?;
        session.keymap_set(&action, &chords)?;
        session.keymap_dto()
    };
    shared.save_prefs();
    Ok(dto)
}

#[tauri::command]
pub fn keymap_reset(shared: State<'_, Shared>) -> ApiResult<KeymapDto> {
    let dto = {
        let mut session = shared.lock()?;
        session.keymap.reset();
        session.keymap_dto()
    };
    shared.save_prefs();
    Ok(dto)
}
