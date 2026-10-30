//! Fenêtre native qui porte le canvas (option A de l'architecture : fenêtre enfant + swapchain
//! wgpu). Sous Windows seulement ; ailleurs l'application démarre sans canvas et l'indique.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Canvas;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::Canvas;
