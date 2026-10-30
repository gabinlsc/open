// Pas de console derrière l'application en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    openpics_app::shell::run();
}
