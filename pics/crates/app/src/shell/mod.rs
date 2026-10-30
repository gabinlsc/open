//! Coque Tauri : fenêtre principale (interface web), canvas natif et commandes IPC.

mod commands;
mod native;
mod render;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use tauri::{AppHandle, Emitter, Manager};

use crate::dto::{CursorEvent, Snapshot};
use crate::error::{ApiError, SessionError};
use crate::prefs::{Prefs, config_dir};
use crate::session::Session;

pub struct AppShared {
    pub session: Mutex<Session>,
    prefs_path: PathBuf,
    models_dir: PathBuf,
    recovery_dir: PathBuf,
    actions_dir: PathBuf,
    portable: bool,
    app: AppHandle,
    canvas: Mutex<Option<native::Canvas>>,
    gpu_error: Mutex<Option<String>>,
}

pub type Shared = Arc<AppShared>;

impl AppShared {
    pub fn lock(&self) -> Result<MutexGuard<'_, Session>, ApiError> {
        // Un panic dans un autre thread empoisonne le verrou ; l'état reste cohérent car chaque
        // méthode de `Session` laisse les données valides avant de pouvoir échouer.
        Ok(self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    pub fn redraw(&self) {
        if let Ok(guard) = self.canvas.lock() {
            if let Some(canvas) = guard.as_ref() {
                canvas.redraw();
            }
        }
    }

    pub fn emit_state(&self) {
        let Ok(session) = self.lock() else { return };
        let snapshot: Snapshot = session.snapshot();
        drop(session);
        let _ = self.app.emit("state:changed", snapshot);
    }

    pub fn emit_cursor(&self, event: CursorEvent) {
        let _ = self.app.emit("cursor:moved", event);
    }

    /// Changement venu de la fenêtre native (molette, glisser) : redessiner et informer l'UI.
    pub fn native_changed(&self) {
        self.redraw();
        self.emit_state();
    }

    pub fn set_gpu_error(&self, message: String) {
        if let Ok(mut slot) = self.gpu_error.lock() {
            *slot = Some(message.clone());
        }
        let _ = self.app.emit("gpu:error", message);
    }

    pub fn save_prefs(&self) {
        let Ok(session) = self.lock() else { return };
        let prefs = Prefs { theme: self.theme_choice(), settings: session.settings, keymap: session.keymap.clone() };
        drop(session);
        if let Err(e) = prefs.save(&self.prefs_path) {
            eprintln!("préférences non enregistrées : {e}");
        }
    }

    fn theme_choice(&self) -> crate::dto::ThemeChoice {
        Prefs::load(&self.prefs_path).theme
    }

    pub fn save_theme_choice(&self, choice: crate::dto::ThemeChoice) {
        let Ok(session) = self.lock() else { return };
        let prefs = Prefs { theme: choice, settings: session.settings, keymap: session.keymap.clone() };
        drop(session);
        if let Err(e) = prefs.save(&self.prefs_path) {
            eprintln!("préférences non enregistrées : {e}");
        }
    }

    /// Ouvre des fichiers hors du thread appelant (le décodage d'une grande image est long) puis
    /// prévient l'interface.
    pub fn open_paths_in_background(self: &Arc<Self>, paths: Vec<PathBuf>) {
        let shared = Arc::clone(self);
        std::thread::spawn(move || {
            let mut failures = Vec::new();
            for path in paths {
                let result: Result<(), SessionError> = shared
                    .lock()
                    .map_err(|_| SessionError::NoActiveDocument)
                    .and_then(|mut s| s.open_path(&path).map(|_| ()));
                if let Err(e) = result {
                    failures.push(format!("{} : {e}", path.display()));
                }
            }
            shared.redraw();
            shared.emit_state();
            if !failures.is_empty() {
                let _ = shared.app.emit("open:failed", failures);
            }
        });
    }
}

/// Copie de récupération toutes les 60 s ; une erreur d'écriture ne doit jamais gêner le travail en cours.
fn spawn_autosave(shared: Shared) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            if let Ok(mut session) = shared.session.lock() {
                let _ = session.autosave();
            }
        }
    });
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from));
            let default_dir = app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."));
            let portable = exe_dir.as_deref().is_some_and(|d| d.join("portable").exists());
            let base_dir = config_dir(exe_dir.as_deref(), default_dir);
            let prefs_path = base_dir.join("prefs.json");
            let models_dir = base_dir.join("models");
            let models_dir_for_session = models_dir.clone();
            let recovery_dir = base_dir.join("recovery");
            let actions_dir = base_dir.join("actions");
            let prefs = Prefs::load(&prefs_path);
            let shared: Shared = Arc::new(AppShared {
                session: Mutex::new(Session::new(prefs.settings, prefs.keymap)),
                prefs_path,
                models_dir,
                recovery_dir: recovery_dir.clone(),
                actions_dir,
                portable,
                app: app.handle().clone(),
                canvas: Mutex::new(None),
                gpu_error: Mutex::new(None),
            });
            if let Ok(mut session) = shared.session.lock() {
                session.set_recovery_dir(recovery_dir);
                session.set_models_dir(models_dir_for_session);
            }
            spawn_autosave(shared.clone());
            app.manage(shared.clone());
            match native::Canvas::attach(app.handle(), &shared) {
                Ok(canvas) => {
                    if let Ok(mut slot) = shared.canvas.lock() {
                        *slot = Some(canvas);
                    }
                }
                Err(message) => shared.set_gpu_error(message),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::app_snapshot,
            commands::doc_new,
            commands::doc_open,
            commands::doc_save,
            commands::doc_export,
            commands::doc_close,
            commands::doc_activate,
            commands::view_set_canvas,
            commands::view_set_canvas_visible,
            commands::view_zoom,
            commands::view_zoom_to,
            commands::view_fit,
            commands::view_actual_size,
            commands::view_set_settings,
            commands::guide_drag,
            commands::guide_commit,
            commands::guide_cancel,
            commands::guide_remove,
            commands::guides_clear,
            commands::layer_select,
            commands::layer_add,
            commands::group_add,
            commands::layer_set_group,
            commands::layer_move_step,
            commands::layer_remove,
            commands::layer_move,
            commands::layer_set_props,
            commands::layer_set_styles,
            commands::layer_transform,
            commands::adjustment_add,
            commands::adjustment_set_param,
            commands::adjustment_set_curve,
            commands::adjustment_set_stops,
            commands::adjustment_load_lut,
            commands::adjustment_match,
            commands::filter_preview,
            commands::filter_commit,
            commands::filter_cancel,
            commands::fonts_list,
            commands::recovery_list,
            commands::recovery_restore,
            commands::recovery_discard,
            commands::batch_run,
            commands::image_resize,
            commands::export_scales,
            commands::compare_set,
            commands::script_run,
            commands::record_start,
            commands::record_stop,
            commands::actions_list,
            commands::actions_load,
            commands::actions_save,
            commands::actions_delete,
            commands::folder_images,
            commands::ai_select_subject,
            commands::ai_remove_background,
            commands::ai_inpaint,
            commands::ai_models,
            commands::ai_model_import,
            commands::ai_model_remove,
            commands::text_set,
            commands::text_set_path,
            commands::shape_set_style,
            commands::shape_boolean,
            commands::shape_to_selection,
            commands::shape_export_svg,
            commands::layer_rasterize,
            commands::svg_import,
            commands::mask_add,
            commands::mask_remove,
            commands::mask_set_enabled,
            commands::history_undo,
            commands::history_redo,
            commands::op_apply,
            commands::tool_set,
            commands::select_all,
            commands::select_none,
            commands::select_invert,
            commands::select_feather,
            commands::select_grow,
            commands::tool_finish,
            commands::tool_cancel,
            commands::tool_options_set,
            commands::paint_options_set,
            commands::pan_modifier_set,
            commands::theme_set,
            commands::keymap_get,
            commands::keymap_set,
            commands::keymap_reset,
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(|handle, event| {
            // Le rendu doit s'arrêter avant que la fenêtre native ne soit détruite.
            if let tauri::RunEvent::Exit = event {
                if let Some(shared) = handle.try_state::<Shared>() {
                    if let Ok(mut slot) = shared.canvas.lock() {
                        if let Some(canvas) = slot.take() {
                            canvas.shutdown();
                        }
                    }
                }
            }
        }),
        Err(e) => eprintln!("OpenPics n'a pas pu démarrer : {e}"),
    }
}
