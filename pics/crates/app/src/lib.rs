//! Application OpenPics : logique de session (portable) et coque Tauri (Windows).

pub mod dto;
pub mod error;
pub mod prefs;
pub mod session;

#[cfg(feature = "shell")]
pub mod shell;
