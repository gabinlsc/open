//! Fenêtre enfant Win32 dédiée au canvas.
//!
//! Elle reçoit directement souris et molette (donc pas de passage par le webview, qui coûterait
//! de la latence) mais ne prend jamais le focus clavier : les raccourcis restent gérés par
//! l'interface web. Le rendu se fait dans un thread séparé (`render.rs`).

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use openpics_core::Vec2;
use raw_window_handle::{HasWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ScreenToClient, ValidateRect};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, ReleaseCapture, SetCapture, VK_MENU};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, HWND_TOP, IDC_CROSS, IDC_HAND,
    IDC_SIZEALL, LoadCursorW, MA_NOACTIVATE, RegisterClassExW, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetCursor, SetWindowLongPtrW, SetWindowPos, WM_CAPTURECHANGED, WM_DROPFILES,
    WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCCREATE, WM_NCDESTROY, WM_PAINT, WM_SETCURSOR, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_ACCEPTFILES, WS_VISIBLE,
};

use crate::dto::{CanvasRect, CursorEvent, Tool};
use crate::session::Pointer;
use crate::shell::Shared;
use crate::shell::render::{RenderMsg, RenderThread};

const CLASS_NAME: &str = "OpenPicsCanvas";
const MK_CONTROL: usize = 0x0008;
const MK_SHIFT: usize = 0x0004;

pub struct Canvas {
    hwnd: isize,
    app: AppHandle,
    tx: Sender<RenderMsg>,
    render: Mutex<Option<RenderThread>>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

impl Canvas {
    /// À appeler depuis le thread principal (`setup` de Tauri) : une fenêtre Win32 ne reçoit ses
    /// messages que sur le thread qui l'a créée.
    pub fn attach(app: &AppHandle, shared: &Shared) -> Result<Canvas, String> {
        let main = app.get_webview_window("main").ok_or("fenêtre principale introuvable")?;
        let parent = match main.window_handle().map_err(|e| e.to_string())?.as_raw() {
            RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
            _ => return Err("poignée de fenêtre inattendue".into()),
        };

        // SAFETY: appels Win32 avec des arguments valides ; `input` est libérée par WM_NCDESTROY.
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(null());
            let class = wide(CLASS_NAME);
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: 0,
                lpfnWndProc: Some(wndproc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class.as_ptr(),
                hIconSm: null_mut(),
            };
            // Un second appel (relance de la fenêtre) échoue avec « classe déjà enregistrée » : sans gravité.
            RegisterClassExW(&wc);
            let input = Box::into_raw(Box::new(Input::new(shared.clone())));
            let hwnd = CreateWindowExW(
                WS_EX_ACCEPTFILES,
                class.as_ptr(),
                null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                1,
                1,
                parent,
                null_mut(),
                hinstance,
                input as *const c_void,
            );
            if hwnd.is_null() {
                drop(Box::from_raw(input));
                return Err("CreateWindowExW a échoué".into());
            }
            DragAcceptFiles(hwnd, 1);
            hwnd
        };

        let mut handle = Win32WindowHandle::new(std::num::NonZeroIsize::new(hwnd as isize).ok_or("poignée nulle")?);
        // SAFETY: GetModuleHandleW(null) renvoie le module courant, toujours valide.
        handle.hinstance = std::num::NonZeroIsize::new(unsafe { GetModuleHandleW(null()) } as isize);
        let target = openpics_gpu::WindowTarget {
            window: RawWindowHandle::Win32(handle),
            display: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
        };
        // SAFETY: la fenêtre enfant vit tant que la fenêtre principale ; le thread est arrêté
        // dans `shutdown`, avant la destruction (RunEvent::Exit).
        let render = unsafe { RenderThread::spawn(target, (1, 1), shared.clone()) };
        let tx = render.tx.clone();
        Ok(Canvas { hwnd: hwnd as isize, app: app.clone(), tx, render: Mutex::new(Some(render)) })
    }

    /// Place la fenêtre sur la zone de canvas mesurée par le frontend (pixels physiques).
    pub fn set_bounds(&self, rect: CanvasRect) {
        let hwnd = self.hwnd;
        let hidden = rect.width == 0 || rect.height == 0;
        let (w, h) = (rect.width.max(1), rect.height.max(1));
        let tx = self.tx.clone();
        // Sur le thread principal, propriétaire de la fenêtre. HWND_TOP à chaque fois : le webview
        // peut se remettre au-dessus de nous quand il se redimensionne.
        let _ = self.app.run_on_main_thread(move || {
            // SAFETY: `hwnd` est une fenêtre enfant valide tant que l'application tourne.
            unsafe {
                if hidden {
                    // Aucun document : le frontend affiche son accueil, que la fenêtre native cacherait.
                    SetWindowPos(
                        hwnd as HWND,
                        null_mut(),
                        0,
                        0,
                        0,
                        0,
                        SWP_HIDEWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    return;
                }
                SetWindowPos(
                    hwnd as HWND,
                    HWND_TOP,
                    rect.x,
                    rect.y,
                    w as i32,
                    h as i32,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            let _ = tx.send(RenderMsg::Resize(w, h));
        });
    }

    /// Masque ou réaffiche la fenêtre native. Elle est toujours au-dessus du webview : tant qu'un menu
    /// ou une boîte de dialogue (HTML) est ouvert, il faut la cacher pour qu'ils restent visibles.
    pub fn set_visible(&self, visible: bool) {
        let hwnd = self.hwnd;
        let tx = self.tx.clone();
        let _ = self.app.run_on_main_thread(move || {
            let flags = SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOACTIVATE
                | if visible { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW | SWP_NOZORDER };
            // SAFETY: `hwnd` est une fenêtre enfant valide tant que l'application tourne.
            unsafe {
                SetWindowPos(hwnd as HWND, HWND_TOP, 0, 0, 0, 0, flags);
            }
            if visible {
                let _ = tx.send(RenderMsg::Redraw);
            }
        });
    }

    pub fn redraw(&self) {
        let _ = self.tx.send(RenderMsg::Redraw);
    }

    pub fn shutdown(self) {
        if let Ok(mut slot) = self.render.lock() {
            if let Some(render) = slot.take() {
                render.stop();
            }
        }
    }
}

/// État de la fenêtre, rattaché à son HWND (GWLP_USERDATA) ; accédé uniquement par le thread de la fenêtre.
struct Input {
    shared: Shared,
    drag_last: Option<(i32, i32)>,
    /// Un geste d'outil (sélection, plus tard peinture) est en cours : la souris est capturée.
    tool_drag: bool,
}

impl Input {
    fn new(shared: Shared) -> Self {
        Self { shared, drag_last: None, tool_drag: false }
    }

    fn start_drag(&mut self, hwnd: HWND, x: i32, y: i32) {
        self.drag_last = Some((x, y));
        // SAFETY: capture de la souris pour la fenêtre courante.
        unsafe { SetCapture(hwnd) };
    }

    fn end_drag(&mut self) {
        if self.drag_last.take().is_some() {
            // SAFETY: relâche la capture éventuellement détenue par ce thread.
            unsafe { ReleaseCapture() };
        }
    }

    fn cursor(&self) -> *const u16 {
        let tool_hand =
            self.shared.lock().map(|s| (s.tool == Tool::Hand || s.pan_modifier, false)).unwrap_or((false, false));
        if self.drag_last.is_some() {
            IDC_SIZEALL
        } else if tool_hand.0 {
            IDC_HAND
        } else {
            // Zoom et outils de sélection : réticule.
            IDC_CROSS
        }
    }

    fn handle(&mut self, hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> Option<LRESULT> {
        let (x, y) = (lp as i16 as i32, (lp >> 16) as i16 as i32);
        match msg {
            WM_MOUSEACTIVATE => Some(MA_NOACTIVATE as LRESULT),
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                // SAFETY: valider la zone évite que Windows renvoie WM_PAINT en boucle.
                unsafe { ValidateRect(hwnd, null()) };
                self.shared.redraw();
                Some(0)
            }
            WM_SETCURSOR => {
                // SAFETY: curseurs système standard.
                unsafe { SetCursor(LoadCursorW(null_mut(), self.cursor())) };
                Some(1)
            }
            WM_MOUSEWHEEL => {
                let notches = f64::from(((wp >> 16) & 0xFFFF) as u16 as i16) / 120.0;
                let mut pt = POINT { x, y };
                // Pour WM_MOUSEWHEEL les coordonnées sont celles de l'écran, pas de la fenêtre.
                // SAFETY: `pt` est un POINT valide, hwnd la fenêtre courante.
                unsafe { ScreenToClient(hwnd, &mut pt) };
                let at = Vec2::new(f64::from(pt.x), f64::from(pt.y));
                let applied = self
                    .shared
                    .lock()
                    .map(|mut s| s.wheel(notches, wp & MK_CONTROL != 0, wp & MK_SHIFT != 0, at).is_ok());
                if applied.unwrap_or(false) {
                    self.shared.native_changed();
                }
                Some(0)
            }
            WM_LBUTTONDOWN => {
                let action = self.shared.lock().map(|s| (s.tool, s.pan_modifier)).unwrap_or((Tool::Hand, false));
                if action.0 == Tool::Hand || action.1 {
                    self.start_drag(hwnd, x, y);
                } else if action.0 != Tool::Zoom {
                    // Outil de sélection : le cœur interprète le geste (voir `session::tools`).
                    let p = pointer(x, y, wp);
                    self.tool_drag = true;
                    // SAFETY: capture de la souris pour la fenêtre courante.
                    unsafe { SetCapture(hwnd) };
                    if self.shared.lock().map(|mut s| s.pointer_down(&p).is_ok()).unwrap_or(false) {
                        self.shared.native_changed();
                    }
                } else {
                    // Outil zoom : clic = avant, Alt+clic = arrière, centré sur le curseur.
                    // SAFETY: lecture d'un état de touche.
                    let alt = unsafe { GetKeyState(i32::from(VK_MENU)) } < 0;
                    let at = Vec2::new(f64::from(x), f64::from(y));
                    if self.shared.lock().map(|mut s| s.zoom_step(!alt, Some(at)).is_ok()).unwrap_or(false) {
                        self.shared.native_changed();
                    }
                }
                Some(0)
            }
            WM_MBUTTONDOWN => {
                self.start_drag(hwnd, x, y);
                Some(0)
            }
            WM_LBUTTONUP | WM_MBUTTONUP => {
                if self.tool_drag && msg == WM_LBUTTONUP {
                    self.tool_drag = false;
                    // SAFETY: relâche la capture prise au clic.
                    unsafe { ReleaseCapture() };
                    if self.shared.lock().map(|mut s| s.pointer_up(&pointer(x, y, wp)).is_ok()).unwrap_or(false) {
                        self.shared.native_changed();
                    }
                }
                self.end_drag();
                Some(0)
            }
            WM_CAPTURECHANGED => {
                self.drag_last = None;
                Some(0)
            }
            WM_MOUSEMOVE => {
                let mut changed = false;
                if let Some((lx, ly)) = self.drag_last {
                    self.drag_last = Some((x, y));
                    changed = self
                        .shared
                        .lock()
                        .map(|mut s| s.pan_by(f64::from(x - lx), f64::from(y - ly)).is_ok())
                        .unwrap_or(false);
                }
                if changed {
                    self.shared.native_changed();
                }
                // Geste d'outil en cours, ou polygone en construction (l'aperçu suit le curseur sans bouton).
                if self.shared.lock().map(|mut s| s.pointer_move(&pointer(x, y, wp)).is_ok()).unwrap_or(false) {
                    self.shared.redraw();
                }
                let at = Vec2::new(f64::from(x), f64::from(y));
                if let Some((p, inside)) = self.shared.lock().ok().and_then(|s| s.cursor_to_doc(at)) {
                    self.shared.emit_cursor(CursorEvent { x: p.x, y: p.y, inside });
                }
                Some(0)
            }
            WM_DROPFILES => {
                let hdrop = wp as HDROP;
                let mut paths = Vec::new();
                // SAFETY: `hdrop` est fourni par Windows pour ce message et libéré par DragFinish.
                unsafe {
                    let count = DragQueryFileW(hdrop, u32::MAX, null_mut(), 0);
                    for i in 0..count {
                        let len = DragQueryFileW(hdrop, i, null_mut(), 0);
                        let mut buf = vec![0u16; len as usize + 1];
                        let copied = DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len + 1);
                        paths.push(PathBuf::from(std::ffi::OsString::from_wide(&buf[..copied as usize])));
                    }
                    DragFinish(hdrop);
                }
                self.shared.open_paths_in_background(paths);
                Some(0)
            }
            _ => None,
        }
    }
}

/// Événement de pointeur pour le cœur : `wp` porte l'état des boutons et de Maj (MK_SHIFT).
fn pointer(x: i32, y: i32, wp: WPARAM) -> Pointer {
    // SAFETY: lecture d'un état de touche.
    let alt = unsafe { GetKeyState(i32::from(VK_MENU)) } < 0;
    Pointer { x: f64::from(x), y: f64::from(y), pressure: 1.0, shift: wp & MK_SHIFT != 0, alt }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: GWLP_USERDATA contient soit 0, soit le pointeur `Input` posé à WM_NCCREATE.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let create = lp as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            WM_NCDESTROY => {
                let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Input;
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, msg, wp, lp)
            }
            _ => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Input;
                match ptr.as_mut().and_then(|input| input.handle(hwnd, msg, wp, lp)) {
                    Some(result) => result,
                    None => DefWindowProcW(hwnd, msg, wp, lp),
                }
            }
        }
    }
}
