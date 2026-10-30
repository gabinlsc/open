//! Application OpenPics : logique de session (portable) et coque Tauri (Windows).

pub mod batch;
pub mod dto;
pub mod error;
pub mod prefs;
pub mod recovery;
pub mod session;

#[cfg(feature = "shell")]
pub mod shell;
