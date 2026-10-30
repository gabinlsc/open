//! Thread de rendu : seul propriétaire du `Renderer`.
//!
//! Il tourne à part pour que la présentation (vsync) ne bloque ni l'interface ni les entrées de
//! la fenêtre native. Les demandes arrivent par un canal et sont fusionnées : dix déplacements
//! de souris entre deux images ne donnent qu'un seul dessin, à l'état le plus récent.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use openpics_gpu::{FrameParams, Renderer, Theme, WindowTarget};

use super::Shared;
use crate::dto::ResolvedTheme;

pub enum RenderMsg {
    Redraw,
    Resize(u32, u32),
    Stop,
}

/// Les poignées natives ne sont pas `Send` par construction (certaines plateformes y mettent des
/// pointeurs). Sous Windows ce sont des entiers ; la fenêtre vit plus longtemps que le thread.
struct SendTarget(WindowTarget);
// SAFETY: voir ci-dessus ; la poignée n'est utilisée que pour créer la surface, une fois.
unsafe impl Send for SendTarget {}

pub struct RenderThread {
    pub tx: Sender<RenderMsg>,
    handle: Option<JoinHandle<()>>,
}

impl RenderThread {
    /// # Safety
    /// `target` doit désigner une fenêtre valide qui survit à ce thread (arrêté via `stop`).
    pub unsafe fn spawn(target: WindowTarget, size: (u32, u32), shared: Shared) -> RenderThread {
        let (tx, rx) = mpsc::channel();
        let target = SendTarget(target);
        let handle = std::thread::Builder::new()
            .name("openpics-render".into())
            .spawn(move || {
                let target = target;
                // SAFETY: garanti par l'appelant de `spawn`.
                match unsafe { Renderer::new_for_window(target.0, size) } {
                    Ok(renderer) => run(renderer, rx, &shared),
                    Err(e) => shared.set_gpu_error(e.to_string()),
                }
            })
            .ok();
        RenderThread { tx, handle }
    }

    pub fn stop(mut self) {
        let _ = self.tx.send(RenderMsg::Stop);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn run(mut renderer: Renderer, rx: Receiver<RenderMsg>, shared: &Shared) {
    while let Ok(first) = rx.recv() {
        let mut redraw = false;
        let mut stop = false;
        let mut resize = None;
        for msg in std::iter::once(first).chain(rx.try_iter()) {
            match msg {
                RenderMsg::Redraw => redraw = true,
                RenderMsg::Resize(w, h) => {
                    resize = Some((w, h));
                    redraw = true;
                }
                RenderMsg::Stop => stop = true,
            }
        }
        if stop {
            return;
        }
        if let Some((w, h)) = resize {
            renderer.resize(w, h);
        }
        if !redraw {
            continue;
        }
        // Copie sous verrou, dessin sans : le verrou n'est jamais tenu pendant le vsync.
        let state = match shared.lock() {
            Ok(session) => session.render_state(),
            Err(_) => continue,
        };
        let frame = FrameParams {
            document: state.document.as_ref(),
            viewport: state.viewport,
            settings: state.settings,
            theme: match state.theme {
                ResolvedTheme::Light => Theme::Light,
                ResolvedTheme::Dark => Theme::Dark,
            },
            preview_guide: state.preview_guide,
        };
        if let Err(e) = renderer.render(&frame) {
            eprintln!("rendu impossible : {e}");
        }
    }
}
