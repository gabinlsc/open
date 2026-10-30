use tauri::AppHandle;

use crate::dto::CanvasRect;
use crate::shell::Shared;

pub struct Canvas;

impl Canvas {
    pub fn attach(_app: &AppHandle, _shared: &Shared) -> Result<Canvas, String> {
        Err("le canvas natif n'est disponible que sous Windows".into())
    }

    pub fn set_bounds(&self, _rect: CanvasRect) {}

    pub fn set_visible(&self, _visible: bool) {}

    pub fn redraw(&self) {}

    pub fn shutdown(self) {}
}
